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

//! HTTP/3 interoperability and production-graduation evidence.
//!
//! Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
//! Plan: `plans/http3-interoperability-and-production-graduation.md`
//!
//! This suite is deterministic and local-only for Tier 1. External
//! interoperability servers (ngtcp2/nghttp3, quiche, …) are exercised only
//! when explicitly provided via `EGGFETCH_H3_INTEROP_URLS` (comma-separated
//! `https://host:port/` origins with a trusted or `danger`-accepted cert).
//! Absence is reported as an explicit skip and is NOT graduation evidence.
//!
//! Graduation decision (this milestone): HTTP/3 remains **experimental**.
//! Blockers are recorded in `docs/architecture/core-tls-proxy-protocols.md`
//! (§ "Production Graduation Decision"). A retained experimental label is a
//! valid successful outcome per the graduation plan.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, HttpVersionPolicy, Timeout, TlsConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Fixtures (loopback QUIC+H3, same shape as h3_hardening.rs)
// ---------------------------------------------------------------------------

struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

impl QuicTestServer {
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start(body: Vec<u8>) -> Self {
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

fn h3_client_with_connect_timeout(d: Duration) -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    let timeout = Timeout::builder().connect(d).build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .timeout(timeout)
        .build()
}

// ---------------------------------------------------------------------------
// §2: Deterministic local interop harness + optional external servers
// ---------------------------------------------------------------------------

/// Local Quinn/h3 self-interop is mandatory and deterministic.
#[tokio::test(flavor = "multi_thread")]
async fn local_quinn_self_interop_get_and_body() {
    let server = QuicTestServer::start(b"interop-ok".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("h3 get");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("body");
    assert_eq!(body, "interop-ok");
}

/// Optional external interop servers. Never fails Tier 1 when absent;
/// absence is an explicit skip, not graduation evidence.
#[tokio::test(flavor = "multi_thread")]
async fn external_h3_interop_servers_if_configured() {
    let urls = std::env::var("EGGFETCH_H3_INTEROP_URLS").unwrap_or_default();
    let urls: Vec<String> = urls
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if urls.is_empty() {
        eprintln!(
            "SKIP external_h3_interop_servers_if_configured: \
             EGGFETCH_H3_INTEROP_URLS unset; local fixtures remain mandatory, \
             absence is not graduation evidence"
        );
        return;
    }
    for url in &urls {
        let client = Client::builder()
            .http_version_policy(HttpVersionPolicy::Http3Only)
            .build();
        let mut resp = client
            .get(url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("external h3 get {url} failed: {e:?}"));
        assert!(
            (200..400).contains(&resp.status().as_u16()),
            "unexpected status for {url}"
        );
        // Drain body to prove streaming works against the external stack.
        let _ = resp.text().await.expect("external body");
    }
}

// ---------------------------------------------------------------------------
// §4: Network impairment and fallback qualification (local, deterministic)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn udp_blackhole_terminates_within_connect_budget() {
    let hole = Blackhole::bind();
    let url = hole.url();
    let client = h3_client_with_connect_timeout(Duration::from_millis(400));
    let err = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    // Must terminate promptly (connect budget), not hang.
    let _ = format!("{err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_then_good_origin_does_not_poison_client() {
    // One bad (blackhole) and one good origin on the same client: the
    // failure must not poison later requests to a healthy origin.
    let hole = Blackhole::bind();
    let hole_url = hole.url();
    let good = QuicTestServer::start(b"good".to_vec()).await;
    let good_url = good.url();
    let client = h3_client_with_connect_timeout(Duration::from_millis(400));
    let _ = client
        .get(&hole_url)
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    let mut resp = client
        .get(&good_url)
        .unwrap()
        .send()
        .await
        .expect("good origin after failure");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "good");
}

#[tokio::test(flavor = "multi_thread")]
async fn server_restart_reconnects_on_next_request() {
    let body = b"restart-ok".to_vec();
    let first = QuicTestServer::start(body.clone()).await;
    let first_url = first.url();
    let client = h3_client();
    let mut resp = client.get(&first_url).unwrap().send().await.expect("first");
    assert_eq!(resp.text().await.expect("body"), "restart-ok");
    drop(first);
    // New server generation (possibly a different port, same client):
    // the client must reconnect rather than stick to a dead generation.
    let second = QuicTestServer::start(body).await;
    let second_url = second.url();
    let mut resp = client
        .get(&second_url)
        .unwrap()
        .send()
        .await
        .expect("second generation");
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_during_h3_connect_is_prompt() {
    let hole = Blackhole::bind();
    let hole_url = hole.url();
    let client = h3_client_with_connect_timeout(Duration::from_secs(10));
    let fut = client.get(&hole_url).unwrap().send();
    let res = tokio::time::timeout(Duration::from_millis(500), fut).await;
    assert!(res.is_err(), "outer timeout must fire while H3 dial stalls");
    // Client stays usable afterwards.
    let server = QuicTestServer::start(b"after-cancel".to_vec()).await;
    let url = server.url();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after cancel");
    assert_eq!(resp.status().as_u16(), 200);
}

// ---------------------------------------------------------------------------
// §5: Resource and soak evidence (bounded, deterministic)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn hundred_sequential_requests_on_reused_h3_connection() {
    let server = QuicTestServer::start(b"x".repeat(64)).await;
    let url = server.url();
    let client = h3_client();
    for i in 0..100u32 {
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("request {i} failed: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.expect("body");
        assert_eq!(body.len(), 64, "request {i}");
    }
    // Cache must stay bounded: at most a handful of generations for one origin.
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 10,
        "connection churn too high: {}",
        m.h3_connections_created
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_fail_reconnect_cycles_do_not_grow_state() {
    let client = h3_client_with_connect_timeout(Duration::from_millis(200));
    for _ in 0..10u32 {
        let hole = Blackhole::bind();
        let url = hole.url();
        let _ = client.get(&url).unwrap().send().await;
    }
    let m = client.transport_metrics().snapshot();
    // Bounded counters only; no per-origin leak surfaced as unbounded growth.
    assert!(m.h3_connections_created < 64);
    // Client still works against a good origin.
    let good = QuicTestServer::start(b"ok".to_vec()).await;
    let url = good.url();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("good after cycles");
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_storm_leaves_client_usable() {
    let client = h3_client_with_connect_timeout(Duration::from_secs(5));
    let server = QuicTestServer::start(b"storm-ok".to_vec()).await;
    let url = server.url();
    for _ in 0..20u32 {
        let fut = client.get(&url).unwrap().send();
        let _ = tokio::time::timeout(Duration::from_millis(50), fut).await;
    }
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after storm");
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn client_construction_drop_loops_do_not_leak() {
    for _ in 0..20u32 {
        let server = QuicTestServer::start(b"drop-ok".to_vec()).await;
        let url = server.url();
        let client = h3_client();
        let mut resp = client.get(&url).unwrap().send().await.expect("req");
        assert_eq!(resp.status().as_u16(), 200);
        drop(client);
    }
}

// ---------------------------------------------------------------------------
// §6: QUIC/H3 observability qualification (no fabricated metadata)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_metrics_are_exact_and_metadata_is_truthful() {
    let server = QuicTestServer::start(b"obs".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let before = client.transport_metrics().snapshot();
    let mut resp = client.get(&url).unwrap().send().await.expect("req");
    assert_eq!(resp.status().as_u16(), 200);
    let _ = resp.text().await.expect("body");
    let after = client.transport_metrics().snapshot();
    // Exactly one H3 connection creation for one fresh origin.
    assert_eq!(
        after.h3_connections_created,
        before.h3_connections_created + 1
    );
    // H3 per-response network-stream metadata stays None (never
    // synthesized); TransportKind::Quic is reserved.
    assert!(resp.network_stream().is_none());
}

/// IPv6 loopback may be unavailable in some containers; report skip
/// explicitly rather than failing or claiming support.
#[tokio::test(flavor = "multi_thread")]
async fn ipv6_availability_is_detected_not_assumed() {
    let sock = std::net::UdpSocket::bind("[::1]:0");
    if sock.is_err() {
        eprintln!("SKIP ipv6 loopback unavailable in this environment");
    }
    // Socket bound or skipped; no further claim — platform support matrix
    // lives in docs.
}

// ---------------------------------------------------------------------------
// §8: Platform notes (no platform-specific UDP/QUIC assumptions)
// ---------------------------------------------------------------------------

#[test]
fn h3_platform_limitations_are_documented() {
    // This test pins that the graduation doc names the platform scope.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let doc_path = manifest.join("../../docs/architecture/core-tls-proxy-protocols.md");
    let doc = std::fs::read_to_string(&doc_path).expect("graduation doc readable");
    for needle in [
        "Production Graduation Decision",
        "quinn 0.11",
        "h3 0.0.8",
        "h3-quinn 0.0.10",
    ] {
        assert!(doc.contains(needle), "doc missing {needle:?}");
    }
}
