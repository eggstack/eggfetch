//! Policy-driven retry subsystem.
//!
//! Retries are opt-in and idempotency-aware. The retry engine wraps the
//! entire logical request attempt (including redirects) under a single
//! total deadline. Each retry restarts the complete logical request,
//! including redirects, under the original total deadline.
//!
//! # Safety
//!
//! POST and PATCH requests are never retried by default unless explicitly
//! allowed by the policy or protected by an idempotency key. Stream
//! bodies are never retried unless a replay factory is provided.
//!
//! # Backoff
//!
//! Uses bounded exponential backoff with jitter. Tests can use
//! deterministic policies by injecting a fixed random source.

use std::time::Duration;

use http::Method;

use crate::body::RequestBody;
use crate::error::Error;

/// Default retryable status codes: 408, 429, 502, 503, 504.
const DEFAULT_RETRYABLE_STATUSES: &[u16] = &[408, 429, 502, 503, 504];

/// Default retryable methods: GET, HEAD, OPTIONS.
const DEFAULT_RETRYABLE_METHODS: &[Method] = &[Method::GET, Method::HEAD, Method::OPTIONS];

/// Policy-driven retry configuration.
///
/// Controls which requests are retried, how many times, and with what
/// backoff strategy. Retries are opt-in: the default policy disables
/// retries.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use eggfetch_core::RetryPolicy;
///
/// let policy = RetryPolicy::builder()
///     .max_attempts(3)
///     .backoff_factor(0.2)
///     .retry_status(429)
///     .retry_status(503)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of total attempts (1 = no retries).
    max_attempts: usize,
    /// Maximum total elapsed time across all attempts including backoff sleeps.
    max_elapsed: Option<Duration>,
    /// Backoff configuration.
    backoff: BackoffPolicy,
    /// Which HTTP methods are eligible for retry.
    method_policy: MethodPolicy,
    /// Which status codes trigger a retry.
    status_policy: StatusPolicy,
    /// Whether to respect `Retry-After` headers.
    respect_retry_after: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            max_elapsed: None,
            backoff: BackoffPolicy::default(),
            method_policy: MethodPolicy::default(),
            status_policy: StatusPolicy::default(),
            respect_retry_after: false,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy builder.
    #[must_use]
    pub fn builder() -> RetryPolicyBuilder {
        RetryPolicyBuilder::new()
    }

    /// Returns the maximum number of total attempts.
    #[must_use]
    pub fn max_attempts(&self) -> usize {
        self.max_attempts
    }

    /// Returns the maximum total elapsed time.
    #[must_use]
    pub fn max_elapsed(&self) -> Option<Duration> {
        self.max_elapsed
    }

    /// Returns a reference to the backoff policy.
    #[must_use]
    pub fn backoff(&self) -> &BackoffPolicy {
        &self.backoff
    }

    /// Returns a reference to the method policy.
    #[must_use]
    pub fn method_policy(&self) -> &MethodPolicy {
        &self.method_policy
    }

    /// Returns a reference to the status policy.
    #[must_use]
    pub fn status_policy(&self) -> &StatusPolicy {
        &self.status_policy
    }

    /// Returns whether `Retry-After` headers are respected.
    #[must_use]
    pub fn respect_retry_after(&self) -> bool {
        self.respect_retry_after
    }

    /// Returns `true` if retries are effectively enabled (`max_attempts` > 1).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.max_attempts > 1
    }

    /// Returns `true` if the given method is eligible for retry.
    #[must_use]
    pub fn is_method_retryable(&self, method: &Method) -> bool {
        self.method_policy.is_retryable(method)
    }

    /// Returns `true` if the given status code is eligible for retry.
    #[must_use]
    pub fn is_status_retryable(&self, status: u16) -> bool {
        self.status_policy.is_retryable(status)
    }

    /// Returns `true` if the given error is eligible for retry.
    #[must_use]
    pub fn is_error_retryable(error: &Error) -> bool {
        match error {
            Error::Connect(_)
            | Error::Io(_)
            | Error::Hyper(_)
            | Error::HyperClient(_)
            | Error::Timeout {
                phase: crate::timeout::TimeoutPhase::Connect,
                ..
            }
            | Error::Timeout {
                phase: crate::timeout::TimeoutPhase::Pool,
                ..
            }
            | Error::H3Connect(_)
            | Error::H3ConnectionClosed(_) => true,
            Error::Http2StreamReset { reason } if reason.starts_with("REFUSED_STREAM") => true,
            _ => false,
        }
    }

    /// Compute the backoff delay for the given attempt number (1-indexed).
    ///
    /// Returns `None` if no delay is needed (attempt 1).
    #[must_use]
    pub fn backoff_delay(&self, attempt: usize) -> Option<Duration> {
        self.backoff.delay(attempt)
    }

    /// Compute the delay after receiving a `Retry-After` header value.
    ///
    /// Parses `Retry-After` as either a number of seconds or an HTTP-date.
    /// Returns `None` if parsing fails, if a past HTTP-date is supplied
    /// (the header carries no useful information), or if the value
    /// exceeds the backoff cap.
    #[must_use]
    pub fn retry_after_delay(&self, retry_after: &str) -> Option<Duration> {
        if !self.respect_retry_after {
            return None;
        }
        let delay = if let Ok(secs) = retry_after.trim().parse::<u64>() {
            Duration::from_secs(secs)
        } else if let Ok(date) = httpdate::parse_http_date(retry_after.trim()) {
            match date.duration_since(std::time::SystemTime::now()) {
                Ok(duration) => duration,
                Err(_) => return None,
            }
        } else {
            return None;
        };
        let cap = self.backoff.max_delay;
        Some(if delay > cap { cap } else { delay })
    }

    /// Check if a request body is replayable for retry.
    ///
    /// Returns `ReplayCheck::Replayable` if the body can be replayed,
    /// or `ReplayCheck::NotReplayable` if the body is a live stream.
    #[must_use]
    pub fn check_body_replayable(body: &RequestBody) -> ReplayCheck {
        if body.is_replayable() {
            ReplayCheck::Replayable
        } else {
            ReplayCheck::NotReplayable
        }
    }
}

/// Builder for constructing a [`RetryPolicy`].
pub struct RetryPolicyBuilder {
    max_attempts: usize,
    max_elapsed: Option<Duration>,
    backoff: BackoffPolicyBuilder,
    method_policy: MethodPolicy,
    status_policy: StatusPolicy,
    respect_retry_after: bool,
    custom_statuses_used: bool,
}

impl RetryPolicyBuilder {
    /// Create a new builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_attempts: 1,
            max_elapsed: None,
            backoff: BackoffPolicyBuilder::new(),
            method_policy: MethodPolicy::default(),
            status_policy: StatusPolicy::default(),
            respect_retry_after: false,
            custom_statuses_used: false,
        }
    }

    /// Set the maximum number of total attempts (including the first request).
    ///
    /// A value of 1 means no retries. A value of 3 means up to 2 retries.
    #[must_use]
    pub fn max_attempts(mut self, max: usize) -> Self {
        self.max_attempts = max;
        self
    }

    /// Set the maximum total elapsed time across all attempts.
    ///
    /// Includes backoff sleeps. If `None`, no total time limit is applied.
    #[must_use]
    pub fn max_elapsed(mut self, elapsed: Duration) -> Self {
        self.max_elapsed = Some(elapsed);
        self
    }

    /// Set the backoff factor for exponential backoff.
    ///
    /// The delay for attempt `n` is `factor * 2^(n-1)` with jitter.
    #[must_use]
    pub fn backoff_factor(mut self, factor: f64) -> Self {
        self.backoff = self.backoff.factor(factor);
        self
    }

    /// Set the maximum delay between retries.
    #[must_use]
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.backoff = self.backoff.max_delay(delay);
        self
    }

    /// Set the initial delay for the first retry.
    #[must_use]
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.backoff = self.backoff.initial_delay(delay);
        self
    }

    /// Add a retryable status code.
    ///
    /// The first call to `retry_status` replaces the default set
    /// (408, 429, 502, 503, 504). Subsequent calls add to the set.
    #[must_use]
    pub fn retry_status(mut self, status: u16) -> Self {
        if !self.custom_statuses_used {
            self.status_policy = StatusPolicy::default_empty();
            self.custom_statuses_used = true;
        }
        self.status_policy.add_status(status);
        self
    }

    /// Set the retryable status codes, replacing any existing ones.
    #[must_use]
    pub fn retry_statuses(mut self, statuses: impl IntoIterator<Item = u16>) -> Self {
        self.status_policy = StatusPolicy::new(statuses);
        self.custom_statuses_used = true;
        self
    }

    /// Add a retryable method.
    #[must_use]
    pub fn retry_method(mut self, method: Method) -> Self {
        self.method_policy.add_method(method);
        self
    }

    /// Allow POST requests to be retried.
    #[must_use]
    pub fn allow_post_retry(mut self) -> Self {
        self.method_policy.add_method(Method::POST);
        self
    }

    /// Allow PUT requests to be retried.
    #[must_use]
    pub fn allow_put_retry(mut self) -> Self {
        self.method_policy.add_method(Method::PUT);
        self
    }

    /// Allow DELETE requests to be retried.
    #[must_use]
    pub fn allow_delete_retry(mut self) -> Self {
        self.method_policy.add_method(Method::DELETE);
        self
    }

    /// Allow PATCH requests to be retried.
    #[must_use]
    pub fn allow_patch_retry(mut self) -> Self {
        self.method_policy.add_method(Method::PATCH);
        self
    }

    /// Enable or disable `Retry-After` header support.
    #[must_use]
    pub fn respect_retry_after(mut self, respect: bool) -> Self {
        self.respect_retry_after = respect;
        self
    }

    /// Build the retry policy.
    #[must_use]
    pub fn build(self) -> RetryPolicy {
        RetryPolicy {
            max_attempts: self.max_attempts,
            max_elapsed: self.max_elapsed,
            backoff: self.backoff.build(),
            method_policy: self.method_policy,
            status_policy: self.status_policy,
            respect_retry_after: self.respect_retry_after,
        }
    }
}

impl Default for RetryPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Backoff strategy for retries.
#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    /// Base factor for exponential backoff.
    factor: f64,
    /// Maximum delay cap.
    max_delay: Duration,
    /// Initial delay for the first retry.
    initial_delay: Duration,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            factor: 0.5,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(500),
        }
    }
}

impl BackoffPolicy {
    /// Compute the delay for the given attempt number (1-indexed).
    ///
    /// Attempt 1 returns `None` (no delay before the first attempt).
    /// Later attempts use exponential backoff with full jitter: the
    /// delay is drawn uniformly from `[0, capped)`, where `capped` is
    /// the exponentially increasing delay clamped to `max_delay`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn delay(&self, attempt: usize) -> Option<Duration> {
        if attempt <= 1 {
            return None;
        }

        // Exponential backoff: initial_delay * factor^(attempt-2)
        let exp = (attempt - 2) as f64;
        let raw = self.initial_delay.as_secs_f64() * self.factor.powf(exp);

        // Guard against NaN, inf, negative, and values exceeding max_delay
        // before converting to Duration. `Duration::from_secs_f64` panics on
        // non-finite, negative, or oversized floats.
        let capped = if !raw.is_finite() || raw < 0.0 || raw > self.max_delay.as_secs_f64() {
            self.max_delay
        } else {
            Duration::from_secs_f64(raw)
        };

        // Full jitter: uniform value in [0, capped) so retries in the
        // saturated regime do not all collapse onto the cap (which would
        // re-create the thundering-herd pattern jitter exists to prevent).
        // If the system RNG fails, degrade to the deterministic capped
        // delay instead of panicking.
        let jittered_secs = match get_random_f64() {
            Some(jitter) => capped.as_secs_f64() * jitter,
            None => capped.as_secs_f64(),
        };

        let final_delay = if !jittered_secs.is_finite() || jittered_secs < 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(jittered_secs)
        };

        Some(final_delay)
    }

    /// Returns the maximum delay.
    #[must_use]
    pub fn max_delay(&self) -> Duration {
        self.max_delay
    }

    /// Returns the factor.
    #[must_use]
    pub fn factor(&self) -> f64 {
        self.factor
    }

    /// Returns the initial delay.
    #[must_use]
    pub fn initial_delay(&self) -> Duration {
        self.initial_delay
    }
}

/// Builder for [`BackoffPolicy`].
pub struct BackoffPolicyBuilder {
    factor: f64,
    max_delay: Duration,
    initial_delay: Duration,
}

impl BackoffPolicyBuilder {
    #[must_use]
    fn new() -> Self {
        Self {
            factor: 0.5,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(500),
        }
    }

    #[must_use]
    fn factor(mut self, factor: f64) -> Self {
        self.factor = factor;
        self
    }

    #[must_use]
    fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    #[must_use]
    fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    #[must_use]
    fn build(self) -> BackoffPolicy {
        BackoffPolicy {
            factor: self.factor,
            max_delay: self.max_delay,
            initial_delay: self.initial_delay,
        }
    }
}

/// Policy controlling which HTTP methods are eligible for retry.
#[derive(Debug, Clone)]
pub struct MethodPolicy {
    /// Set of methods eligible for retry.
    methods: Vec<Method>,
}

impl Default for MethodPolicy {
    fn default() -> Self {
        Self {
            methods: DEFAULT_RETRYABLE_METHODS.to_vec(),
        }
    }
}

impl MethodPolicy {
    /// Create a method policy with the given methods.
    #[must_use]
    pub fn new(methods: Vec<Method>) -> Self {
        Self { methods }
    }

    /// Add a method to the retryable set.
    pub fn add_method(&mut self, method: Method) {
        if !self.methods.contains(&method) {
            self.methods.push(method);
        }
    }

    /// Returns `true` if the method is retryable.
    #[must_use]
    pub fn is_retryable(&self, method: &Method) -> bool {
        self.methods.contains(method)
    }

    /// Returns the list of retryable methods.
    #[must_use]
    pub fn methods(&self) -> &[Method] {
        &self.methods
    }
}

/// Policy controlling which status codes trigger a retry.
#[derive(Debug, Clone)]
pub struct StatusPolicy {
    /// Set of status codes eligible for retry.
    statuses: Vec<u16>,
}

impl Default for StatusPolicy {
    fn default() -> Self {
        Self {
            statuses: DEFAULT_RETRYABLE_STATUSES.to_vec(),
        }
    }
}

impl StatusPolicy {
    /// Create a status policy with the given codes.
    #[must_use]
    pub fn new(statuses: impl IntoIterator<Item = u16>) -> Self {
        Self {
            statuses: statuses.into_iter().collect(),
        }
    }

    /// Create an empty status policy (no retryable statuses).
    #[must_use]
    pub fn default_empty() -> Self {
        Self {
            statuses: Vec::new(),
        }
    }

    /// Add a status code to the retryable set.
    pub fn add_status(&mut self, status: u16) {
        if !self.statuses.contains(&status) {
            self.statuses.push(status);
        }
    }

    /// Returns `true` if the status code is retryable.
    #[must_use]
    pub fn is_retryable(&self, status: u16) -> bool {
        self.statuses.contains(&status)
    }

    /// Returns the list of retryable status codes.
    #[must_use]
    pub fn statuses(&self) -> &[u16] {
        &self.statuses
    }
}

/// Result of a body replayability check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayCheck {
    /// The body can be replayed.
    Replayable,
    /// The body cannot be replayed.
    NotReplayable,
}

/// The cause of a retry decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryCause {
    /// A transport error occurred (connection refused, reset, etc.).
    TransportError(String),
    /// A retryable HTTP status code was received.
    Status(u16),
    /// A timeout occurred during a retryable phase.
    Timeout(String),
}

impl std::fmt::Display for RetryCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransportError(msg) => write!(f, "transport error: {msg}"),
            Self::Status(status) => write!(f, "retryable status: {status}"),
            Self::Timeout(phase) => write!(f, "timeout: {phase}"),
        }
    }
}

/// Context for a retry attempt.
#[derive(Debug, Clone)]
pub struct RetryContext {
    /// The current attempt number (1-indexed).
    pub attempt: usize,
    /// The HTTP method.
    pub method: Method,
    /// Whether the request body is replayable.
    pub body_replayable: bool,
    /// The cause that triggered this retry.
    pub cause: RetryCause,
    /// Remaining total budget.
    pub remaining_total: Option<Duration>,
}

/// Determine if an error/status should trigger a retry.
///
/// Returns `Some(RetryCause)` if the error is retryable under the given
/// policy, `None` otherwise.
#[must_use]
pub fn should_retry(
    policy: &RetryPolicy,
    method: &Method,
    body: &RequestBody,
    error: Option<&Error>,
    status: Option<u16>,
) -> Option<RetryCause> {
    // Check method eligibility
    if !policy.is_method_retryable(method) {
        return None;
    }

    // Check body replayability
    if !body.is_replayable() {
        return None;
    }

    // Check error-based retry
    if let Some(err) = error {
        if RetryPolicy::is_error_retryable(err) {
            return Some(RetryCause::TransportError(err.to_string()));
        }
        return None;
    }

    // Check status-based retry
    if let Some(status) = status {
        if policy.is_status_retryable(status) {
            return Some(RetryCause::Status(status));
        }
    }

    None
}

/// Get a random f64 in [0, 1) using getrandom.
///
/// Returns `None` if the system RNG fails so callers can degrade
/// gracefully (e.g. to a deterministic delay) instead of panicking.
#[allow(clippy::cast_precision_loss)]
fn get_random_f64() -> Option<f64> {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).ok()?;
    let val = u64::from_le_bytes(buf);
    // Use the top 53 bits so the value is exactly representable and the
    // result stays strictly below 1.0 (dividing a rounded u64 by
    // `u64::MAX as f64` could round up to exactly 1.0).
    Some(((val >> 11) as f64) / ((1u64 << 53) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn retry_policy_default_disables_retries() {
        let policy = RetryPolicy::default();
        assert!(!policy.is_enabled());
        assert_eq!(policy.max_attempts(), 1);
    }

    #[test]
    fn jitter_is_strictly_below_one() {
        // The documented contract is [0, 1); a sample that rounds up to
        // exactly 1.0 would let the backoff delay reach the capped max.
        for _ in 0..10_000 {
            let Some(jitter) = get_random_f64() else {
                // RNG unavailable in this environment: the production
                // path degrades to a deterministic delay, so there is
                // nothing to assert here.
                return;
            };
            assert!((0.0..1.0).contains(&jitter));
        }
    }

    #[test]
    fn retry_policy_builder_basic() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .backoff_factor(0.2)
            .build();
        assert!(policy.is_enabled());
        assert_eq!(policy.max_attempts(), 3);
    }

    #[test]
    fn retry_policy_method_default_safe() {
        let policy = RetryPolicy::default();
        assert!(policy.is_method_retryable(&Method::GET));
        assert!(policy.is_method_retryable(&Method::HEAD));
        assert!(policy.is_method_retryable(&Method::OPTIONS));
        assert!(!policy.is_method_retryable(&Method::POST));
        assert!(!policy.is_method_retryable(&Method::PUT));
        assert!(!policy.is_method_retryable(&Method::DELETE));
        assert!(!policy.is_method_retryable(&Method::PATCH));
    }

    #[test]
    fn retry_policy_method_allow_post() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .allow_post_retry()
            .build();
        assert!(policy.is_method_retryable(&Method::POST));
    }

    #[test]
    fn retry_policy_status_default() {
        let policy = RetryPolicy::default();
        assert!(policy.is_status_retryable(408));
        assert!(policy.is_status_retryable(429));
        assert!(policy.is_status_retryable(502));
        assert!(policy.is_status_retryable(503));
        assert!(policy.is_status_retryable(504));
        assert!(!policy.is_status_retryable(200));
        assert!(!policy.is_status_retryable(404));
    }

    #[test]
    fn retry_policy_status_custom() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .retry_status(429)
            .retry_status(503)
            .build();
        assert!(policy.is_status_retryable(429));
        assert!(policy.is_status_retryable(503));
        assert!(!policy.is_status_retryable(408));
        assert!(!policy.is_status_retryable(504));
    }

    #[test]
    fn backoff_delay_none_for_first_attempt() {
        let backoff = BackoffPolicy::default();
        assert!(backoff.delay(1).is_none());
    }

    #[test]
    fn backoff_delay_bounded_for_later_attempts() {
        let backoff = BackoffPolicy {
            factor: 0.5,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(100),
        };
        // Full jitter: the delay is drawn from [0, capped).
        let d2 = backoff.delay(2).unwrap();
        assert!(d2 <= Duration::from_millis(100));

        let d3 = backoff.delay(3).unwrap();
        assert!(d3 <= Duration::from_millis(50));
    }

    #[test]
    fn backoff_delay_capped_at_max() {
        let backoff = BackoffPolicy {
            factor: 2.0,
            max_delay: Duration::from_secs(5),
            initial_delay: Duration::from_secs(1),
        };
        let d = backoff.delay(10).unwrap();
        assert!(d < Duration::from_secs(5)); // full jitter stays below cap
    }

    #[test]
    fn retry_after_disabled_by_default() {
        let policy = RetryPolicy::default();
        assert!(policy.retry_after_delay("5").is_none());
    }

    #[test]
    fn retry_after_respected_when_enabled() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .respect_retry_after(true)
            .build();
        let delay = policy.retry_after_delay("5").unwrap();
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn retry_after_capped_at_max_delay() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .respect_retry_after(true)
            .max_delay(Duration::from_secs(10))
            .build();
        let delay = policy.retry_after_delay("60").unwrap();
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn retry_after_invalid_ignored() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .respect_retry_after(true)
            .build();
        assert!(policy.retry_after_delay("not-a-number").is_none());
    }

    #[test]
    fn retry_after_http_date_parses() {
        use std::time::UNIX_EPOCH;

        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .respect_retry_after(true)
            .build();
        // Build a future SystemTime and format it with httpdate, then parse it back
        // to guarantee the day-of-week matches
        let future_time = UNIX_EPOCH
            + Duration::from_secs(
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ); // 1 hour from now
        let formatted = httpdate::fmt_http_date(future_time);
        let parsed = httpdate::parse_http_date(&formatted);
        assert!(parsed.is_ok(), "round-trip parse failed: {formatted}");
        let delay = policy.retry_after_delay(&formatted);
        assert!(
            delay.is_some(),
            "retry_after_delay returned None for HTTP-date: {formatted}"
        );
        let d = delay.unwrap();
        assert!(d.as_secs() > 0, "delay should be positive");
    }

    #[test]
    fn retry_after_past_http_date_is_ignored() {
        use std::time::UNIX_EPOCH;

        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .respect_retry_after(true)
            .build();
        // One hour in the past: the header carries no useful information,
        // so it must be ignored (None) rather than mapped to zero.
        let past_time = UNIX_EPOCH
            + Duration::from_secs(
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .saturating_sub(3600),
            );
        let formatted = httpdate::fmt_http_date(past_time);
        assert!(policy.retry_after_delay(&formatted).is_none());
    }

    #[test]
    fn body_replayable_empty() {
        let body = RequestBody::Empty;
        assert_eq!(
            RetryPolicy::check_body_replayable(&body),
            ReplayCheck::Replayable
        );
    }

    #[test]
    fn body_replayable_bytes() {
        let body = RequestBody::from(bytes::Bytes::from("hello"));
        assert_eq!(
            RetryPolicy::check_body_replayable(&body),
            ReplayCheck::Replayable
        );
    }

    #[test]
    fn body_not_replayable_stream() {
        let body = RequestBody::from_stream(
            futures_util::stream::empty::<std::result::Result<bytes::Bytes, crate::Error>>(),
            None,
        );
        assert_eq!(
            RetryPolicy::check_body_replayable(&body),
            ReplayCheck::NotReplayable
        );
    }

    #[test]
    fn should_retry_get_with_retryable_status() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let cause = should_retry(&policy, &Method::GET, &body, None, Some(503));
        assert_eq!(cause, Some(RetryCause::Status(503)));
    }

    #[test]
    fn should_not_retry_post_by_default() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let cause = should_retry(&policy, &Method::POST, &body, None, Some(503));
        assert_eq!(cause, None);
    }

    #[test]
    fn should_retry_post_when_allowed() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .allow_post_retry()
            .build();
        let body = RequestBody::Empty;
        let cause = should_retry(&policy, &Method::POST, &body, None, Some(503));
        assert_eq!(cause, Some(RetryCause::Status(503)));
    }

    #[test]
    fn should_not_retry_non_retryable_status() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let cause = should_retry(&policy, &Method::GET, &body, None, Some(200));
        assert_eq!(cause, None);
    }

    #[test]
    fn should_not_retry_stream_body() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::from_stream(
            futures_util::stream::empty::<std::result::Result<bytes::Bytes, crate::Error>>(),
            None,
        );
        let cause = should_retry(&policy, &Method::GET, &body, None, Some(503));
        assert_eq!(cause, None);
    }

    #[test]
    fn should_retry_connect_error() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::Connect("connection refused".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert!(matches!(cause, Some(RetryCause::TransportError(_))));
    }

    #[test]
    fn should_not_retry_invalid_url() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::InvalidUrl("bad url".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert_eq!(cause, None);
    }

    #[test]
    #[cfg(feature = "http3")]
    fn should_retry_h3_connect_error() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::H3Connect("QUIC handshake failed".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert!(matches!(cause, Some(RetryCause::TransportError(_))));
    }

    #[test]
    #[cfg(feature = "http3")]
    fn should_retry_h3_connection_closed() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::H3ConnectionClosed("peer sent GOAWAY".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert!(matches!(cause, Some(RetryCause::TransportError(_))));
    }

    #[test]
    #[cfg(feature = "http3")]
    fn should_not_retry_h3_protocol_error() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::H3Protocol("unexpected frame".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert_eq!(cause, None);
    }

    #[test]
    #[cfg(feature = "http3")]
    fn should_not_retry_h3_stream_error() {
        let policy = RetryPolicy::builder().max_attempts(3).build();
        let body = RequestBody::Empty;
        let err = Error::H3Stream("stream reset".into());
        let cause = should_retry(&policy, &Method::GET, &body, Some(&err), None);
        assert_eq!(cause, None);
    }

    #[test]
    fn retry_cause_display() {
        assert_eq!(
            RetryCause::TransportError("refused".into()).to_string(),
            "transport error: refused"
        );
        assert_eq!(RetryCause::Status(503).to_string(), "retryable status: 503");
        assert_eq!(
            RetryCause::Timeout("total".into()).to_string(),
            "timeout: total"
        );
    }

    #[test]
    fn method_policy_add_method() {
        let mut policy = MethodPolicy::default();
        assert!(!policy.is_retryable(&Method::POST));
        policy.add_method(Method::POST);
        assert!(policy.is_retryable(&Method::POST));
    }

    #[test]
    fn status_policy_add_status() {
        let mut policy = StatusPolicy::default();
        assert!(!policy.is_retryable(418));
        policy.add_status(418);
        assert!(policy.is_retryable(418));
    }

    #[test]
    fn retry_policy_max_elapsed() {
        let policy = RetryPolicy::builder()
            .max_attempts(3)
            .max_elapsed(Duration::from_secs(10))
            .build();
        assert_eq!(policy.max_elapsed(), Some(Duration::from_secs(10)));
    }

    #[test]
    fn retry_policy_display_cause() {
        let cause = RetryCause::Status(429);
        assert_eq!(format!("{cause}"), "retryable status: 429");
    }

    #[test]
    fn prop_backoff_delay_some_for_attempt_gt_1() {
        proptest::proptest!(|(attempt in 2usize..100usize)| {
            let backoff = BackoffPolicy::default();
            prop_assert!(backoff.delay(attempt).is_some());
        });
    }

    #[test]
    fn prop_backoff_delay_bounded_by_max_delay() {
        proptest::proptest!(|(
            factor in 0.1f64..2.0f64,
            initial_ms in 1u64..1000u64,
            max_secs in 1u64..60u64,
            attempt in 2usize..20usize,
        )| {
            let backoff = BackoffPolicy {
                factor,
                max_delay: Duration::from_secs(max_secs),
                initial_delay: Duration::from_millis(initial_ms),
            };
            if let Some(delay) = backoff.delay(attempt) {
                prop_assert!(delay <= Duration::from_secs(max_secs * 2));
            }
        });
    }

    #[test]
    fn prop_method_policy_default_contains_get_head_options() {
        let policy = MethodPolicy::default();
        assert!(policy.is_retryable(&Method::GET));
        assert!(policy.is_retryable(&Method::HEAD));
        assert!(policy.is_retryable(&Method::OPTIONS));
    }

    #[test]
    fn prop_status_policy_default_contains_expected() {
        let policy = StatusPolicy::default();
        assert!(policy.is_retryable(408));
        assert!(policy.is_retryable(429));
        assert!(policy.is_retryable(502));
        assert!(policy.is_retryable(503));
        assert!(policy.is_retryable(504));
    }

    #[test]
    fn prop_retry_after_delay_returns_none_for_garbage() {
        proptest::proptest!(|(input in ".{0,100}")| {
            let policy = RetryPolicy::builder()
                .max_attempts(3)
                .respect_retry_after(true)
                .build();
            let _ = policy.retry_after_delay(&input);
        });
    }

    #[test]
    fn regression_fuzz_retry_crash_extreme_backoff_factor() {
        let backoff = BackoffPolicy {
            factor: 22.0,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(500),
        };
        for attempt in 2..=20 {
            let delay = backoff.delay(attempt);
            assert!(delay.is_some());
            assert!(delay.unwrap() <= backoff.max_delay);
        }
    }

    #[test]
    fn backoff_delay_nan_factor_returns_bounded_delay() {
        let backoff = BackoffPolicy {
            factor: f64::NAN,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(500),
        };
        let d = backoff.delay(5).unwrap();
        assert!(d <= Duration::from_secs(30));
    }

    #[test]
    fn backoff_delay_infinite_factor_returns_bounded_delay() {
        let backoff = BackoffPolicy {
            factor: f64::INFINITY,
            max_delay: Duration::from_secs(30),
            initial_delay: Duration::from_millis(500),
        };
        let d = backoff.delay(5).unwrap();
        assert!(d <= Duration::from_secs(30));
    }

    #[test]
    fn prop_is_error_retryable_never_panics() {
        proptest::proptest!(|(msg in ".{0,200}")| {
            let errors = vec![
                Error::Connect(msg.clone()),
                Error::Io(std::sync::Arc::new(std::io::Error::other(msg.clone()))),
                Error::InvalidUrl(msg.clone()),
                Error::Tls(msg.clone()),
                Error::Protocol(msg.clone()),
                Error::Body(msg.clone()),
            ];
            for err in &errors {
                let _ = RetryPolicy::is_error_retryable(err);
            }
        });
    }
}
