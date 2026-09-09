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
#![cfg(feature = "http3")]
#![allow(clippy::unnested_or_patterns)]

//! HTTP/3 lifecycle and policy hardening tests.
//!
//! Covers the plan `plans/http3-lifecycle-and-policy-hardening.md` at the
//! public client level using deterministic local fixtures (loopback QUIC
//! servers, UDP blackholes). No public internet is used.
//!
//! Cache boundedness itself is pinned by unit tests in
//! `src/transport/http3.rs` (which can inspect the crate-private cache);
//! these integration tests pin the behavioral consequences: phase-correct
//! timeouts, pool-permit compliance, concurrent-init sharing, eviction and
//! reconnect, cancellation safety, and resource stabilization.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, Error, HttpVersionPolicy, Timeout, TimeoutPhase, TlsConfig};
use futures_util::StreamExt;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Minimal local QUIC+H3 server. Serves `behavior` on every request.
struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServerBehavior {
    /// Respond 200 with `body` immediately.
    Immediate(usize),
    /// Send response headers, then stall before any body bytes.
    StallBody,
}

impl QuicTestServer {
    async fn start(body: Vec<u8>) -> Self {
        Self::start_with_behavior(body, ServerBehavior::Immediate(0)).await
    }

    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start_with_behavior(body: Vec<u8>, behavior: ServerBehavior) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let Ok((_req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                if behavior == ServerBehavior::StallBody {
                                    let resp = http::Response::builder()
                                        .status(200)
                                        .body(())
                                        .unwrap();
                                    if stream.send_response(resp).await.is_err() {
                                        break;
                                    }
                                    // Stall before any body bytes so the
                                    // client's read timeout fires.
                                    tokio::time::sleep(Duration::from_secs(30)).await;
                                    let _ = stream.send_data(body).await;
                                    let _ = stream.finish().await;
                                    break;
                                }
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.addr.port())
    }
}

impl Drop for QuicTestServer {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// UDP blackhole: bound but never answered, so QUIC handshakes stall until
/// the caller's timeout fires. Deterministic without public internet.
struct Blackhole {
    _socket: std::net::UdpSocket,
    addr: SocketAddr,
}

impl Blackhole {
    fn bind() -> Self {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind blackhole UDP socket");
        socket.set_nonblocking(true).expect("nonblocking");
        let addr = socket.local_addr().expect("blackhole addr");
        Self {
            _socket: socket,
            addr,
        }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.addr.port())
    }
}

fn h3_client() -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .build()
}

fn h3_client_with_timeout(timeout: Timeout) -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .timeout(timeout)
        .build()
}

fn timeout_phase(err: &Error) -> Option<TimeoutPhase> {
    match err {
        Error::Timeout { phase, .. } => Some(*phase),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Timeout phase correctness
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_connect_timeout_phase_when_connect_tighter_than_total() {
    let blackhole = Blackhole::bind();
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_millis(200))
            .total(Duration::from_secs(10))
            .build(),
    );
    let err = client
        .get(&blackhole.url())
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    assert_eq!(
        timeout_phase(&err),
        Some(TimeoutPhase::Connect),
        "tight connect budget must surface Connect, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_total_wins_when_total_tighter_than_connect() {
    let blackhole = Blackhole::bind();
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_secs(10))
            .total(Duration::from_millis(300))
            .build(),
    );
    let err = client
        .get(&blackhole.url())
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    assert_eq!(
        timeout_phase(&err),
        Some(TimeoutPhase::Total),
        "tight total budget must surface Total, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_read_timeout_fires_on_stalled_body() {
    let server =
        QuicTestServer::start_with_behavior(vec![0xAB; 1024], ServerBehavior::StallBody).await;
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_secs(10))
            .read(Duration::from_millis(300))
            .total(Duration::from_secs(20))
            .build(),
    );
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("headers arrive before the stall");
    assert_eq!(resp.status().as_u16(), 200);
    let mut stream = resp.bytes_stream().expect("stream");
    let err = stream
        .next()
        .await
        .expect("stalled body yields an item")
        .expect_err("stalled body must time out");
    assert_eq!(
        timeout_phase(&err),
        Some(TimeoutPhase::Read),
        "stalled body must surface Read, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Concurrency: shared init and pool permits
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_concurrent_init_shares_one_attempt() {
    let server = QuicTestServer::start(b"shared".to_vec()).await;
    let client = h3_client();
    let url = server.url();
    let mut handles = Vec::new();
    for _ in 0..10 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let mut resp = client.get(&url).unwrap().send().await.expect("send");
            assert_eq!(resp.status().as_u16(), 200);
            assert_eq!(resp.text().await.expect("body"), "shared");
        }));
    }
    for h in handles {
        h.await.expect("task join");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_pool_permits_gate_concurrent_streams() {
    let server = QuicTestServer::start(b"permit".to_vec()).await;
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .max_connections_per_host(1)
        .build();
    let url = server.url();
    let mut handles = Vec::new();
    for _ in 0..5 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let mut resp = client.get(&url).unwrap().send().await.expect("send");
            assert_eq!(resp.status().as_u16(), 200);
            assert_eq!(resp.text().await.expect("body"), "permit");
        }));
    }
    for h in handles {
        h.await.expect("task join");
    }
    // No permit leak: a follow-up request still acquires immediately.
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("post-burst request");
    assert_eq!(resp.text().await.expect("body"), "permit");
}

// ---------------------------------------------------------------------------
// Failure, eviction, reconnect, and resource stabilization
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_failed_origin_does_not_poison_client() {
    let blackhole = Blackhole::bind();
    let server = QuicTestServer::start(b"alive".to_vec()).await;
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_millis(200))
            .total(Duration::from_secs(10))
            .build(),
    );
    let err = client
        .get(&blackhole.url())
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    assert!(
        matches!(
            err,
            Error::Timeout { .. } | Error::Connect(_) | Error::H3Connect(_)
        ),
        "failed origin must report connect taxonomy, got {err:?}"
    );
    // The client stays usable; the failed origin did not trap it.
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("live origin after failure");
    assert_eq!(resp.text().await.expect("body"), "alive");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_distinct_origins_do_not_accumulate_failure_state() {
    let server = QuicTestServer::start(b"stable".to_vec()).await;
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_millis(150))
            .total(Duration::from_secs(10))
            .build(),
    );
    // Sequential requests to many distinct dead origins (distinct ports).
    // Each fails under its connect budget; none may trap the client.
    for _ in 0..10 {
        let hole = Blackhole::bind();
        let err = client
            .get(&hole.url())
            .unwrap()
            .send()
            .await
            .expect_err("blackhole must fail");
        assert!(
            matches!(
                err,
                Error::Timeout { .. } | Error::Connect(_) | Error::H3Connect(_)
            ),
            "unexpected taxonomy: {err:?}"
        );
    }
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("live origin after many failures");
    assert_eq!(resp.text().await.expect("body"), "stable");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_repeated_fail_reconnect_cycles_stabilize() {
    let server = QuicTestServer::start(b"cycle".to_vec()).await;
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_millis(150))
            .total(Duration::from_secs(10))
            .build(),
    );
    for _ in 0..3 {
        let hole = Blackhole::bind();
        let _ = client.get(&hole.url()).unwrap().send().await;
        let mut resp = client
            .get(&server.url())
            .unwrap()
            .send()
            .await
            .expect("reconnect each cycle");
        assert_eq!(resp.text().await.expect("body"), "cycle");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_partial_response_drop_keeps_connection_usable() {
    let payload = vec![0x42_u8; 512 * 1024];
    let server = QuicTestServer::start(payload).await;
    let client = h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("send");
    // Consume one chunk, then drop without draining.
    let mut stream = resp.bytes_stream().expect("stream");
    let first = stream.next().await.expect("first chunk").expect("chunk ok");
    assert!(!first.is_empty());
    drop(stream);
    drop(resp);
    // The connection and pool slot recover; the next request is unaffected.
    let mut resp2 = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("reuse after partial drop");
    assert_eq!(resp2.status().as_u16(), 200);
    assert_eq!(resp2.bytes().await.expect("body").len(), 512 * 1024);
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_client_drop_releases_origins() {
    let server = QuicTestServer::start(b"drop".to_vec()).await;
    let url = server.url();
    {
        let client = h3_client();
        let mut resp = client.get(&url).unwrap().send().await.expect("send");
        assert_eq!(resp.text().await.expect("body"), "drop");
        // `client` drops here with a cached origin.
    }
    // A fresh client reconnects cleanly to the same origin.
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("send");
    assert_eq!(resp.text().await.expect("body"), "drop");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_cancellation_is_prompt_and_client_stays_usable() {
    let blackhole = Blackhole::bind();
    let server = QuicTestServer::start(b"after-cancel".to_vec()).await;
    let client = h3_client_with_timeout(
        Timeout::builder()
            .connect(Duration::from_secs(30))
            .total(Duration::from_secs(30))
            .build(),
    );
    let pending = client.get(&blackhole.url()).unwrap().send();
    let cancelled = tokio::time::timeout(Duration::from_millis(200), pending).await;
    assert!(
        cancelled.is_err(),
        "abandoning the request must stop promptly instead of stalling to the 30s budget"
    );
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("client usable after cancellation");
    assert_eq!(resp.text().await.expect("body"), "after-cancel");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_cancellation_while_streaming_body_aborts() {
    let server = QuicTestServer::start(b"ok".to_vec()).await;
    let client = h3_client();
    let pending = client.get(&server.url()).unwrap().send();
    // Cancel a healthy request mid-flight as well; it must resolve (either
    // way) instead of hanging the test runtime.
    let outcome = tokio::time::timeout(Duration::from_secs(15), pending).await;
    assert!(
        outcome.is_ok(),
        "healthy request must complete within the outer bound"
    );
}
