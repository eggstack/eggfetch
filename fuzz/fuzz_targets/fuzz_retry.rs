#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::{MethodPolicy, RetryPolicy, StatusPolicy};
use http::Method;
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let parts: Vec<&str> = input.split('\0').collect();

    // RetryPolicy::builder should never panic for any configuration.
    let max_attempts = parts
        .first()
        .and_then(|s| s.parse::<usize>().unwrap_or(1).checked_add(1))
        .unwrap_or(2);
    let backoff_factor = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.5);
    let max_delay_secs = parts.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(30);
    let initial_delay_ms = parts.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(500);

    let policy = RetryPolicy::builder()
        .max_attempts(max_attempts)
        .backoff_factor(backoff_factor)
        .max_delay(Duration::from_secs(max_delay_secs))
        .initial_delay(Duration::from_millis(initial_delay_ms))
        .build();

    let _ = policy.is_enabled();
    let _ = policy.max_attempts();
    let _ = policy.max_elapsed();
    let _ = policy.respect_retry_after();

    // BackoffPolicy::delay should never panic.
    let backoff = policy.backoff();
    for attempt in 0..=20 {
        let _ = backoff.delay(attempt);
    }
    let _ = backoff.max_delay();
    let _ = backoff.factor();
    let _ = backoff.initial_delay();

    // MethodPolicy tests.
    let methods = policy.method_policy();
    for method in [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::HEAD,
        Method::OPTIONS,
    ] {
        let _ = methods.is_retryable(&method);
    }

    // StatusPolicy tests.
    let statuses = policy.status_policy();
    for code in [200u16, 301, 400, 404, 408, 429, 500, 502, 503, 504, 999] {
        let _ = statuses.is_retryable(code);
    }

    // retry_after_delay should never panic.
    if let Some(retry_after_str) = parts.get(4) {
        let _ = policy.retry_after_delay(retry_after_str);
    }

    // MethodPolicy direct construction.
    let mut mp = MethodPolicy::new(vec![Method::GET, Method::HEAD]);
    mp.add_method(Method::POST);
    let _ = mp.is_retryable(&Method::GET);
    let _ = mp.methods();

    // StatusPolicy direct construction.
    let mut sp = StatusPolicy::new([429, 503]);
    sp.add_status(502);
    let _ = sp.is_retryable(429);
    let _ = sp.statuses();

    // is_error_retryable with various error types.
    let _ = RetryPolicy::is_error_retryable(&eggfetch_core::Error::Connect("test".into()));
    let _ = RetryPolicy::is_error_retryable(&eggfetch_core::Error::Io(
        std::sync::Arc::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "test",
        )),
    ));
    let _ = RetryPolicy::is_error_retryable(&eggfetch_core::Error::InvalidUrl("test".into()));
});
