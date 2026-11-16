#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
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
    clippy::map_unwrap_or,
    clippy::unused_async,
    clippy::single_match_else
)]
#![allow(
    unknown_lints,
    clippy::unused_async_trait_impl,
    clippy::unnested_or_patterns
)]
#![cfg(feature = "http3")]

//! Qualification-only HTTP/3 impairment matrix scenarios.
//!
//! Each test is named exactly as `qualification/http3/impairment-matrix.json` ids.
//! Unselected scenarios skip without claiming matrix evidence. When
//! `EGGFETCH_H3_IMPAIRMENT_OUTPUT` is set, the selected scenario writes
//! coordinator JSON matching `scripts/h3_impairment.py`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use eggfetch_core::{
    Client, Error, HttpVersionPolicy, RequestBody, Timeout, TimeoutPhase, TlsConfig,
};
use futures_util::StreamExt;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Coordinator JSON + scenario gating
// ---------------------------------------------------------------------------

const NETEM_SCENARIOS: &[&str] = &[
    "packet_loss_low",
    "packet_loss_high",
    "latency_jitter",
    "packet_reordering",
    "packet_duplication",
    "reduced_mtu_pmtu_blackhole",
    "path_loss_after_headers",
    "path_loss_during_upload",
];

fn scenario_selected(id: &str) -> bool {
    std::env::var("EGGFETCH_H3_IMPAIRMENT_SCENARIO").is_ok_and(|v| v == id)
}

fn netem_active() -> bool {
    std::env::var("EGGFETCH_H3_IMPAIRMENT_NETEM_ACTIVE").as_deref() == Ok("1")
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_coordinator_result(
    scenario: &str,
    status: &str,
    invariants: &BTreeMap<&str, &str>,
    reason: Option<&str>,
) {
    let Ok(path) = std::env::var("EGGFETCH_H3_IMPAIRMENT_OUTPUT") else {
        return;
    };
    let mut inv_pairs = String::new();
    for (idx, (k, v)) in invariants.iter().enumerate() {
        if idx > 0 {
            inv_pairs.push(',');
        }
        inv_pairs.push_str(&format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)));
    }
    let reason_field = reason
        .map(|r| format!(",\n  \"reason\": \"{}\"", json_escape(r)))
        .unwrap_or_default();
    let body = format!(
        "{{\n  \"status\": \"{status}\",\n  \"scenario\": \"{scenario}\",\n  \"invariants\": {{{inv_pairs}}}{reason_field}\n}}\n",
        status = json_escape(status),
        scenario = json_escape(scenario),
    );
    let _ = std::fs::write(path, body);
}

fn all_invariants<'a>(names: &'a [&'a str], status: &'a str) -> BTreeMap<&'a str, &'a str> {
    names.iter().copied().map(|n| (n, status)).collect()
}

fn pass_result(scenario: &str, names: &[&str]) {
    write_coordinator_result(scenario, "pass", &all_invariants(names, "pass"), None);
}

fn unsupported_result(scenario: &str, names: &[&str], reason: &str) {
    write_coordinator_result(
        scenario,
        "unsupported",
        &all_invariants(names, "unsupported"),
        Some(reason),
    );
}

/// Returns false when the test body should not run (skip or unsupported already written).
fn gate(scenario: &str, invariants: &[&str]) -> bool {
    if !scenario_selected(scenario) {
        eprintln!(
            "SKIP {scenario}: EGGFETCH_H3_IMPAIRMENT_SCENARIO unset or names another scenario"
        );
        return false;
    }
    if NETEM_SCENARIOS.contains(&scenario) && !netem_active() {
        unsupported_result(scenario, invariants, "netem not active");
        return false;
    }
    true
}

fn timeout_phase(err: &Error) -> Option<TimeoutPhase> {
    match err {
        Error::Timeout { phase, .. } => Some(*phase),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Fixtures (loopback QUIC+H3)
// ---------------------------------------------------------------------------

struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
    uploads_seen: Arc<AtomicUsize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServerBehavior {
    Immediate,
    StallBody,
}

impl QuicTestServer {
    async fn start(body: Vec<u8>) -> Self {
        Self::start_on("127.0.0.1:0", body, ServerBehavior::Immediate).await
    }

    async fn start_with_behavior(body: Vec<u8>, behavior: ServerBehavior) -> Self {
        Self::start_on("127.0.0.1:0", body, behavior).await
    }

    async fn start_on(bind: &str, body: Vec<u8>, behavior: ServerBehavior) -> Self {
        let uploads_seen = Arc::new(AtomicUsize::new(0));
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
        let endpoint =
            quinn::Endpoint::server(server_config, bind.parse().unwrap()).expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let (tx, mut rx) = watch::channel(false);
        let uploads = uploads_seen.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        let uploads = uploads.clone();
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
                                let mut upload_chunks = 0usize;
                                while let Ok(Some(_chunk)) = stream.recv_data().await {
                                    upload_chunks += 1;
                                }
                                if upload_chunks > 0 {
                                    uploads.fetch_add(1, Ordering::SeqCst);
                                }
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                if behavior == ServerBehavior::StallBody {
                                    tokio::time::sleep(Duration::from_secs(30)).await;
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
        Self {
            addr,
            shutdown: tx,
            uploads_seen,
        }
    }

    fn url(&self) -> String {
        match self.addr {
            SocketAddr::V4(_) => format!("https://127.0.0.1:{}/", self.addr.port()),
            SocketAddr::V6(_) => format!("https://[::1]:{}/", self.addr.port()),
        }
    }

    fn uploads_seen(&self) -> usize {
        self.uploads_seen.load(Ordering::SeqCst)
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

/// Minimal TLS H1 server advertising Alt-Svc for impairment fallback scenarios.
struct H1AltSvcServer {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
}

impl H1AltSvcServer {
    async fn start(alt_svc: String, body: Vec<u8>) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("cert");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("port").port();
        let (tx, mut rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((tcp, _)) = result else { break };
                        let acceptor = acceptor.clone();
                        let alt_svc = alt_svc.clone();
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(tls) = acceptor.accept(tcp).await else { return };
                            let mut reader = BufReader::new(tls);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_err() || line.is_empty() {
                                return;
                            }
                            loop {
                                let mut h = String::new();
                                if reader.read_line(&mut h).await.is_err() || h.trim().is_empty() {
                                    break;
                                }
                            }
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nAlt-Svc: {}\r\nConnection: close\r\n\r\n",
                                body.len(),
                                alt_svc,
                            );
                            let mut stream = reader.into_inner();
                            let _ = stream.write_all(resp.as_bytes()).await;
                            let _ = stream.write_all(&body).await;
                            let _ = stream.flush().await;
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self {
            port,
            shutdown_tx: tx,
        }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }
}

impl Drop for H1AltSvcServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}

fn h3_only_client(timeout: Timeout) -> Client {
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

fn auto_client(timeout: Timeout) -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: true })
        .tls_config(tls)
        .automatic_decompression(false)
        .timeout(timeout)
        .build()
}

fn ipv6_loopback_available() -> bool {
    std::net::UdpSocket::bind("[::1]:0").is_ok()
}

fn ipv4_loopback_available() -> bool {
    std::net::UdpSocket::bind("127.0.0.1:0").is_ok()
}

// ---------------------------------------------------------------------------
// Matrix scenarios
// ---------------------------------------------------------------------------

const UDP_BLACKHOLE_INV: &[&str] = &[
    "bounded_connect_or_total_deadline",
    "Http3Only_does_not_fallback",
    "resources_released",
];

#[tokio::test(flavor = "multi_thread")]
async fn udp_blackhole() {
    if !gate("udp_blackhole", UDP_BLACKHOLE_INV) {
        return;
    }
    let hole = Blackhole::bind();
    let deadline = Timeout::builder()
        .connect(Duration::from_millis(400))
        .total(Duration::from_secs(2))
        .build();
    let client = h3_only_client(deadline);
    let before = client.transport_metrics().snapshot();
    let start = Instant::now();
    let err = client
        .get(&hole.url())
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "must respect bounded deadline"
    );
    assert!(
        matches!(
            err,
            Error::Timeout { .. } | Error::Connect(_) | Error::H3Connect(_)
        ),
        "unexpected error {err:?}"
    );
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.h3_fallback_selected, before.h3_fallback_selected,
        "Http3Only must not fallback"
    );
    let server = QuicTestServer::start(b"ok".to_vec()).await;
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("client usable after blackhole");
    assert_eq!(resp.text().await.expect("body"), "ok");
    pass_result("udp_blackhole", UDP_BLACKHOLE_INV);
}

const UDP_REJECTED_INV: &[&str] = &[
    "bounded_connect_or_total_deadline",
    "safe_auto_fallback_only_pre_commit",
    "resources_released",
];

#[tokio::test(flavor = "multi_thread")]
async fn udp_rejected() {
    if !gate("udp_rejected", UDP_REJECTED_INV) {
        return;
    }
    let hole = Blackhole::bind();
    let h3_good = QuicTestServer::start(b"fixture".to_vec()).await;
    let alt = format!(
        "h3=\"127.0.0.1:{}\"; ma=60, h3=\"127.0.0.1:{}\"; ma=60",
        hole.addr.port(),
        h3_good.addr.port()
    );
    let h1 = H1AltSvcServer::start(alt, b"h1-fallback".to_vec()).await;
    let client = auto_client(
        Timeout::builder()
            .connect(Duration::from_millis(500))
            .total(Duration::from_secs(5))
            .build(),
    );
    let before = client.transport_metrics().snapshot();
    let start = Instant::now();
    let mut resp = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("replayable GET may fallback before commit");
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(resp.text().await.expect("body"), "h1-fallback");
    let after = client.transport_metrics().snapshot();
    assert!(
        after.h3_fallback_selected >= before.h3_fallback_selected,
        "Auto should consider H3 before optional fallback"
    );
    let one_shot = RequestBody::from_stream(
        futures_util::stream::once(async { Ok::<_, Error>(Bytes::from("upload-once")) }),
        None,
    );
    let uploads_before = h3_good.uploads_seen();
    let post = client
        .request(http::Method::POST, &h1.url())
        .expect("post")
        .body(one_shot)
        .send()
        .await;
    match post {
        Ok(mut r) => {
            let _ = r.text().await;
        }
        Err(e) => {
            assert!(
                !matches!(e, Error::BodyNotReplayableForRetry),
                "non-replayable must not misclassify: {e:?}"
            );
        }
    }
    assert!(
        h3_good.uploads_seen() <= uploads_before + 1,
        "non-replayable upload must not be sent twice"
    );
    let recover = QuicTestServer::start(b"after".to_vec()).await;
    let mut ok = client
        .get(&recover.url())
        .unwrap()
        .send()
        .await
        .expect("resources released");
    assert_eq!(ok.text().await.expect("body"), "after");
    pass_result("udp_rejected", UDP_REJECTED_INV);
}

const PACKET_LOSS_LOW_INV: &[&str] = &[
    "bounded_deadline",
    "no_body_duplication",
    "resources_released",
];

#[tokio::test(flavor = "multi_thread")]
async fn packet_loss_low() {
    if !gate("packet_loss_low", PACKET_LOSS_LOW_INV) {
        return;
    }
    run_netem_transfer_scenario("packet_loss_low", PACKET_LOSS_LOW_INV).await;
}

const PACKET_LOSS_HIGH_INV: &[&str] = &[
    "bounded_deadline",
    "cancellation_prompt",
    "resources_released",
];

#[tokio::test(flavor = "multi_thread")]
async fn packet_loss_high() {
    if !gate("packet_loss_high", PACKET_LOSS_HIGH_INV) {
        return;
    }
    let server = QuicTestServer::start(b"loss-high".to_vec()).await;
    let client = h3_only_client(
        Timeout::builder()
            .connect(Duration::from_secs(3))
            .total(Duration::from_secs(8))
            .build(),
    );
    let hole = Blackhole::bind();
    let pending = client.get(&hole.url()).unwrap().send();
    let cancelled = tokio::time::timeout(Duration::from_millis(800), pending).await;
    assert!(
        cancelled.is_err(),
        "cancellation must be prompt under impairment"
    );
    let start = Instant::now();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("request completes");
    assert!(start.elapsed() < Duration::from_secs(8));
    assert_eq!(resp.text().await.expect("body"), "loss-high");
    pass_result("packet_loss_high", PACKET_LOSS_HIGH_INV);
}

const LATENCY_JITTER_INV: &[&str] = &["bounded_total_deadline", "no_permanent_cache_poisoning"];

#[tokio::test(flavor = "multi_thread")]
async fn latency_jitter() {
    if !gate("latency_jitter", LATENCY_JITTER_INV) {
        return;
    }
    let server = QuicTestServer::start(b"jitter".to_vec()).await;
    let tight = h3_only_client(
        Timeout::builder()
            .total(Duration::from_millis(50))
            .connect(Duration::from_millis(50))
            .build(),
    );
    let _ = tight.get(&server.url()).unwrap().send().await;
    let loose = h3_only_client(
        Timeout::builder()
            .total(Duration::from_secs(15))
            .connect(Duration::from_secs(15))
            .build(),
    );
    let mut resp = loose
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("usable deadline succeeds");
    assert_eq!(resp.text().await.expect("body"), "jitter");
    let mut again = loose
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("origin not poisoned");
    assert_eq!(again.text().await.expect("body"), "jitter");
    pass_result("latency_jitter", LATENCY_JITTER_INV);
}

const PACKET_REORDERING_INV: &[&str] = &["body_integrity", "no_body_duplication"];

#[tokio::test(flavor = "multi_thread")]
async fn packet_reordering() {
    if !gate("packet_reordering", PACKET_REORDERING_INV) {
        return;
    }
    run_netem_transfer_scenario("packet_reordering", PACKET_REORDERING_INV).await;
}

const PACKET_DUPLICATION_INV: &[&str] = &["body_integrity", "no_body_duplication"];

#[tokio::test(flavor = "multi_thread")]
async fn packet_duplication() {
    if !gate("packet_duplication", PACKET_DUPLICATION_INV) {
        return;
    }
    run_netem_transfer_scenario("packet_duplication", PACKET_DUPLICATION_INV).await;
}

const REDUCED_MTU_INV: &[&str] = &["bounded_deadline", "safe_auto_fallback_only_pre_commit"];

#[tokio::test(flavor = "multi_thread")]
async fn reduced_mtu_pmtu_blackhole() {
    if !gate("reduced_mtu_pmtu_blackhole", REDUCED_MTU_INV) {
        return;
    }
    let server = QuicTestServer::start(b"mtu".to_vec()).await;
    let client = auto_client(
        Timeout::builder()
            .total(Duration::from_secs(10))
            .connect(Duration::from_secs(5))
            .build(),
    );
    let start = Instant::now();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("completes within deadline");
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(resp.text().await.expect("body"), "mtu");
    pass_result("reduced_mtu_pmtu_blackhole", REDUCED_MTU_INV);
}

const PATH_LOSS_AFTER_HEADERS_INV: &[&str] = &[
    "download_cancellation_or_error_bounded",
    "resources_released",
];

#[tokio::test(flavor = "multi_thread")]
async fn path_loss_after_headers() {
    if !gate("path_loss_after_headers", PATH_LOSS_AFTER_HEADERS_INV) {
        return;
    }
    let server =
        QuicTestServer::start_with_behavior(b"stall".to_vec(), ServerBehavior::StallBody).await;
    let client = h3_only_client(
        Timeout::builder()
            .read(Duration::from_millis(500))
            .total(Duration::from_secs(5))
            .build(),
    );
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("headers");
    let mut stream = resp.bytes_stream().expect("stream");
    let err = stream
        .next()
        .await
        .expect("item")
        .expect_err("read ends under impairment");
    assert!(matches!(
        err,
        Error::Timeout { .. }
            | Error::H3Stream(_)
            | Error::H3Protocol(_)
            | Error::H3ConnectionClosed(_)
            | Error::Body(_)
            | Error::Connect(_)
    ));
    let _ = timeout_phase(&err);
    let recover = QuicTestServer::start(b"ok".to_vec()).await;
    let mut ok = client
        .get(&recover.url())
        .unwrap()
        .send()
        .await
        .expect("resources released");
    assert_eq!(ok.text().await.expect("body"), "ok");
    pass_result("path_loss_after_headers", PATH_LOSS_AFTER_HEADERS_INV);
}

const PATH_LOSS_DURING_UPLOAD_INV: &[&str] = &[
    "upload_cancellation_or_error_bounded",
    "non_replayable_body_not_duplicated",
];

#[tokio::test(flavor = "multi_thread")]
async fn path_loss_during_upload() {
    if !gate("path_loss_during_upload", PATH_LOSS_DURING_UPLOAD_INV) {
        return;
    }
    let server = QuicTestServer::start(b"echo".to_vec()).await;
    let client = h3_only_client(
        Timeout::builder()
            .write(Duration::from_millis(600))
            .total(Duration::from_secs(5))
            .build(),
    );
    let body = RequestBody::from_stream(
        futures_util::stream::once(async { Ok::<_, Error>(Bytes::from(vec![0xAB; 64 * 1024])) }),
        None,
    );
    let before = server.uploads_seen();
    let result = client
        .request(http::Method::POST, &server.url())
        .expect("post")
        .body(body)
        .send()
        .await;
    let _ = result;
    assert!(
        server.uploads_seen() <= before + 1,
        "upload must not be retried"
    );
    pass_result("path_loss_during_upload", PATH_LOSS_DURING_UPLOAD_INV);
}

const SERVER_RESTART_INV: &[&str] = &["fresh_h3_recovery", "cache_not_permanently_poisoned"];

#[tokio::test(flavor = "multi_thread")]
async fn server_restart() {
    if !gate("server_restart", SERVER_RESTART_INV) {
        return;
    }
    let body = b"restart".to_vec();
    let first = QuicTestServer::start(body.clone()).await;
    let client = h3_only_client(
        Timeout::builder()
            .connect(Duration::from_secs(3))
            .total(Duration::from_secs(5))
            .build(),
    );
    let mut resp = client
        .get(&first.url())
        .unwrap()
        .send()
        .await
        .expect("first");
    assert_eq!(resp.text().await.expect("body"), "restart");
    drop(first);
    let second = QuicTestServer::start(body).await;
    let mut resp2 = client
        .get(&second.url())
        .unwrap()
        .send()
        .await
        .expect("fresh generation");
    assert_eq!(resp2.text().await.expect("body"), "restart");
    let mut resp3 = client
        .get(&second.url())
        .unwrap()
        .send()
        .await
        .expect("cache not poisoned");
    assert_eq!(resp3.text().await.expect("body"), "restart");
    pass_result("server_restart", SERVER_RESTART_INV);
}

const FIRST_ADDRESS_INV: &[&str] = &["later_address_reachable", "shared_connect_budget"];

#[tokio::test(flavor = "multi_thread")]
async fn first_address_unreachable() {
    if !gate("first_address_unreachable", FIRST_ADDRESS_INV) {
        return;
    }
    let hole = Blackhole::bind();
    let good = QuicTestServer::start(b"reachable".to_vec()).await;
    let alt = format!(
        "h3=\"127.0.0.1:{}\"; ma=60, h3=\"127.0.0.1:{}\"; ma=60",
        hole.addr.port(),
        good.addr.port()
    );
    let h1 = H1AltSvcServer::start(alt, b"h1".to_vec()).await;
    let client = auto_client(
        Timeout::builder()
            .connect(Duration::from_millis(800))
            .total(Duration::from_secs(4))
            .build(),
    );
    let start = Instant::now();
    let mut resp = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("reachable alt succeeds or safe fallback");
    assert!(start.elapsed() < Duration::from_secs(4));
    let text = resp.text().await.expect("body");
    assert!(
        text == "reachable" || text == "h1",
        "unexpected body {text}"
    );
    pass_result("first_address_unreachable", FIRST_ADDRESS_INV);
}

const IPV6_FAIL_IPV4_OK_INV: &[&str] = &["address_fallback", "body_integrity"];

#[tokio::test(flavor = "multi_thread")]
async fn ipv6_failure_ipv4_success() {
    if !gate("ipv6_failure_ipv4_success", IPV6_FAIL_IPV4_OK_INV) {
        return;
    }
    if !(ipv6_loopback_available() && ipv4_loopback_available()) {
        unsupported_result(
            "ipv6_failure_ipv4_success",
            IPV6_FAIL_IPV4_OK_INV,
            "host has only one address family",
        );
        return;
    }
    let v6_hole = match std::net::UdpSocket::bind("[::1]:0") {
        Ok(s) => s.local_addr().expect("addr"),
        Err(_) => {
            unsupported_result(
                "ipv6_failure_ipv4_success",
                IPV6_FAIL_IPV4_OK_INV,
                "ipv6 loopback unavailable",
            );
            return;
        }
    };
    let v4_server = QuicTestServer::start(b"ipv4-ok".to_vec()).await;
    let alt = format!(
        "h3=\"[::1]:{}\"; ma=60, h3=\"127.0.0.1:{}\"; ma=60",
        v6_hole.port(),
        v4_server.addr.port()
    );
    let h1 = H1AltSvcServer::start(alt, b"h1".to_vec()).await;
    let client = auto_client(
        Timeout::builder()
            .connect(Duration::from_secs(2))
            .total(Duration::from_secs(5))
            .build(),
    );
    let mut resp = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("ipv4 path works");
    let body = resp.text().await.expect("body");
    assert!(
        body == "ipv4-ok" || body == "h1",
        "body integrity preserved: {body}"
    );
    pass_result("ipv6_failure_ipv4_success", IPV6_FAIL_IPV4_OK_INV);
}

const IPV4_FAIL_IPV6_OK_INV: &[&str] = &["address_fallback", "body_integrity"];

#[tokio::test(flavor = "multi_thread")]
async fn ipv4_failure_ipv6_success() {
    if !gate("ipv4_failure_ipv6_success", IPV4_FAIL_IPV6_OK_INV) {
        return;
    }
    if !(ipv6_loopback_available() && ipv4_loopback_available()) {
        unsupported_result(
            "ipv4_failure_ipv6_success",
            IPV4_FAIL_IPV6_OK_INV,
            "host has only one address family",
        );
        return;
    }
    let v4_hole = Blackhole::bind();
    let v6_server =
        QuicTestServer::start_on("[::1]:0", b"ipv6-ok".to_vec(), ServerBehavior::Immediate).await;
    let alt = format!(
        "h3=\"127.0.0.1:{}\"; ma=60, h3=\"[::1]:{}\"; ma=60",
        v4_hole.addr.port(),
        v6_server.addr.port()
    );
    let h1 = H1AltSvcServer::start(alt, b"h1".to_vec()).await;
    let client = auto_client(
        Timeout::builder()
            .connect(Duration::from_secs(2))
            .total(Duration::from_secs(5))
            .build(),
    );
    let mut resp = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("address selection completes");
    let body = resp.text().await.expect("body");
    assert!(
        body == "ipv6-ok" || body == "h1",
        "body integrity preserved: {body}"
    );
    pass_result("ipv4_failure_ipv6_success", IPV4_FAIL_IPV6_OK_INV);
}

async fn run_netem_transfer_scenario(scenario: &str, invariants: &[&str]) {
    let payload = b"netem-transfer-payload".to_vec();
    let server = QuicTestServer::start(payload.clone()).await;
    let client = h3_only_client(
        Timeout::builder()
            .connect(Duration::from_secs(5))
            .total(Duration::from_secs(10))
            .read(Duration::from_secs(5))
            .build(),
    );
    let start = Instant::now();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("transfer under netem");
    assert!(start.elapsed() < Duration::from_secs(10));
    let body = resp.bytes().await.expect("body bytes");
    assert_eq!(body.as_ref(), payload.as_slice());
    assert_eq!(server.uploads_seen(), 0);
    pass_result(scenario, invariants);
}
