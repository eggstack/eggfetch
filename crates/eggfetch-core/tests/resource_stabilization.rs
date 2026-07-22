#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_funcall,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
//! Repeated-failure resource stabilization tests.
//!
//! Verifies that after repeated connection failures (connection refused,
//! DNS failure, timeout), resource usage returns to a bounded steady state.

use std::time::Duration;

use eggfetch_core::{Client, Timeout};

/// Measure current RSS in bytes. Returns 0 on unsupported platforms.
fn current_rss_bytes() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|kb| kb * 1024)
                })
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux, use a heuristic based on client object counts.
        // Return 0 to skip the RSS check.
        0
    }
}

/// Repeated connection-refused errors should not leak resources.
///
/// Connects to a port that is not listening. The TCP SYN is sent but
/// the connection is refused immediately. After N iterations, resource
/// usage should be within a bounded envelope of the baseline.
#[tokio::test]
async fn test_repeated_connection_refused_stabilizes() {
    // Use a port that is not listening. We pick a high port that is
    // unlikely to be in use.
    let target = "http://127.0.0.1:19999/";

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(200)),
            ..Default::default()
        })
        .build();

    let iterations = 50;

    // Warm up: let the runtime stabilize.
    for _ in 0..5 {
        let _ = client.get(target).unwrap().send().await;
    }

    let rss_before = current_rss_bytes();

    for _ in 0..iterations {
        let result = client.get(target).unwrap().send().await;
        // Connection refused is expected.
        assert!(result.is_err());
    }

    let rss_after = current_rss_bytes();

    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        // Allow up to 2 MB of RSS growth after 50 failed requests.
        assert!(
            delta < 2 * 1024 * 1024,
            "RSS grew by {delta} bytes after {iterations} connection-refused errors \
             (before={rss_before}, after={rss_after})"
        );
    }
}

/// Repeated DNS failures should not leak resources.
///
/// Queries a non-existent hostname. The DNS lookup fails immediately.
/// After N iterations, resource usage should be bounded.
#[tokio::test]
async fn test_repeated_dns_failure_stabilizes() {
    let target = "http://this-host-does-not-exist-xyz.invalid/";

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(200)),
            ..Default::default()
        })
        .build();

    let iterations = 50;

    // Warm up.
    for _ in 0..5 {
        let _ = client.get(target).unwrap().send().await;
    }

    let rss_before = current_rss_bytes();

    for _ in 0..iterations {
        let result = client.get(target).unwrap().send().await;
        assert!(result.is_err());
    }

    let rss_after = current_rss_bytes();

    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        assert!(
            delta < 2 * 1024 * 1024,
            "RSS grew by {delta} bytes after {iterations} DNS failures \
             (before={rss_before}, after={rss_after})"
        );
    }
}

/// Repeated timeouts should not leak resources.
///
/// Connects to an unroutable IP (TEST-NET-1) which hangs until the
/// timeout fires. After N iterations, resource usage should be bounded.
#[tokio::test]
async fn test_repeated_timeout_stabilizes() {
    // 192.0.2.1 is TEST-NET-1 (RFC 5737) — unroutable.
    let target = "http://192.0.2.1:80/";

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Default::default()
        })
        .build();

    let iterations = 30;

    // Warm up.
    for _ in 0..3 {
        let _ = client.get(target).unwrap().send().await;
    }

    let rss_before = current_rss_bytes();

    for _ in 0..iterations {
        let result = client.get(target).unwrap().send().await;
        assert!(result.is_err());
    }

    let rss_after = current_rss_bytes();

    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        assert!(
            delta < 2 * 1024 * 1024,
            "RSS grew by {delta} bytes after {iterations} timeouts \
             (before={rss_before}, after={rss_after})"
        );
    }
}

/// Repeated client open/close cycles should not leak resources.
///
/// Creates and destroys clients in a loop. After N iterations,
/// resource usage should be bounded.
#[tokio::test]
async fn test_repeated_client_creation_stabilizes() {
    let iterations = 20;

    // Warm up: create and destroy one client.
    {
        let client = Client::builder().timeout(Timeout::from_secs(1)).build();
        drop(client);
    }

    let rss_before = current_rss_bytes();

    for _ in 0..iterations {
        let client = Client::builder().timeout(Timeout::from_secs(1)).build();
        drop(client);
    }

    let rss_after = current_rss_bytes();

    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        assert!(
            delta < 2 * 1024 * 1024,
            "RSS grew by {delta} bytes after {iterations} client creation/destruction cycles \
             (before={rss_before}, after={rss_after})"
        );
    }
}

/// Mixed failure types should stabilize.
///
/// Alternates between connection refused, DNS failure, and timeout.
/// After N iterations, resource usage should be bounded.
#[tokio::test]
async fn test_mixed_failures_stabilize() {
    let targets = [
        ("http://127.0.0.1:19999/", "connection_refused"),
        (
            "http://this-host-does-not-exist-xyz.invalid/",
            "dns_failure",
        ),
        ("http://192.0.2.1:80/", "timeout"),
    ];

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(150)),
            ..Default::default()
        })
        .build();

    let iterations = 30;

    // Warm up.
    for (target, _) in &targets {
        for _ in 0..2 {
            let _ = client.get(*target).unwrap().send().await;
        }
    }

    let rss_before = current_rss_bytes();

    for i in 0..iterations {
        let (target, _kind) = &targets[i % targets.len()];
        let result = client.get(*target).unwrap().send().await;
        assert!(result.is_err());
    }

    let rss_after = current_rss_bytes();

    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        assert!(
            delta < 2 * 1024 * 1024,
            "RSS grew by {delta} bytes after {iterations} mixed failures \
             (before={rss_before}, after={rss_after})"
        );
    }
}
