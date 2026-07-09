//! Connection pool and concurrency lifecycle tests.

mod test_server;

use std::sync::atomic::Ordering;
use std::time::Duration;

use eggfetch_core::Client;
use test_server::{TestServer, TestServerConfig};

/// Sequential requests to the same host reuse connections.
#[tokio::test]
async fn test_reuse_connections() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder().build();

    for _ in 0..5 {
        let resp = client.get(&url).unwrap().send().await.unwrap();
        assert!(resp.is_success());
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let accepted = server.connections_accepted();

    // Hyper should reuse the connection; we should see at most 2 TCP
    // connections (one initial, one possibly during warm-up).
    assert!(
        accepted <= 2,
        "expected at most 2 TCP connections for sequential reuse, got {accepted}"
    );

    server.shutdown();
}

/// Connections are not reused when the server sends `Connection: close`.
#[tokio::test]
async fn test_connection_close_not_reused() {
    let mut server = TestServer::start(&TestServerConfig {
        close_connection: true,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().max_connections(8).build();

    for _ in 0..5 {
        let resp = client.get(&url).unwrap().send().await.unwrap();
        assert!(resp.is_success());
        tokio::task::yield_now().await;
    }

    // Each request should have opened a new TCP connection.
    let accepted = server.connections_accepted();
    assert!(
        accepted >= 5,
        "expected at least 5 TCP connections with Connection: close, got {accepted}"
    );

    server.shutdown();
}

/// Global `max_connections` limits concurrent in-flight requests.
///
/// Uses timing-based verification: with `max_connections=2` and a 200ms
/// server delay, 6 requests should take at least 400ms if the limit works.
#[tokio::test]
async fn test_max_connections_limits() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let url = server.url();

    let max_conn: usize = 2;
    let client = Client::builder().max_connections(max_conn).build();

    let start = std::time::Instant::now();

    // Fire 6 concurrent requests.
    let mut handles = Vec::new();
    for _ in 0..6 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).unwrap().send().await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let elapsed = start.elapsed();

    // With max_connections=2 and 200ms delay, 6 requests need 3 batches
    // of 2 = 600ms minimum. Allow generous margin for timing jitter.
    // Without the limit, all 6 would complete in ~200ms.
    let min_expected = Duration::from_millis(350);
    assert!(
        elapsed >= min_expected,
        "expected at least {min_expected:?} with max_connections={max_conn}, elapsed {elapsed:?}"
    );

    // Verify actual server-side concurrency didn't exceed the limit.
    let accepted = server.connections_accepted();
    assert!(
        accepted <= max_conn + 1,
        "expected at most {max_conn}+1 TCP connections, got {accepted}"
    );

    server.shutdown();
}

/// Per-host `max_connections` limits concurrent requests to one host.
#[tokio::test]
async fn test_max_connections_per_host() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let url = server.url();

    let max_per_host: usize = 2;
    let client = Client::builder()
        .max_connections(8)
        .max_connections_per_host(max_per_host)
        .build();

    let start = std::time::Instant::now();

    let mut handles = Vec::new();
    for _ in 0..6 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).unwrap().send().await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let elapsed = start.elapsed();

    // Same timing logic: max_per_host=2 means 3 batches of 2 = ~600ms.
    let min_expected = Duration::from_millis(350);
    assert!(
        elapsed >= min_expected,
        "expected at least {min_expected:?} with max_connections_per_host={max_per_host}, elapsed {elapsed:?}"
    );

    server.shutdown();
}

/// Pool metrics counters are updated after requests.
#[tokio::test]
async fn test_pool_metrics_tracking() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder().build();

    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());

    // At this point, the pool's acquisition_waits should be 0 (no limits
    // configured, so no waiting needed).
    let metrics = client.pool_metrics();
    assert_eq!(
        metrics.acquisition_waits.load(Ordering::Relaxed),
        0,
        "no waits expected without limits"
    );
    assert_eq!(
        metrics.acquisition_cancellations.load(Ordering::Relaxed),
        0,
        "no cancellations expected"
    );

    server.shutdown();
}

/// A cloned client shares the same pool as the original.
#[tokio::test]
async fn test_clone_shares_pool() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder().max_connections(4).build();

    let client2 = client.clone();

    // Both should work and share the pool.
    let r1 = client.get(&url).unwrap().send().await.unwrap();
    let r2 = client2.get(&url).unwrap().send().await.unwrap();
    assert!(r1.is_success());
    assert!(r2.is_success());
    tokio::task::yield_now().await;

    // Total connections should still be small (pool reuse).
    let accepted = server.connections_accepted();
    assert!(
        accepted <= 3,
        "cloned clients should share pool, expected <= 3 connections, got {accepted}"
    );

    server.shutdown();
}

/// Client builder methods configure pool correctly.
#[test]
fn test_client_builder_pool_config() {
    let _client = Client::builder()
        .max_connections(10)
        .max_connections_per_host(5)
        .max_idle_connections(20)
        .max_idle_connections_per_host(10)
        .idle_timeout(Duration::from_secs(30))
        .user_agent("test-agent")
        .build();
}

/// Idle timeout configuration is accepted by the builder.
#[test]
fn test_idle_timeout_config() {
    let _client = Client::builder()
        .idle_timeout(Duration::from_secs(60))
        .build();
}

/// Multiple sequential requests to the same server reuse connections.
#[tokio::test]
async fn test_connection_reuse_same_host() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder().build();

    // First request establishes the connection.
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());
    tokio::task::yield_now().await;

    let after_first = server.connections_accepted();
    assert!(
        after_first >= 1,
        "at least 1 connection after first request"
    );

    // Subsequent requests should reuse the connection.
    for _ in 0..5 {
        let resp = client.get(&url).unwrap().send().await.unwrap();
        assert!(resp.is_success());
        tokio::task::yield_now().await;
    }

    let after_all = server.connections_accepted();
    // Connections may increase by at most 1 during initial setup.
    assert!(
        after_all <= after_first + 1,
        "expected reuse: {after_first} -> {after_all}"
    );

    server.shutdown();
}

/// With `max_connections=1`, the second request must wait for the first.
#[tokio::test]
async fn test_acquisition_wait() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 150,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().max_connections(1).build();

    let start = std::time::Instant::now();

    let h1 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };
    let h2 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    let r1 = h1.await.unwrap().unwrap();
    let r2 = h2.await.unwrap().unwrap();
    assert!(r1.is_success());
    assert!(r2.is_success());

    // With max_connections=1 and a 150ms delay per request, two sequential
    // requests should take at least 250ms (accounting for timing jitter).
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(250),
        "expected serialization delay, elapsed {elapsed:?}"
    );

    // The pool should have recorded at least one acquisition wait.
    let metrics = client.pool_metrics();
    assert!(
        metrics.acquisition_waits.load(Ordering::SeqCst) >= 1,
        "expected at least 1 acquisition wait"
    );

    server.shutdown();
}

/// With `max_connections=2`, three concurrent requests show bounded concurrency.
#[tokio::test]
async fn test_max_connections_bounds_three() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 300,
        ..Default::default()
    });
    let url = server.url();

    let max_conn: usize = 2;
    let client = Client::builder().max_connections(max_conn).build();

    let start = std::time::Instant::now();

    let mut handles = Vec::new();
    for _ in 0..3 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).unwrap().send().await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let elapsed = start.elapsed();

    // With max_connections=2 and 300ms delay, 3 requests need 2 batches:
    // 2 concurrent + 1 sequential = ~600ms minimum.
    let min_expected = Duration::from_millis(450);
    assert!(
        elapsed >= min_expected,
        "expected at least {min_expected:?} with max_connections={max_conn}, elapsed {elapsed:?}"
    );

    server.shutdown();
}

/// Test that requests fail gracefully when the server is not running.
#[tokio::test]
async fn test_connection_refused() {
    let client = Client::builder().build();
    let result = client.get("http://127.0.0.1:1").unwrap().send().await;
    assert!(result.is_err());
}

/// Test sending a POST request with a body to the test server.
#[tokio::test]
async fn test_post_with_body() {
    let mut server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().build();
    let resp = client
        .post(&url)
        .unwrap()
        .header("Content-Type", "text/plain")
        .body("hello")
        .send()
        .await
        .unwrap();
    assert!(resp.is_success());

    server.shutdown();
}

/// Pool metrics track acquisition waits correctly.
#[tokio::test]
async fn test_pool_metrics_waits() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().max_connections(1).build();

    let start = std::time::Instant::now();

    // Fire two concurrent requests with max_connections=1.
    let h1 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };
    let h2 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    let r1 = h1.await.unwrap().unwrap();
    let r2 = h2.await.unwrap().unwrap();
    assert!(r1.is_success());
    assert!(r2.is_success());

    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(150),
        "expected serialization, elapsed {elapsed:?}"
    );

    let metrics = client.pool_metrics();
    assert!(
        metrics.acquisition_waits.load(Ordering::SeqCst) >= 1,
        "expected at least 1 acquisition wait"
    );

    server.shutdown();
}

/// Idle connections are closed after the configured idle timeout.
///
/// After the first request completes, the connection goes idle. If we wait
/// longer than `idle_timeout`, the pool should close it, and the next request
/// opens a fresh connection.
#[tokio::test]
async fn test_idle_timeout_closes_stale_connection() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder()
        .idle_timeout(Duration::from_millis(100))
        .build();

    // First request opens a connection.
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());
    assert_eq!(server.connections_accepted(), 1);

    // Wait for the idle timeout to expire.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Second request: the old connection should have been evicted by the pool,
    // so a new TCP connection is accepted.
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());
    assert!(
        server.connections_accepted() >= 2,
        "expected at least 2 connections after idle timeout, got {}",
        server.connections_accepted()
    );

    server.shutdown();
}

/// Cancelling a waiting acquisition is safe and releases the waiter cleanly.
///
/// With `max_connections=1`, the first request holds the slot. A second request
/// starts waiting. We cancel it, then drop the first. The third request
/// should succeed without deadlock.
#[tokio::test]
async fn test_cancelled_waiter_releases_cleanly() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 50,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().max_connections(1).build();

    // First request holds the only slot.
    let h1 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    // Give first request time to acquire the slot.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Second request starts waiting for the slot.
    let h2 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    // Give the waiter time to start blocking on the semaphore.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Cancel the waiter by aborting the task.
    h2.abort();
    // Wait for abort to propagate.
    tokio::time::sleep(Duration::from_millis(10)).await;

    // First request completes, releasing the slot.
    let r1 = h1.await.unwrap().unwrap();
    assert!(r1.is_success());

    // Third request should succeed — no deadlock from the cancelled waiter.
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());

    server.shutdown();
}
