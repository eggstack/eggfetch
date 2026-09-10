//! Authenticated Alt-Svc discovery, bounded alternative-service state,
//! and broken-route suppression for HTTP/3.
//!
//! This module is the single owner of Alt-Svc semantics. It is logically
//! separate from [`super::http3::H3Connector`]'s `sender_cache`: an origin
//! may advertise an alternative even when no QUIC connection exists, and a
//! failed H3 connection must not erase the origin's advertised HTTPS
//! semantics.
//!
//! # Model
//!
//! - Origin key is the normalized `(scheme, host, port)` of the
//!   authenticated HTTPS origin. Only `https` origins are learnable.
//! - Only the `h3` protocol identifier is recorded; all other alternatives
//!   (`h2`, `http/1.1`, draft `h3-XX`, …) are ignored.
//! - Each origin holds at most one fresh alternative (authority + port +
//!   expiry + generation). The cache is bounded at
//!   [`ALTSVC_CACHE_MAX_ENTRIES`] with arbitrary-entry eviction (no LRU
//!   dependency, matching the H3/SOCKS caches).
//! - `ma` expiry and `clear` are honored (RFC 7838 §3). `ma=0` means
//!   immediately expired and never becomes a fresh route.
//! - No credentials, auth headers, cookies, or URL userinfo are stored.
//!   The alternative authority is transport routing only; cookies, auth,
//!   `Host`, and security policy always use the logical origin.
//! - Suppression is routing suppression, not request retry policy. A failing
//!   alternative is skipped during backoff; success clears it and a new
//!   advertisement generation re-enables the route without waiting.
//!
//! # Parsing bounds (fail closed)
//!
//! - Total header value over [`ALTSVC_MAX_HEADER_BYTES`] is rejected.
//! - At most [`ALTSVC_MAX_ALTERNATIVES`] alternatives are parsed; extras ignored.
//! - Malformed fields, bad quoting, unknown protocol IDs, zero `ma`,
//!   `ma` overflow, and bad ports are ignored per-entry, never panicking
//!   and never allocating without bound.
//!
//! # Trust
//!
//! Learning is allowed only from an authenticated HTTPS response context
//! (see [`AltSvcLearnContext`]). Plaintext `http`, proxy-routed responses,
//! and cross-origin confusion are rejected before parsing. Alternative
//! endpoints never change origin authentication: QUIC TLS validates the
//! original origin (SNI = origin host), not the alternative hostname.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Upper bound on cached Alt-Svc origin entries.
///
/// Matches the H3 connection cache (64) without a new LRU dependency.
/// Eviction is arbitrary-entry and never removes the origin being inserted.
pub const ALTSVC_CACHE_MAX_ENTRIES: usize = 64;

/// Upper bound on a single combined Alt-Svc header value.
///
/// Hostile oversized headers fail closed (entire value rejected) instead of
/// allocating without bound.
pub const ALTSVC_MAX_HEADER_BYTES: usize = 8192;

/// Upper bound on alternatives parsed from one header value.
pub const ALTSVC_MAX_ALTERNATIVES: usize = 16;

/// Default `ma` (max-age) when the parameter is absent, per RFC 7838 §3 (24h).
pub const ALTSVC_DEFAULT_MA_SECS: u64 = 86_400;

/// Base backoff for broken-route suppression.
pub const SUPPRESSION_BASE: Duration = Duration::from_secs(5);

/// Maximum backoff for broken-route suppression.
pub const SUPPRESSION_MAX: Duration = Duration::from_secs(300);

/// Upper bound on suppression entries (per-origin).
pub const SUPPRESSION_MAX_ENTRIES: usize = 64;

/// Normalized authenticated origin.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AltSvcOrigin {
    scheme: String,
    host: String,
    port: u16,
}

impl AltSvcOrigin {
    /// Normalize `scheme/host/port` for cache keying.
    ///
    /// Host is lowercased; an empty host fails. Port must be non-zero.
    /// Returns `None` for unusable origins (fail closed, no panic).
    #[must_use]
    pub fn new(scheme: &str, host: &str, port: u16) -> Option<Self> {
        if host.is_empty() || port == 0 {
            return None;
        }
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        if host.is_empty() || host.len() > 253 {
            return None;
        }
        // Reject userinfo-looking or path-looking hosts before they can
        // become cache keys.
        if host.contains(['@', '/', '?', '#', ' ', '\t', '\r', '\n']) {
            return None;
        }
        Some(Self {
            scheme: scheme.to_ascii_lowercase(),
            host,
            port,
        })
    }

    /// Build from a `url::Url`. Returns `None` when the URL has no host.
    #[must_use]
    pub fn from_url(url: &url::Url) -> Option<Self> {
        let host = url.host_str()?;
        let port = url.port_or_known_default().unwrap_or(443);
        Self::new(url.scheme(), host, port)
    }

    /// Cache key string (`scheme://host:port`), for debugging only.
    #[must_use]
    pub fn cache_key(&self) -> String {
        format!("{}://{}:{}", self.scheme, self.host, self.port)
    }

    /// Logical host (for SNI / Host / cookie scope; never the alternative).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Logical port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Logical scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
}

/// A fresh H3 alternative for one origin.
#[derive(Debug, Clone)]
pub struct AltSvcEntry {
    /// Alternative host (resolved; equals origin host when advertisement
    /// used `:port` form).
    pub alt_host: String,
    /// Alternative UDP port.
    pub alt_port: u16,
    /// When the advertisement expires.
    pub expires_at: Instant,
    /// Monotonic generation: each successful learn bumps the cache-wide
    /// sequence so suppression can tell “new advertisement” from stale
    /// failure state.
    pub generation: u64,
}

impl AltSvcEntry {
    /// Returns `true` when `now` is at or past expiry.
    #[must_use]
    pub fn is_expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }
}

/// Trust context for learning an advertisement.
///
/// All fields must hold for learning; otherwise the value is rejected
/// before parsing (counted as `altsvc_rejected` by the caller).
#[derive(Debug, Clone, Copy)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "trust is four orthogonal booleans (https, authenticated, proxy, hop); an enum would obscure call sites"
)]
pub struct AltSvcLearnContext {
    /// Response URL scheme is `https`.
    pub is_https: bool,
    /// TLS verification was actually performed for this origin
    /// (not plaintext, not `danger_accept_invalid_certs`).
    pub tls_authenticated: bool,
    /// Response did not arrive via a proxy route (no proxy-added headers).
    pub via_proxy: bool,
    /// The header belongs to this hop's logical origin (redirect hops learn
    /// only for their own origin).
    pub origin_matches_hop: bool,
}

impl AltSvcLearnContext {
    /// Returns `true` when learning is permitted.
    #[must_use]
    pub fn trustworthy(self) -> bool {
        self.is_https && self.tls_authenticated && !self.via_proxy && self.origin_matches_hop
    }
}

/// Outcome of a learn attempt (for metrics, no secrets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltSvcLearnOutcome {
    /// No Alt-Svc header present; nothing changed.
    Absent,
    /// Fresh H3 alternative cached.
    Learned,
    /// `clear` or empty-after-filter; existing entry removed if present.
    Cleared,
    /// Value rejected (oversized, malformed-only, or untrusted context).
    Rejected,
}

/// One parsed alternative before filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedAlternative {
    protocol: String,
    authority: String,
    ma: Option<u64>,
}

/// Parse one Alt-Svc header value into raw alternatives.
///
/// Never panics; bounds output at [`ALTSVC_MAX_ALTERNATIVES`]. Malformed
/// entries are skipped, not fatal. `clear` is handled by the caller via
/// [`is_clear_value`].
fn parse_alternatives(value: &str) -> Vec<ParsedAlternative> {
    let mut out = Vec::new();
    for part in split_header_list(value) {
        if out.len() >= ALTSVC_MAX_ALTERNATIVES {
            break;
        }
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Skip bare `clear` here; caller checks for clear-only values.
        if part.eq_ignore_ascii_case("clear") {
            continue;
        }
        // protocol-id "=" alt-authority *( ";" param )
        // Find first '=' separating protocol from authority.
        let Some(eq) = part.find('=') else {
            continue;
        };
        let (proto, rest) = part.split_at(eq);
        let proto = proto.trim();
        if proto.is_empty() || !is_token(proto) {
            continue;
        }
        let rest = rest[1..].trim_start();
        // Authority is a quoted-string.
        let Some((authority, params)) = parse_quoted_string(rest) else {
            continue;
        };
        let mut ma: Option<u64> = None;
        let mut ma_present = false;
        let mut ma_invalid = false;
        // Params are `; name=value` or `; name` (e.g. persist).
        for param in params.split(';').skip(1) {
            let param = param.trim();
            if param.is_empty() {
                continue;
            }
            let (name, pvalue) = match param.find('=') {
                Some(i) => (param[..i].trim(), Some(param[i + 1..].trim())),
                None => (param, None),
            };
            if name.eq_ignore_ascii_case("ma") {
                ma_present = true;
                if let Some(v) = pvalue {
                    // Strip optional quotes around the number.
                    let v = v.trim().trim_matches('"');
                    // Overflow or non-numeric fails closed for this entry.
                    if let Ok(n) = v.parse::<u64>() {
                        ma = Some(n);
                    } else {
                        ma_invalid = true;
                        break;
                    }
                } else {
                    ma_invalid = true;
                    break;
                }
            }
            // `persist` and unknown params are accepted and ignored.
        }
        // Unparseable `ma` fails closed for this entry.
        if ma_invalid {
            continue;
        }
        // `ma=` present with empty value leaves `ma=None`; skip as invalid.
        // Absent `ma` leaves `ma_present=false` and uses the default.
        // `ma=0` also yields `Some(0)`. So: if ma_present and ma is None,
        // the value was invalid → skip.
        if ma_present && ma.is_none() {
            continue;
        }
        out.push(ParsedAlternative {
            protocol: proto.to_owned(),
            authority,
            ma,
        });
    }
    out
}

/// Returns `true` when the combined value is a clear-only advertisement
/// (`clear`, optionally quoted/whitespace-padded).
fn is_clear_value(value: &str) -> bool {
    let v = value.trim().trim_matches('"').trim();
    v.eq_ignore_ascii_case("clear")
}

/// Split a header list on commas that are not inside quotes.
fn split_header_list(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, b) in value.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match b {
            b'\\' if in_quotes => escaped = true,
            b'"' => in_quotes = !in_quotes,
            b',' if !in_quotes => {
                parts.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

/// Parse a `"`-quoted string at the start of `s`.
///
/// Returns `(unescaped_value, remainder_after_closing_quote)`.
/// Handles `\"` and `\\` escapes; rejects unterminated quotes and
/// control bytes (fail closed).
fn parse_quoted_string(s: &str) -> Option<(String, &str)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return None;
                }
                let c = bytes[i];
                // Only allow escaping of `"` and `\` per RFC; reject others
                // to fail closed on hostile input.
                if c != b'"' && c != b'\\' {
                    return None;
                }
                out.push(c as char);
                i += 1;
            }
            b'"' => {
                return Some((out, &s[i + 1..]));
            }
            c if c < 0x20 || c == 0x7f => return None,
            c => {
                out.push(c as char);
                i += 1;
            }
        }
        // Bound authority length to avoid unbounded allocation.
        if out.len() > 512 {
            return None;
        }
    }
    None
}

/// RFC 7230 token check.
fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b > 0x20
                && b < 0x7f
                && !matches!(
                    b,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                        | b'{'
                        | b'}'
                        | b' '
                        | b'\t'
                )
        })
}

/// Parse an Alt-Svc authority into `(host_or_empty, port)`.
///
/// Accepts `host:port`, `:port` (same host), and bracketed IPv6
/// `[::1]:443`. Rejects userinfo (`@`), missing ports, bad ports, and
/// overlong hosts. Returns `None` for malformed authorities.
fn parse_authority(authority: &str) -> Option<(String, u16)> {
    if authority.is_empty() || authority.len() > 512 {
        return None;
    }
    if authority.contains('@') {
        return None;
    }
    // Bracketed IPv6: [addr]:port
    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        let after = &rest[end + 1..];
        let port_str = after.strip_prefix(':')?;
        if host.is_empty() || port_str.is_empty() {
            return None;
        }
        let port: u16 = port_str.parse().ok()?;
        if port == 0 {
            return None;
        }
        return Some((host.to_ascii_lowercase(), port));
    }
    // General case: split on last ':'.
    let colon = authority.rfind(':')?;
    let (host, port_str) = authority.split_at(colon);
    let port_str = &port_str[1..];
    if port_str.is_empty() {
        return None;
    }
    let port: u16 = port_str.parse().ok()?;
    if port == 0 {
        return None;
    }
    if host.is_empty() {
        // `:port` → same host.
        return Some((String::new(), port));
    }
    if host.contains([' ', '\t', '\r', '\n', '/', '?', '#']) {
        return None;
    }
    Some((host.to_ascii_lowercase(), port))
}

/// Bounded Alt-Svc cache owned by the client.
pub struct AltSvcCache {
    entries: DashMap<AltSvcOrigin, AltSvcEntry>,
    sequence: AtomicU64,
}

impl Default for AltSvcCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AltSvcCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
            sequence: AtomicU64::new(1),
        }
    }

    /// Number of cached origins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` when `origin` has any entry (fresh or expired).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn contains_origin(&self, origin: &AltSvcOrigin) -> bool {
        self.entries.contains_key(origin)
    }

    /// Get the fresh alternative for `origin`, if any.
    ///
    /// Expired entries are removed lazily. Returns `None` when absent,
    /// expired (after removal), or when `origin` is unusable. No panics.
    #[must_use]
    pub fn get_fresh(&self, origin: &AltSvcOrigin, now: Instant) -> Option<AltSvcEntry> {
        let entry = self.entries.get(origin)?.clone();
        if entry.is_expired(now) {
            drop(entry);
            self.entries.remove(origin);
            return None;
        }
        self.entries.get(origin).map(|e| e.clone())
    }

    /// Get fresh with expiry metering.
    ///
    /// Like [`Self::get_fresh`] but reports whether an expired entry was
    /// removed so the caller can count `altsvc_expired` exactly once.
    pub fn get_fresh_counted(
        &self,
        origin: &AltSvcOrigin,
        now: Instant,
    ) -> (Option<AltSvcEntry>, bool) {
        let Some(entry) = self.entries.get(origin).map(|e| e.clone()) else {
            return (None, false);
        };
        if entry.is_expired(now) {
            drop(entry);
            self.entries.remove(origin);
            return (None, true);
        }
        (self.entries.get(origin).map(|e| e.clone()), false)
    }

    /// Remove the entry for `origin`. Returns `true` when one existed.
    pub fn clear(&self, origin: &AltSvcOrigin) -> bool {
        self.entries.remove(origin).is_some()
    }

    /// Learn from combined Alt-Svc header values for `origin`.
    ///
    /// `values` are the raw `alt-svc` header values in wire order. An empty
    /// slice means “absent”. Trust must already be checked by the caller
    /// via [`AltSvcLearnContext`]; this function only parses and bounds.
    /// Returns the outcome for metrics. Never stores credentials.
    pub fn learn(
        &self,
        origin: &AltSvcOrigin,
        values: &[String],
        now: Instant,
    ) -> AltSvcLearnOutcome {
        self.learn_counted(origin, values, now).0
    }

    /// Learn variant that reports whether a previous entry was replaced.
    ///
    /// Used by the pipeline to count `cleared` exactly (only when an entry
    /// actually existed).
    pub fn learn_counted(
        &self,
        origin: &AltSvcOrigin,
        values: &[String],
        now: Instant,
    ) -> (AltSvcLearnOutcome, bool) {
        if values.is_empty() {
            return (AltSvcLearnOutcome::Absent, false);
        }
        let combined = values.join(", ");
        if combined.len() > ALTSVC_MAX_HEADER_BYTES {
            return (AltSvcLearnOutcome::Rejected, false);
        }
        if is_clear_value(combined.trim()) {
            let had = self.clear(origin);
            return (AltSvcLearnOutcome::Cleared, had);
        }
        let parsed = parse_alternatives(&combined);
        let mut best: Option<(String, u16, Instant)> = None;
        for alt in parsed {
            if !alt.protocol.eq_ignore_ascii_case("h3") {
                continue;
            }
            let Some((host, port)) = parse_authority(&alt.authority) else {
                continue;
            };
            let ma_secs = alt.ma.unwrap_or(ALTSVC_DEFAULT_MA_SECS);
            if ma_secs == 0 {
                continue;
            }
            let Some(expires_at) = now.checked_add(Duration::from_secs(ma_secs)) else {
                continue;
            };
            let resolved_host = if host.is_empty() {
                origin.host().to_owned()
            } else {
                host
            };
            if best.is_none() {
                best = Some((resolved_host, port, expires_at));
            }
        }
        let Some((alt_host, alt_port, expires_at)) = best else {
            let had = self.clear(origin);
            if had {
                return (AltSvcLearnOutcome::Cleared, true);
            }
            return (AltSvcLearnOutcome::Absent, false);
        };
        // Same-endpoint re-advertisement must not bump generation or clear
        // suppression: the route is still the same broken alternative.
        // Only a changed authority/port creates a new generation that
        // re-enables the route without waiting on stale failure state.
        if let Some(existing) = self.entries.get(origin).map(|e| e.clone()) {
            if existing.alt_host == alt_host && existing.alt_port == alt_port {
                // Refresh expiry without changing generation; preserve
                // suppression (do not call `note_new_advertisement`).
                // Update expiry only if later (monotonic, never shorten via
                // re-advertisement race).
                let new_expiry = expires_at.max(existing.expires_at);
                // Only write when expiry actually moves to avoid churn.
                if new_expiry != existing.expires_at {
                    self.entries.insert(
                        origin.clone(),
                        AltSvcEntry {
                            alt_host,
                            alt_port,
                            expires_at: new_expiry,
                            generation: existing.generation,
                        },
                    );
                }
                return (AltSvcLearnOutcome::Learned, false);
            }
        }
        self.ensure_bound(origin);
        let generation = self.sequence.fetch_add(1, Ordering::Relaxed);
        self.entries.insert(
            origin.clone(),
            AltSvcEntry {
                alt_host,
                alt_port,
                expires_at,
                generation,
            },
        );
        (AltSvcLearnOutcome::Learned, false)
    }

    /// Test-only direct insert (explicit test injection bypassing trust).
    #[cfg(any(test, feature = "test-util"))]
    pub fn insert_for_test(&self, origin: AltSvcOrigin, entry: AltSvcEntry) {
        self.ensure_bound(&origin);
        self.entries.insert(origin, entry);
    }

    /// Ensure boundedness without evicting `except`.
    fn ensure_bound(&self, except: &AltSvcOrigin) {
        if self.entries.len() < ALTSVC_CACHE_MAX_ENTRIES {
            return;
        }
        let victim = self.entries.iter().find_map(|e| {
            let k = e.key().clone();
            (k != *except).then_some(k)
        });
        if let Some(victim) = victim {
            self.entries.remove(&victim);
        }
    }
}

impl std::fmt::Debug for AltSvcCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log authorities in full debug builds that might be scraped;
        // report only the count (no secrets are stored anyway).
        f.debug_struct("AltSvcCache")
            .field("len", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// Failure class for broken-route suppression.
///
/// Derived from [`crate::error::Error`] variants, never from error strings,
/// so the taxonomy cannot become fragile string matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3FailureClass {
    /// TCP/DNS/QUIC-handshake/connect-timeout (UDP reachability).
    Connectivity,
    /// H3 protocol/stream errors.
    Protocol,
    /// Peer closed the connection.
    Closed,
    /// Anything else (never suppresses longer than base).
    Other,
}

impl H3FailureClass {
    /// Classify without inspecting message text.
    ///
    /// Only connect-phase timeouts count as connectivity (UDP reachability);
    /// write/read/total timeouts are request-scoped and must not trigger
    /// broken-route suppression or safe fallback.
    #[must_use]
    pub fn from_error(err: &crate::error::Error) -> Self {
        match err {
            crate::error::Error::Connect(_) | crate::error::Error::H3Connect(_) => {
                Self::Connectivity
            }
            crate::error::Error::Timeout {
                phase: crate::timeout::TimeoutPhase::Connect,
                ..
            } => Self::Connectivity,
            crate::error::Error::H3Protocol(_) | crate::error::Error::H3Stream(_) => Self::Protocol,
            crate::error::Error::H3ConnectionClosed(_) => Self::Closed,
            // Write/read/total timeouts and all other errors are
            // request-scoped (`Other`): never suppress, never fallback.
            _ => Self::Other,
        }
    }
}

/// Bounded per-origin broken-route suppression.
pub struct BrokenRouteSuppressor {
    entries: DashMap<AltSvcOrigin, SuppressionEntry>,
}

#[derive(Debug, Clone)]
struct SuppressionEntry {
    generation: u64,
    failures: u32,
    suppressed_until: Instant,
    #[allow(
        dead_code,
        reason = "retained for diagnostics; backoff is uniform across classes"
    )]
    reason: H3FailureClass,
}

impl Default for BrokenRouteSuppressor {
    fn default() -> Self {
        Self::new()
    }
}

impl BrokenRouteSuppressor {
    /// Create an empty suppressor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Returns `true` when `origin@generation` is suppressed at `now`.
    ///
    /// A generation change re-enables the route without waiting: stale
    /// failure state never blocks a fresh advertisement.
    #[must_use]
    pub fn is_suppressed(&self, origin: &AltSvcOrigin, generation: u64, now: Instant) -> bool {
        let Some(entry) = self.entries.get(origin).map(|e| e.clone()) else {
            return false;
        };
        if entry.generation != generation {
            return false;
        }
        now < entry.suppressed_until
    }

    /// Record a failure; starts or extends backoff.
    ///
    /// `now` is caller-supplied for deterministic tests. Backoff is
    /// `min(BASE * 2^(failures-1), MAX)`. A generation change resets the
    /// count so a new advertisement is immediately eligible.
    pub fn record_failure(
        &self,
        origin: &AltSvcOrigin,
        generation: u64,
        now: Instant,
        reason: H3FailureClass,
    ) {
        // Fast path: new generation resets.
        if let Some(mut entry) = self.entries.get_mut(origin) {
            if entry.generation != generation {
                entry.generation = generation;
                entry.failures = 1;
                entry.reason = reason;
                entry.suppressed_until = now + SUPPRESSION_BASE.min(SUPPRESSION_MAX);
                return;
            }
            let next = entry.failures.saturating_add(1);
            entry.failures = next;
            entry.reason = reason;
            let shift = next.saturating_sub(1).min(10);
            let backoff = SUPPRESSION_BASE
                .checked_mul(1u32 << shift)
                .unwrap_or(SUPPRESSION_MAX)
                .min(SUPPRESSION_MAX);
            entry.suppressed_until = now + backoff;
            return;
        }
        self.ensure_bound(origin);
        self.entries.insert(
            origin.clone(),
            SuppressionEntry {
                generation,
                failures: 1,
                suppressed_until: now + SUPPRESSION_BASE.min(SUPPRESSION_MAX),
                reason,
            },
        );
    }

    /// Record success; clears suppression for `origin`.
    pub fn record_success(&self, origin: &AltSvcOrigin) {
        self.entries.remove(origin);
    }

    /// A new advertisement generation clears stale suppression.
    pub fn note_new_advertisement(&self, origin: &AltSvcOrigin, generation: u64) {
        if let Some(entry) = self.entries.get(origin).map(|e| e.clone()) {
            if entry.generation != generation {
                self.entries.remove(origin);
            }
        }
    }

    /// Number of suppressed origins (for tests).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when empty (for tests).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn ensure_bound(&self, except: &AltSvcOrigin) {
        if self.entries.len() < SUPPRESSION_MAX_ENTRIES {
            return;
        }
        let victim = self.entries.iter().find_map(|e| {
            let k = e.key().clone();
            (k != *except).then_some(k)
        });
        if let Some(victim) = victim {
            self.entries.remove(&victim);
        }
    }
}

impl std::fmt::Debug for BrokenRouteSuppressor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokenRouteSuppressor")
            .field("len", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// Shared Alt-Svc state owned by the client (separate from QUIC sessions).
#[derive(Debug, Default)]
pub struct AltSvcState {
    cache: AltSvcCache,
    suppressor: BrokenRouteSuppressor,
}

impl AltSvcState {
    /// Create empty shared state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: AltSvcCache::new(),
            suppressor: BrokenRouteSuppressor::new(),
        }
    }

    /// Borrow the cache.
    #[must_use]
    pub fn cache(&self) -> &AltSvcCache {
        &self.cache
    }

    /// Borrow the suppressor.
    #[must_use]
    pub fn suppressor(&self) -> &BrokenRouteSuppressor {
        &self.suppressor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> AltSvcOrigin {
        AltSvcOrigin::new("https", "example.com", 443).expect("origin")
    }

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn origin_normalizes_case_and_rejects_bad() {
        let o = AltSvcOrigin::new("HTTPS", "Example.COM", 443).expect("origin");
        assert_eq!(o.host(), "example.com");
        assert!(AltSvcOrigin::new("https", "", 443).is_none());
        assert!(AltSvcOrigin::new("https", "example.com", 0).is_none());
        assert!(AltSvcOrigin::new("https", "user@example.com", 443).is_none());
    }

    #[test]
    fn parses_basic_h3_with_ma() {
        let alts = parse_alternatives(r#"h3=":443"; ma=86400"#);
        assert_eq!(alts.len(), 1);
        assert_eq!(alts[0].protocol, "h3");
        assert_eq!(alts[0].authority, ":443");
        assert_eq!(alts[0].ma, Some(86_400));
    }

    #[test]
    fn ignores_unsupported_protocols() {
        let cache = AltSvcCache::new();
        let o = origin();
        let (outcome, had) = cache.learn_counted(&o, &[r#"h2=":443"; ma=60"#.to_owned()], now());
        assert_eq!(outcome, AltSvcLearnOutcome::Absent);
        assert!(!had);
        assert!(cache.get_fresh(&o, now()).is_none());
    }

    #[test]
    fn learns_supported_h3_and_resolves_same_host() {
        let cache = AltSvcCache::new();
        let o = origin();
        let outcome = cache.learn(&o, &[r#"h3=":8443"; ma=60"#.to_owned()], now());
        assert_eq!(outcome, AltSvcLearnOutcome::Learned);
        let entry = cache.get_fresh(&o, now()).expect("fresh");
        assert_eq!(entry.alt_host, "example.com");
        assert_eq!(entry.alt_port, 8443);
    }

    #[test]
    fn learns_alternative_authority() {
        let cache = AltSvcCache::new();
        let o = origin();
        let outcome = cache.learn(
            &o,
            &[r#"h3="alt.example.com:443"; ma=60"#.to_owned()],
            now(),
        );
        assert_eq!(outcome, AltSvcLearnOutcome::Learned);
        let entry = cache.get_fresh(&o, now()).expect("fresh");
        assert_eq!(entry.alt_host, "alt.example.com");
    }

    #[test]
    fn clear_removes_entry() {
        let cache = AltSvcCache::new();
        let o = origin();
        assert_eq!(
            cache.learn(&o, &[r#"h3=":443"; ma=60"#.to_owned()], now()),
            AltSvcLearnOutcome::Learned
        );
        assert_eq!(
            cache.learn(&o, &["clear".to_owned()], now()),
            AltSvcLearnOutcome::Cleared
        );
        assert!(cache.get_fresh(&o, now()).is_none());
    }

    #[test]
    fn zero_ma_never_caches() {
        let cache = AltSvcCache::new();
        let o = origin();
        let (outcome, had) = cache.learn_counted(&o, &[r#"h3=":443"; ma=0"#.to_owned()], now());
        assert_eq!(outcome, AltSvcLearnOutcome::Absent);
        assert!(!had);
        assert!(cache.get_fresh(&o, now()).is_none());
    }

    #[test]
    fn malformed_values_fail_closed() {
        let cache = AltSvcCache::new();
        let o = origin();
        for bad in [
            r#"h3=":443"#.to_owned(),                              // unterminated quote
            r"h3=:443; ma=60".to_owned(),                          // unquoted authority
            r#"h3="user@:443"; ma=60"#.to_owned(),                 // userinfo injection
            r#"h3=":0"; ma=60"#.to_owned(),                        // bad port
            r#"h3=":443"; ma=abc"#.to_owned(),                     // bad ma
            r#"h3=":443"; ma=99999999999999999999999"#.to_owned(), // overflow
        ] {
            let outcome = cache.learn(&o, &[bad], now());
            assert!(
                matches!(
                    outcome,
                    AltSvcLearnOutcome::Cleared | AltSvcLearnOutcome::Absent
                ),
                "bad value must not learn: {outcome:?}"
            );
            assert!(cache.get_fresh(&o, now()).is_none());
        }
    }

    #[test]
    fn oversized_header_rejected_without_allocation() {
        let cache = AltSvcCache::new();
        let o = origin();
        let big = format!(
            "h3=\":443\"; ma=60, {}",
            "x".repeat(ALTSVC_MAX_HEADER_BYTES + 1)
        );
        assert_eq!(cache.learn(&o, &[big], now()), AltSvcLearnOutcome::Rejected);
    }

    #[test]
    fn expiry_is_lazy() {
        let cache = AltSvcCache::new();
        let o = origin();
        assert_eq!(
            cache.learn(&o, &[r#"h3=":443"; ma=60"#.to_owned()], now()),
            AltSvcLearnOutcome::Learned
        );
        let future = now() + Duration::from_secs(61);
        assert!(cache.get_fresh(&o, future).is_none());
        assert!(!cache.contains_origin(&o));
    }

    #[test]
    fn cache_stays_bounded() {
        let cache = AltSvcCache::new();
        for i in 0..(ALTSVC_CACHE_MAX_ENTRIES + 10) {
            let o = AltSvcOrigin::new("https", &format!("host-{i}.example"), 443).expect("origin");
            let _ = cache.learn(&o, &[r#"h3=":443"; ma=60"#.to_owned()], now());
        }
        assert!(cache.len() <= ALTSVC_CACHE_MAX_ENTRIES);
    }

    #[test]
    fn duplicate_alternatives_first_wins_deterministically() {
        let cache = AltSvcCache::new();
        let o = origin();
        let _ = cache.learn(
            &o,
            &[r#"h3="a.example:443"; ma=60, h3="b.example:443"; ma=60"#.to_owned()],
            now(),
        );
        let entry = cache.get_fresh(&o, now()).expect("fresh");
        assert_eq!(entry.alt_host, "a.example");
    }

    #[test]
    fn suppression_backoff_and_expiry() {
        let s = BrokenRouteSuppressor::new();
        let o = origin();
        let t0 = now();
        assert!(!s.is_suppressed(&o, 1, t0));
        s.record_failure(&o, 1, t0, H3FailureClass::Connectivity);
        assert!(s.is_suppressed(&o, 1, t0));
        assert!(!s.is_suppressed(&o, 1, t0 + SUPPRESSION_BASE + Duration::from_secs(1)));
        // Second failure extends backoff.
        s.record_failure(&o, 1, t0, H3FailureClass::Connectivity);
        assert!(s.is_suppressed(&o, 1, t0 + SUPPRESSION_BASE + Duration::from_secs(1)));
        // Success clears.
        s.record_success(&o);
        assert!(!s.is_suppressed(&o, 1, t0));
    }

    #[test]
    fn new_generation_reenables_without_waiting() {
        let s = BrokenRouteSuppressor::new();
        let o = origin();
        let t0 = now();
        s.record_failure(&o, 1, t0, H3FailureClass::Connectivity);
        assert!(s.is_suppressed(&o, 1, t0));
        // New advertisement generation is immediately eligible.
        assert!(!s.is_suppressed(&o, 2, t0));
        s.note_new_advertisement(&o, 2);
        assert!(!s.is_suppressed(&o, 2, t0));
    }

    #[test]
    fn failure_class_uses_variants_not_strings() {
        use crate::error::Error;
        assert_eq!(
            H3FailureClass::from_error(&Error::H3Connect("x".into())),
            H3FailureClass::Connectivity
        );
        assert_eq!(
            H3FailureClass::from_error(&Error::H3Protocol("x".into())),
            H3FailureClass::Protocol
        );
        assert_eq!(
            H3FailureClass::from_error(&Error::H3ConnectionClosed("x".into())),
            H3FailureClass::Closed
        );
    }

    #[test]
    fn learn_context_trust_rules() {
        let good = AltSvcLearnContext {
            is_https: true,
            tls_authenticated: true,
            via_proxy: false,
            origin_matches_hop: true,
        };
        assert!(good.trustworthy());
        for bad in [
            AltSvcLearnContext {
                is_https: false,
                ..good
            },
            AltSvcLearnContext {
                tls_authenticated: false,
                ..good
            },
            AltSvcLearnContext {
                via_proxy: true,
                ..good
            },
            AltSvcLearnContext {
                origin_matches_hop: false,
                ..good
            },
        ] {
            assert!(!bad.trustworthy());
        }
    }
}
