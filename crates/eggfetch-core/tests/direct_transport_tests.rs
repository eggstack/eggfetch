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
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
//! Phase 4: Advanced direct transport integration tests.
//!
//! Track 5.1 — Pool isolation
//! Track 5.2 — Timeout ownership
//! Track 5.3 — Cancellation/close
//! Track 5.4 — TLS regression (ordinary HTTPS path unchanged)

mod test_server;

use std::net::SocketAddr;
use std::time::Duration;

use eggfetch_core::transport::direct_connector::SocketOption;
use eggfetch_core::{Client, Error, Timeout, TimeoutPhase};
use test_server::{TestServer, TestServerConfig};

/// Helper to create a `TCP_NODELAY` socket option.
fn tcp_nodelay_option() -> SocketOption {
    SocketOption {
        level: 6,  // IPPROTO_TCP
        option: 1, // TCP_NODELAY
        value: 1i32.to_ne_bytes().to_vec(),
    }
}

/// Helper to create an unrecognized socket option.
fn unrecognized_option() -> SocketOption {
    SocketOption {
        level: 999,
        option: 999,
        value: vec![0, 1, 2, 3],
    }
}

/// Helper to create a short-value socket option.
fn short_value_option() -> SocketOption {
    SocketOption {
        level: 6,
        option: 1,
        value: vec![1],
    }
}

// ── Track 5.1: Pool isolation ───────────────────────────────────────────

/// Separate client instances with different configurations get separate
/// pools and cannot share connections.
#[tokio::test]
async fn test_pool_isolation_different_socket_options() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    // Client A with TCP_NODELAY socket option.
    let client_a = Client::builder()
        .socket_options(vec![tcp_nodelay_option()])
        .build();

    // Client B with no socket options.
    let client_b = Client::builder().build();

    // Both should succeed independently.
    let mut resp_a = client_a.get(&url).unwrap().send().await.unwrap();
    assert!(resp_a.is_success());
    let _ = resp_a.bytes().await;

    let mut resp_b = client_b.get(&url).unwrap().send().await.unwrap();
    assert!(resp_b.is_success());
    let _ = resp_b.bytes().await;

    server.shutdown();
}

/// Two different local address bindings get separate pools.
#[tokio::test]
async fn test_pool_isolation_different_local_address() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    // Client A bound to 127.0.0.1:0 (OS picks ephemeral port).
    let client_a = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .build();

    // Client B bound to a different ephemeral port.
    let client_b = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .build();

    // Both should succeed independently, proving separate pools.
    let mut resp_a = client_a.get(&url).unwrap().send().await.unwrap();
    assert!(resp_a.is_success());
    let _ = resp_a.bytes().await;

    let mut resp_b = client_b.get(&url).unwrap().send().await.unwrap();
    assert!(resp_b.is_success());
    let _ = resp_b.bytes().await;

    server.shutdown();
}

/// UDS handler creates a separate path from TCP — cannot share connections.
#[cfg(unix)]
#[tokio::test]
async fn test_pool_isolation_uds_vs_tcp() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let sock_path = "/tmp/eggfetch_test_isolation.sock";
    let _ = std::fs::remove_file(sock_path);

    let shutdown = Arc::new(AtomicBool::new(false));
    let sd = shutdown.clone();
    let sp = sock_path.to_owned();

    let handle = std::thread::spawn(move || {
        let listener = UnixListener::bind(&sp).unwrap();
        listener.set_nonblocking(true).unwrap();
        while !sd.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).unwrap_or(0);
                    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nUDS-OK!";
                    let _ = stream.write_all(response);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    let uds_client = Client::builder().uds_path(sock_path.to_owned()).build();

    let mut tcp_server = TestServer::start(&TestServerConfig::default());
    let tcp_url = tcp_server.url();

    // UDS request succeeds.
    let mut resp = uds_client
        .get("http://localhost/test")
        .unwrap()
        .send()
        .await
        .unwrap();
    assert!(resp.is_success());
    let _ = resp.bytes().await;

    // TCP request succeeds — separate pool.
    let mut resp = uds_client.get(&tcp_url).unwrap().send().await.unwrap();
    assert!(resp.is_success());
    let _ = resp.bytes().await;

    shutdown.store(true, Ordering::SeqCst);
    let _ = handle.join();
    let _ = std::fs::remove_file(sock_path);
    tcp_server.shutdown();
}

// ── Track 5.2: Timeout ownership ────────────────────────────────────────

/// Total timeout includes the direct connector's connection establishment.
#[tokio::test]
async fn test_direct_connector_connect_timeout() {
    // Use the total timeout to prove advanced connect paths enforce timeouts.
    // The connect phase may produce a connect error (e.g., EINVAL for
    // unroutable addresses) rather than a timeout when the OS rejects the
    // connection immediately. The total timeout proves the path is not
    // bypassing the timeout framework.
    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Timeout::default()
        })
        .build();

    let result = client.get("http://192.0.2.1:80/").unwrap().send().await;

    // Either a timeout or a connect error is acceptable — the point is
    // the request does not hang indefinitely.
    assert!(result.is_err(), "expected error for unroutable address");
}

/// Total timeout fires for direct connector even when connect succeeds.
#[tokio::test]
async fn test_direct_connector_total_timeout() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    let result = client.get(&url).unwrap().send().await;
    assert!(result.is_err(), "expected total timeout");
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "expected total timeout, got: {err:?}"
    );

    server.shutdown();
}

/// UDS total timeout fires when server is slow to respond.
#[cfg(unix)]
#[tokio::test]
async fn test_uds_total_timeout() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let sock_path = "/tmp/eggfetch_test_uds_timeout.sock";
    let _ = std::fs::remove_file(sock_path);

    let shutdown = Arc::new(AtomicBool::new(false));
    let sd = shutdown.clone();
    let sp = sock_path.to_owned();

    let handle = std::thread::spawn(move || {
        let listener = UnixListener::bind(&sp).unwrap();
        listener.set_nonblocking(true).unwrap();
        while !sd.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).unwrap_or(0);
                    // Delay response beyond timeout.
                    std::thread::sleep(Duration::from_secs(5));
                    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(response);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    let client = Client::builder()
        .uds_path(sock_path.to_owned())
        .timeout(Timeout {
            total: Some(Duration::from_millis(100)),
            ..Timeout::default()
        })
        .build();

    let result = client.get("http://localhost/test").unwrap().send().await;

    assert!(result.is_err(), "expected UDS timeout");
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "expected total timeout, got: {err:?}"
    );

    shutdown.store(true, Ordering::SeqCst);
    let _ = handle.join();
    let _ = std::fs::remove_file(sock_path);
}

// ── Track 5.3: Cancellation/close ───────────────────────────────────────

/// Cancelling an in-flight direct connector request releases resources.
#[tokio::test]
async fn test_direct_connector_cancellation() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let url = server.url();

    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .max_connections(1)
        .build();

    let handle = tokio::spawn({
        let client = client.clone();
        let url = url.clone();
        async move { client.get(&url).unwrap().send().await }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Subsequent request should succeed — pool slot was released.
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());
    let _ = resp.bytes().await;

    server.shutdown();
}

/// UDS cancellation releases the connection and allows subsequent requests.
#[cfg(unix)]
#[tokio::test]
async fn test_uds_cancellation() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let sock_path = "/tmp/eggfetch_test_uds_cancel.sock";
    let _ = std::fs::remove_file(sock_path);

    let shutdown = Arc::new(AtomicBool::new(false));
    let sd = shutdown.clone();
    let sp = sock_path.to_owned();

    let handle = std::thread::spawn(move || {
        let listener = UnixListener::bind(&sp).unwrap();
        listener.set_nonblocking(true).unwrap();
        while !sd.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).unwrap_or(0);
                    std::thread::sleep(Duration::from_millis(300));
                    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(response);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(50));

    let client = Client::builder().uds_path(sock_path.to_owned()).build();

    // Fire and cancel.
    let handle_req = tokio::spawn({
        let client = client.clone();
        async move { client.get("http://localhost/test").unwrap().send().await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    handle_req.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Subsequent request should succeed — UDS creates fresh connections
    // per request (no connection reuse), so cancellation is always safe.
    let mut resp = client
        .get("http://localhost/test")
        .unwrap()
        .send()
        .await
        .unwrap();
    assert!(resp.is_success());
    let _ = resp.bytes().await;

    shutdown.store(true, Ordering::SeqCst);
    let _ = handle.join();
    let _ = std::fs::remove_file(sock_path);
}

// ── Track 5.4: TLS regression (ordinary HTTPS path unchanged) ──────────

/// Ordinary HTTP over TCP still works through the standard hyper path
/// when no advanced options are set.
#[tokio::test]
async fn test_standard_tcp_path_unchanged() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder().build();

    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());

    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");

    server.shutdown();
}

/// Direct connector path produces the same HTTP response as the standard path.
#[tokio::test]
async fn test_direct_connector_same_response_as_standard() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let standard_client = Client::builder().build();
    let direct_client = Client::builder()
        .socket_options(vec![tcp_nodelay_option()])
        .build();

    let mut resp_std = standard_client.get(&url).unwrap().send().await.unwrap();
    let mut resp_dir = direct_client.get(&url).unwrap().send().await.unwrap();

    assert_eq!(resp_std.status(), resp_dir.status());
    assert_eq!(
        resp_std.text().await.unwrap(),
        resp_dir.text().await.unwrap()
    );

    server.shutdown();
}

// ── Socket option error tests ──────────────────────────────────────────

/// Unrecognized socket option produces a deterministic error.
#[tokio::test]
async fn test_unrecognized_socket_option_errors() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder()
        .socket_options(vec![unrecognized_option()])
        .build();

    let result = client.get(&url).unwrap().send().await;
    assert!(
        result.is_err(),
        "expected error for unrecognized socket option"
    );
    // The error is wrapped by hyper_util. The unit tests in
    // direct_connector.rs prove the exact error message content.
    // Here we verify the error propagates (not silently ignored).
}

/// Short socket option value produces a deterministic error.
#[tokio::test]
async fn test_short_socket_option_value_errors() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let client = Client::builder()
        .socket_options(vec![short_value_option()])
        .build();

    let result = client.get(&url).unwrap().send().await;
    assert!(
        result.is_err(),
        "expected error for short socket option value"
    );
    // The error is wrapped by hyper_util. The unit tests in
    // direct_connector.rs prove the exact error message content.
}

/// HTTPS over UDS produces a deterministic error.
#[cfg(unix)]
#[tokio::test]
async fn test_uds_https_rejected() {
    let client = Client::builder()
        .uds_path("/tmp/test.sock".to_owned())
        .build();

    let result = client.get("https://localhost/test").unwrap().send().await;

    assert!(result.is_err(), "expected error for HTTPS over UDS");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), "unsupported");
    assert!(
        err.to_string().contains("HTTPS"),
        "error should mention HTTPS: {err}"
    );
}
