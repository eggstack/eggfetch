#![allow(
    missing_docs,
    dead_code,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Total-deadline response-body lifecycle regressions.
//!
//! Planning baseline `60a6e2b3` was red for these cases: response headers
//! arrived before `Timeout.total` but body completion exceeded it, and the
//! body incorrectly finished (or waited beyond total) instead of reporting
//! `TimeoutPhase::Total`.
//!
//! Covers: immediate-headers/body-total, post-first-chunk stall with no
//! read timeout, continuous trickle within read but past total, delayed
//! first poll, read-vs-total precedence, buffered/streaming/text/raw/decoded
//! modes, trailers, pool-lease release, redirect remaining-budget, and the
//! native frame-preserving surface.

mod test_server;

use std::time::{Duration, Instant};

use bytes::Bytes;
use eggfetch_core::{Client, Error, Timeout, TimeoutPhase};
use futures_util::StreamExt;
use test_server::{TestServer, TestServerConfig};

fn total_client(total_ms: u64) -> Client {
    Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(total_ms)),
            ..Timeout::default()
        })
        .build()
}

fn assert_total(err: &Error) {
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "expected TimeoutPhase::Total, got: {err:?}"
    );
}

fn assert_read(err: &Error) {
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Read,
                ..
            }
        ),
        "expected TimeoutPhase::Read, got: {err:?}"
    );
}

// --- G1: immediate headers, body exceeds total (bytes) ---

#[tokio::test]
async fn total_body_deadline_bytes_reports_total() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ0123456789".to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(250)
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("headers arrive before total");
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

// --- G1: post-first-chunk stall with no read timeout ---

#[tokio::test]
async fn total_body_deadline_stall_without_read_reports_total() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"abcdefghijkl".to_vec()),
        chunk_stall_after: Some(1),
        chunk_stall_ms: 800,
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(300)
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("headers + first chunk before total");
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

// --- G1: continuous trickle within read but past total ---

#[tokio::test]
async fn total_body_deadline_trickle_within_read_still_reports_total() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ0123456789".to_vec()),
        chunk_delay_ms: 50,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .timeout(Timeout {
            read: Some(Duration::from_secs(5)),
            total: Some(Duration::from_millis(250)),
            ..Timeout::default()
        })
        .build();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("headers before total");
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

// --- G1: no total configured -> streaming unchanged ---

#[tokio::test]
async fn no_total_streaming_body_completes() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"hello world".to_vec()),
        chunk_delay_ms: 20,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .timeout(Timeout {
            read: Some(Duration::from_secs(5)),
            ..Timeout::default()
        })
        .build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"hello world");
    server.shutdown();
}

// --- G2: delayed first poll after total ---

#[tokio::test]
async fn delayed_first_poll_after_total_reports_total() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(b"ok".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(200)
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("headers before total");
    // Wait until the original total deadline has passed without polling.
    tokio::time::sleep(Duration::from_millis(400)).await;
    // The ready inner body must not extend the request.
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

#[tokio::test]
async fn delayed_first_stream_poll_after_total_reports_total() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(b"ok".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(200)
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("headers before total");
    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut stream = resp.bytes_stream().unwrap();
    let err = stream.next().await.unwrap().unwrap_err();
    assert_total(&err);
    // Fused: repeated polling does not repeatedly emit errors.
    assert!(stream.next().await.is_none());
    server.shutdown();
}

// --- G3: read-vs-total precedence ---

#[tokio::test]
async fn shorter_read_wins_over_total() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"abcdefghij".to_vec()),
        chunk_stall_after: Some(1),
        chunk_stall_ms: 600,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .timeout(Timeout {
            read: Some(Duration::from_millis(100)),
            total: Some(Duration::from_secs(5)),
            ..Timeout::default()
        })
        .build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert_read(&err);
    server.shutdown();
}

#[tokio::test]
async fn shorter_total_wins_over_read() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"abcdefghij".to_vec()),
        chunk_stall_after: Some(1),
        chunk_stall_ms: 600,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .timeout(Timeout {
            read: Some(Duration::from_secs(5)),
            total: Some(Duration::from_millis(200)),
            ..Timeout::default()
        })
        .build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

// --- G4: consumption modes share semantics ---

#[tokio::test]
async fn total_applies_to_bytes_stream_mode() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ".to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(250).get(&url).unwrap().send().await.unwrap();
    let mut stream = resp.bytes_stream().unwrap();
    let mut saw_total = false;
    while let Some(item) = stream.next().await {
        if let Err(e) = item {
            assert_total(&e);
            saw_total = true;
            break;
        }
    }
    assert!(saw_total, "stream must terminate with Total");
    server.shutdown();
}

#[tokio::test]
async fn total_applies_to_text_mode() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ".to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(250).get(&url).unwrap().send().await.unwrap();
    let err = resp.text().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

#[cfg(feature = "json")]
#[tokio::test]
async fn total_applies_to_json_mode() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(br#"{"k":"v","pad":"0123456789ABCDEF"}"#.to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let mut resp = total_client(250).get(&url).unwrap().send().await.unwrap();
    let err = resp.json::<serde_json::Value>().await.unwrap_err();
    assert_total(&err);
    server.shutdown();
}

// Compressed raw + decoded paths (all-features): headers arrive, body
// stalls past total. Stall-before-body is used because mid-body gzip
// trickling surfaces decoder errors before the outer absolute deadline in
// the current async-compression pipeline; stall proves the single outer
// total mechanism covers both raw and decoded final streams.

#[cfg(all(
    feature = "compression-gzip",
    feature = "high-level-url",
    any(feature = "transport-http1", feature = "transport-http2")
))]
mod compressed {
    use super::*;

    fn gzip_bytes(input: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    async fn start_gzip_stall_server(encoded: Vec<u8>, stall_ms: u64) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0u8; 4096];
                let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
                    encoded.len()
                );
                if socket.write_all(header.as_bytes()).await.is_err() {
                    return;
                }
                let _ = socket.flush().await;
                tokio::time::sleep(Duration::from_millis(stall_ms)).await;
                let _ = socket.write_all(&encoded).await;
                let _ = socket.flush().await;
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn total_applies_to_decoded_compressed_stream() {
        let payload = b"decoded body must respect total 0123456789abcdef";
        let encoded = gzip_bytes(payload);
        let url = start_gzip_stall_server(encoded, 800).await;
        let mut resp = super::total_client(250)
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap();
        let err = resp.bytes().await.unwrap_err();
        super::assert_total(&err);
    }

    #[tokio::test]
    async fn total_applies_to_raw_compressed_stream() {
        let payload = b"raw body must respect total 0123456789abcdef";
        let encoded = gzip_bytes(payload);
        let url = start_gzip_stall_server(encoded, 800).await;
        let mut resp = super::total_client(250)
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap();
        let mut stream = resp.raw_bytes_stream().unwrap();
        let mut saw_total = false;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                super::assert_total(&e);
                saw_total = true;
                break;
            }
        }
        assert!(saw_total, "raw stream must terminate with Total");
    }

    #[tokio::test]
    async fn total_applies_to_decoded_streaming_mode() {
        let payload = b"decoded trickle body 0123456789abcdef";
        let encoded = gzip_bytes(payload);
        let url = start_gzip_stall_server(encoded, 800).await;
        let mut resp = super::total_client(250)
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap();
        let mut stream = resp.bytes_stream().unwrap();
        let mut saw_total = false;
        while let Some(item) = stream.next().await {
            if let Err(e) = item {
                super::assert_total(&e);
                saw_total = true;
                break;
            }
        }
        assert!(saw_total, "decoded stream must terminate with Total");
    }
}

// --- G5: trailers cannot outlive total ---

#[tokio::test]
async fn trailer_arrival_delayed_past_total_reports_total() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n5\r\nhello\r\n",
                )
                .await;
            let _ = socket.flush().await;
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    let url = format!("http://{addr}/");
    let mut resp = total_client(250).get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert_total(&err);
    assert!(
        resp.trailers().is_none(),
        "trailers unavailable when EOF not reached"
    );
}

// --- G6: total-timeout releases the logical pool lease ---

#[tokio::test]
async fn total_timeout_releases_pool_lease() {
    // Slow trickle body: ~700ms total. First body has a 250ms total and
    // must fail with Total, releasing its permit while a second same-origin
    // request waits on the pool (max 1). Without Total-on-body, the first
    // stream would hold the permit indefinitely and the second would stall.
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ".to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .max_in_flight_requests(1)
        .timeout(Timeout {
            total: Some(Duration::from_millis(300)),
            ..Timeout::default()
        })
        .build();

    let mut first = client.get(&url).unwrap().send().await.unwrap();
    let mut first_stream = first.bytes_stream().unwrap();
    // First chunk arrives quickly.
    let _ = first_stream.next().await.unwrap().unwrap();

    // Second request starts while the first body is still streaming; it
    // must block on the pool until the first body's Total releases it.
    let client2 = client.clone();
    let url2 = url.clone();
    let second = tokio::spawn(async move {
        // Generous total so the slow body can complete once admitted.
        client2
            .get(&url2)
            .unwrap()
            .timeout(Timeout {
                total: Some(Duration::from_secs(5)),
                ..Timeout::default()
            })
            .send()
            .await
    });

    // Drive the first body to its Total terminal state.
    let mut saw_total = false;
    while let Some(item) = first_stream.next().await {
        if let Err(e) = item {
            assert_total(&e);
            saw_total = true;
            break;
        }
    }
    assert!(saw_total, "first body must hit Total");
    drop(first);

    // The waiting second request must now be admitted and succeed.
    let mut second_resp = tokio::time::timeout(Duration::from_secs(5), second)
        .await
        .expect("second request admitted after Total lease release")
        .unwrap()
        .unwrap();
    second_resp.bytes().await.unwrap();
    server.shutdown();
}

// --- E1: redirect aggregate deadline ---

#[tokio::test]
async fn redirect_final_body_uses_remaining_original_budget() {
    // Discrimination window: a fresh total after final headers would need
    // the full `total` again, while the correct remaining budget resolves
    // much sooner. The external timeout sits between those two deadlines.
    //
    // total = 1500ms, first hop ~= 1000ms, remaining ~= 500ms,
    // discrimination = 900ms, fresh restarted total = 1500ms after headers.
    let total = Duration::from_millis(1500);
    let discrimination = Duration::from_millis(900);

    // Final origin: headers immediately, then stall well beyond both the
    // remaining budget and a freshly restarted total.
    let final_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let final_addr = final_listener.local_addr().unwrap();
    let final_url = format!("http://{final_addr}/");
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        if let Ok((mut socket, _)) = final_listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let _ = socket.flush().await;
            // Stall past both the remaining budget (~500ms) and a fresh
            // total (1500ms) so only the deadline decides the outcome.
            tokio::time::sleep(Duration::from_millis(3000)).await;
            let _ = socket.write_all(b"0\r\n\r\n").await;
        }
    });

    // First hop consumes most of the total via header delay, then redirects.
    let mut first_server = TestServer::start(&TestServerConfig {
        response_delay_ms: 1000,
        redirect: Some((302, final_url.clone())),
        ..Default::default()
    });
    let first_url = first_server.url();

    let client = Client::builder()
        .follow_redirects(true)
        .timeout(Timeout {
            total: Some(total),
            ..Timeout::default()
        })
        .build();
    let start = Instant::now();
    let mut resp = client
        .get(&first_url)
        .unwrap()
        .send()
        .await
        .expect("redirect reaches final headers before original total");
    // Final URL reached; body must still respect the original deadline.
    assert_eq!(resp.url().as_str(), final_url.as_str());

    // After final headers, the body must fail with Total inside the
    // discrimination window. A restarted fresh total would still be pending
    // here, surfacing as the outer test timeout instead of Total.
    let body_start = Instant::now();
    let outcome = tokio::time::timeout(discrimination, resp.bytes()).await;
    let body_elapsed = body_start.elapsed();
    let result = outcome.expect(
        "final body must resolve within the discrimination window; \
         a fresh total after redirect would still be pending",
    );
    let err = result.unwrap_err();
    assert_total(&err);
    assert!(
        body_elapsed < discrimination,
        "body must use the remaining original budget, not a fresh total; body_elapsed={body_elapsed:?}"
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(1000) + discrimination + Duration::from_millis(600),
        "overall request must stay near the original deadline; elapsed={elapsed:?}"
    );
    first_server.shutdown();
}

// --- Body-size controls remain authoritative (Part F) ---

#[tokio::test]
async fn unknown_length_chunked_body_respects_decoded_limit() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEF".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_decoded_body_size(4).build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert_eq!(err.kind(), "decoded_body_too_large");
    server.shutdown();
}

// --- G7: native frame-preserving surface ---

mod native {
    use super::*;
    use eggfetch_core::NativeRequestOptions;
    use http_body::Body as _;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn start_stalling_frame_server(stall_ms: u64) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
                    .await;
                let _ = socket.flush().await;
                tokio::time::sleep(Duration::from_millis(stall_ms)).await;
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn native_body_exceeding_total_reports_total() {
        let url = start_stalling_frame_server(800).await;
        let request = http::Request::get(&url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = Client::new()
            .execute_http_body(
                request,
                NativeRequestOptions::default().timeout(Timeout {
                    total: Some(Duration::from_millis(250)),
                    ..Timeout::default()
                }),
            )
            .await
            .expect("headers before total");
        let mut body = Box::pin(response.into_body());
        // First DATA frame may arrive before total; terminal stall must be Total.
        let mut saw_total = false;
        while let Some(frame) =
            futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await
        {
            if let Err(e) = frame {
                assert_total(&e);
                saw_total = true;
                break;
            }
        }
        assert!(saw_total, "native body must terminate with Total");
    }

    #[tokio::test]
    async fn native_delayed_first_frame_poll_after_total_reports_total() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await;
            }
        });
        let url = format!("http://{addr}/");
        let request = http::Request::get(&url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = Client::new()
            .execute_http_body(
                request,
                NativeRequestOptions::default().timeout(Timeout {
                    total: Some(Duration::from_millis(150)),
                    ..Timeout::default()
                }),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(350)).await;
        let mut body = Box::pin(response.into_body());
        let err = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
            .await
            .unwrap()
            .unwrap_err();
        assert_total(&err);
    }

    #[tokio::test]
    async fn native_read_shorter_reports_read() {
        let url = start_stalling_frame_server(800).await;
        let request = http::Request::get(&url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = Client::new()
            .execute_http_body(
                request,
                NativeRequestOptions::default().timeout(Timeout {
                    read: Some(Duration::from_millis(100)),
                    total: Some(Duration::from_secs(5)),
                    ..Timeout::default()
                }),
            )
            .await
            .unwrap();
        let mut body = Box::pin(response.into_body());
        // Drain the ready "ab" frame, then the stall must be Read.
        let first = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"ab"));
        let err = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
            .await
            .unwrap()
            .unwrap_err();
        assert_read(&err);
    }

    #[tokio::test]
    async fn native_total_shorter_reports_total() {
        let url = start_stalling_frame_server(800).await;
        let request = http::Request::get(&url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = Client::new()
            .execute_http_body(
                request,
                NativeRequestOptions::default().timeout(Timeout {
                    read: Some(Duration::from_secs(5)),
                    total: Some(Duration::from_millis(200)),
                    ..Timeout::default()
                }),
            )
            .await
            .unwrap();
        let mut body = Box::pin(response.into_body());
        let mut saw_total = false;
        while let Some(frame) =
            futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await
        {
            if let Err(e) = frame {
                assert_total(&e);
                saw_total = true;
                break;
            }
        }
        assert!(saw_total);
    }

    #[tokio::test]
    async fn native_total_timeout_releases_pool_lease() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    if tokio::io::AsyncWriteExt::write_all(
                        &mut socket,
                        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab",
                    )
                    .await
                    .is_err()
                    {
                        return;
                    }
                    let _ = socket.flush().await;
                    // First connection stalls; second serves promptly.
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    let _ = socket.write_all(b"cd").await;
                }
            }
        });
        // Simpler deterministic lease proof: first body times out, second
        // same-origin request proceeds after the lease is released.
        let slow_url = format!("http://{addr}/slow");
        let client = Client::builder().max_in_flight_requests(1).build();
        let request = http::Request::get(&slow_url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = client
            .execute_http_body(
                request,
                NativeRequestOptions::default().timeout(Timeout {
                    total: Some(Duration::from_millis(200)),
                    ..Timeout::default()
                }),
            )
            .await
            .unwrap();
        let mut body = Box::pin(response.into_body());
        let mut saw_total = false;
        while let Some(frame) =
            futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await
        {
            if let Err(e) = frame {
                if matches!(
                    e,
                    Error::Timeout {
                        phase: TimeoutPhase::Total,
                        ..
                    }
                ) {
                    saw_total = true;
                }
                break;
            }
        }
        assert!(saw_total, "first native body must hit Total");
        drop(body);
        // Lease released: the second same-origin request owns the only
        // logical permit and must reach response headers successfully.
        let request2 = http::Request::get(&slow_url)
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(3),
            client.execute_http_body_default(request2),
        )
        .await
        .expect("second request admitted after total timeout")
        .expect("second request should reach response headers");
        assert_eq!(response.status(), http::StatusCode::OK);
    }
}
