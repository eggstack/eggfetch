#![allow(missing_docs, dead_code, unused_mut, clippy::all)]
//! Timeout integration tests for eggfetch-core.

mod test_server;

use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, Error, RequestBody, Timeout, TimeoutPhase};
use futures_util::StreamExt;
use test_server::{TestServer, TestServerConfig};

/// Pool timeout fires when `max_connections=1` saturates and the wait exceeds
/// the configured pool timeout.
#[tokio::test]
async fn test_pool_timeout_with_saturated_pool() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 300,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .max_connections(1)
        .timeout(Timeout {
            pool: Some(Duration::from_millis(200)),
            ..Timeout::default()
        })
        .build();

    let client2 = client.clone();
    let url2 = url.clone();

    // First request holds the only slot.
    let h1 = tokio::spawn(async move { client.get(&url).unwrap().send().await });

    // Give the first request time to acquire the slot.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second request should fail with pool timeout.
    let h2 = tokio::spawn(async move { client2.get(&url2).unwrap().send().await });

    let r2 = h2.await.unwrap();
    assert!(
        matches!(
            r2,
            Err(Error::Timeout {
                phase: TimeoutPhase::Pool,
                ..
            })
        ),
        "expected pool timeout, got: {r2:?}"
    );

    // First request may still be in flight; wait for it to complete.
    let _ = h1.await;

    server.shutdown();
}

/// Connect timeout fires when connecting to an unroutable IP.
#[tokio::test]
async fn test_connect_timeout() {
    // 192.0.2.1 is TEST-NET-1 (RFC 5737) — unroutable on any real network.
    let client = Client::builder()
        .timeout(Timeout {
            connect: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    let result = client.get("http://192.0.2.1:80/").unwrap().send().await;

    assert!(
        result.is_err(),
        "expected error connecting to unroutable IP"
    );
    // The error could be a timeout, connect error, or hyper error depending
    // on OS behavior with TEST-NET addresses.
    match &result {
        Err(
            Error::Timeout { .. }
            | Error::Connect(_)
            | Error::HyperClient(_)
            | Error::Hyper(_)
            | Error::Io(_),
        ) => { /* acceptable */ }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// Total timeout fires when the request takes longer than the total deadline.
#[tokio::test]
async fn test_total_timeout() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    let result = client.get(&url).unwrap().send().await;

    assert!(
        matches!(
            result,
            Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            })
        ),
        "expected total timeout, got: {result:?}"
    );

    server.shutdown();
}

/// Fast requests succeed even with generous timeouts.
#[tokio::test]
async fn test_no_timeout_when_fast_enough() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 0,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().timeout(Timeout::from_secs(5)).build();

    let result = client.get(&url).unwrap().send().await;
    assert!(
        result.is_ok(),
        "expected success for fast request, got: {result:?}"
    );

    server.shutdown();
}

/// Request-level timeout overrides client-level timeout.
#[tokio::test]
async fn test_request_level_timeout_overrides_client() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 300,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().timeout(Timeout::from_secs(5)).build();

    // Request-level total timeout overrides the generous client timeout.
    let result = client
        .get(&url)
        .unwrap()
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .send()
        .await;

    assert!(
        matches!(
            result,
            Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            })
        ),
        "expected total timeout from request-level override, got: {result:?}"
    );

    server.shutdown();
}

/// Disabled timeouts (all None) allow slow requests to succeed.
#[tokio::test]
async fn test_disabled_timeout_does_not_fail() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder().timeout(Timeout::disabled()).build();

    let result = client.get(&url).unwrap().send().await;
    assert!(
        result.is_ok(),
        "expected success with disabled timeout, got: {result:?}"
    );

    server.shutdown();
}

/// Pool timeout errors report `TimeoutPhase::Pool`; total timeout errors
/// report `TimeoutPhase::Total`.
#[tokio::test]
async fn test_timeout_error_reports_correct_phase() {
    // --- Pool phase ---
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 300,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .max_connections(1)
        .timeout(Timeout {
            pool: Some(Duration::from_millis(200)),
            ..Timeout::default()
        })
        .build();

    let client2 = client.clone();
    let url2 = url.clone();

    let h1 = tokio::spawn(async move {
        let _ = client.get(&url).unwrap().send().await;
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let h2 = tokio::spawn(async move { client2.get(&url2).unwrap().send().await });

    let pool_err = h2.await.unwrap();
    assert!(
        matches!(
            pool_err,
            Err(Error::Timeout {
                phase: TimeoutPhase::Pool,
                ..
            })
        ),
        "expected TimeoutPhase::Pool, got: {pool_err:?}"
    );

    let _ = h1.await;
    server.shutdown();

    // --- Total phase ---
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    let total_err = client.get(&url).unwrap().send().await;
    assert!(
        matches!(
            total_err,
            Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            })
        ),
        "expected TimeoutPhase::Total, got: {total_err:?}"
    );

    server.shutdown();
}

/// Client builder accepts timeout configuration and compiles.
#[test]
fn test_client_builder_timeout_config() {
    let client = Client::builder().timeout(Timeout::from_secs(5)).build();

    let req = client.get("https://example.com").unwrap().build().unwrap();
    assert_eq!(*req.method(), http::Method::GET);
}

/// Request builder accepts timeout configuration and compiles.
#[test]
fn test_request_builder_timeout_config() {
    let client = Client::new();
    let req = client
        .get("https://example.com")
        .unwrap()
        .timeout(Timeout::from_secs(5))
        .build()
        .unwrap();

    assert!(req.timeout().is_some());
    let t = req.timeout().unwrap();
    assert_eq!(t.pool, Some(Duration::from_secs(5)));
    assert_eq!(t.connect, Some(Duration::from_secs(5)));
    assert_eq!(t.write, Some(Duration::from_secs(5)));
    assert_eq!(t.read, Some(Duration::from_secs(5)));
    assert!(t.total.is_none());
}

/// Cancelling a waiting acquisition does not cause deadlock. The third
/// request succeeds after the first completes.
#[tokio::test]
async fn test_pool_timeout_cancellation_safety() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .max_connections(1)
        .timeout(Timeout {
            pool: Some(Duration::from_millis(200)),
            ..Timeout::default()
        })
        .build();

    // First request holds the only slot.
    let h1 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    // Give first request time to acquire the slot.
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Second request starts waiting, then we cancel it by aborting.
    let h2 = {
        let client = client.clone();
        let url = url.clone();
        tokio::spawn(async move { client.get(&url).unwrap().send().await })
    };

    // Give the waiter time to start blocking on the semaphore.
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Cancel the waiter.
    h2.abort();
    tokio::time::sleep(Duration::from_millis(10)).await;

    // First request completes; consume and drop it so the streaming body
    // releases the pooled connection before we issue the next request.
    {
        let mut r1 = h1.await.unwrap().expect("first request should succeed");
        let _ = r1.bytes().await;
    }

    // Third request should succeed without deadlock.
    let resp = client.get(&url).unwrap().send().await;
    assert!(
        resp.is_ok(),
        "third request should succeed after cancellation"
    );

    server.shutdown();
}

/// Read timeout fires when a chunked response stalls between chunks.
#[tokio::test]
async fn test_read_timeout_on_chunked_response_stall() {
    // Server sends 2 chunks quickly, then stalls 500ms before the 3rd.
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"abcdefghij".to_vec()),
        chunk_stall_after: Some(2),
        chunk_stall_ms: 500,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .timeout(Timeout {
            read: Some(Duration::from_millis(150)),
            ..Timeout::default()
        })
        .build();

    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let mut stream = resp.bytes_stream().unwrap();

    // First two chunks arrive well within the read timeout.
    for _ in 0..2 {
        let next = tokio::time::timeout(Duration::from_millis(400), stream.next())
            .await
            .expect("early chunks should arrive within the read timeout")
            .expect("stream should yield chunk")
            .expect("chunk should be Ok");
        assert!(!next.is_empty(), "chunk should have data");
    }

    // The server now stalls before the next chunk; read timeout fires.
    let next = stream.next().await;
    match next {
        Some(Err(Error::Timeout {
            phase: TimeoutPhase::Read,
            ..
        })) => {}
        other => panic!("expected read timeout error, got: {other:?}"),
    }

    server.shutdown();
}

/// Write timeout fires when a streaming request body's producer stalls
/// between chunks.
#[tokio::test]
async fn test_write_timeout_on_request_body_stall() {
    let mut server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .timeout(Timeout {
            write: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    // Producer yields one chunk quickly, then stalls BEFORE yielding a
    // second chunk. The server reads the request body, so hyper must
    // drive the body stream — which gives the write timeout a chance
    // to fire.
    let producer = futures_util::stream::unfold(0u32, |i| async move {
        if i == 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Some((Ok(Bytes::from("chunk0")), 1))
        } else if i == 1 {
            // Stall past the write timeout.
            tokio::time::sleep(Duration::from_secs(2)).await;
            Some((Ok(Bytes::from("chunk1")), 2))
        } else {
            Some((Ok(Bytes::from("chunk2")), 3))
        }
    });
    let stream = Box::pin(producer);
    // With a known Content-Length (17 bytes), hyper MUST read that many
    // bytes from the body before the server will respond.
    let body = RequestBody::from_stream(stream, Some(17));

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        client.post(&url).unwrap().body(body).send(),
    )
    .await
    .expect("overall send should complete (likely with write timeout)");

    match result {
        Err(Error::Timeout {
            phase: TimeoutPhase::Write,
            ..
        }) => {}
        other => panic!("expected write timeout error, got: {other:?}"),
    }

    server.shutdown();
}
