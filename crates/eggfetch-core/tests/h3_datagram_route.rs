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
    clippy::map_unwrap_or,
    clippy::unused_async,
    clippy::never_loop,
    clippy::uninlined_format_args,
    clippy::too_many_arguments,
    clippy::useless_vec
)]
#![cfg(all(feature = "http3", feature = "tls-rustls"))]

//! Caller-owned fixed-target datagram routing for HTTP/3.
//!
//! Covers `plans/implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md`
//! at the public client level, using deterministic local fixtures only. The
//! carrier under test is a loopback UDP socket standing in for a future
//! CONNECT-UDP datagram stream: the library itself owns no concrete carrier,
//! so the carrier lives here in the harness.
//!
//! The control for "did this really use the caller's route?" is always the same
//! shape: the same QUIC server is reachable directly, so a request that
//! *succeeds* proves a direct socket was never opened on its behalf.
//!
//! Alt-Svc discovery only learns from a TLS-authenticated hop, so the Alt-Svc
//! case needs a real certificate authority rather than the
//! `danger_accept_invalid_certs` shortcut the other cases use.

mod tls_fixtures;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{
    Client, DatagramDialFuture, DatagramDialer, DatagramRecvFuture, DatagramRoute,
    DatagramSendFuture, DialError, DialErrorKind, DialTarget, Error, HttpVersionPolicy, Timeout,
    TlsConfig,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// QUIC (H3) server
// ---------------------------------------------------------------------------

/// A minimal QUIC server that answers every request with a fixed body.
struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

impl QuicTestServer {
    /// Synchronous: binding a loopback QUIC endpoint needs no await.
    fn start(body: Vec<u8>) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        Self::start_with_tls(cert_der, key_der, body)
    }

    /// Starts a server presenting a CA-signed certificate, required whenever a
    /// hop must count as TLS-authenticated for Alt-Svc learning.
    fn start_signed(
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
        body: Vec<u8>,
    ) -> Self {
        Self::start_with_tls(cert, key, body)
    }

    /// Synchronous: every fixture variant shares this constructor and none of
    /// them needs to await while binding.
    fn start_with_tls(
        cert_der: CertificateDer<'static>,
        key_der: PrivateKeyDer<'static>,
        body: Vec<u8>,
    ) -> Self {
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS version config")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("server TLS cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        server_tls.max_early_data_size = u32::MAX;

        let quic_crypto = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
            .expect("QUIC server config conversion");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));

        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind QUIC endpoint");
        let addr = endpoint.local_addr().expect("local addr");

        let body = Bytes::from(body);
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let mut server_conn = match h3::server::Connection::<_, Bytes>::new(h3_conn).await {
                                Ok(c) => c,
                                Err(_) => return,
                            };
                            loop {
                                let resolver = match server_conn.accept().await {
                                    Ok(Some(r)) => r,
                                    _ => break,
                                };
                                let body = body.clone();
                                let (_req, mut stream) = match resolver.resolve_request().await {
                                    Ok(rs) => rs,
                                    Err(_) => continue,
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .expect("response");
                                if stream.send_response(resp).await.is_err() { break; }
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        Self { addr, shutdown }
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

// ---------------------------------------------------------------------------
// HTTPS (H1) server that advertises Alt-Svc
// ---------------------------------------------------------------------------

/// HTTPS server that answers every request with `body` plus an optional
/// `Alt-Svc` header.
struct AltSvcH1Server {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

impl AltSvcH1Server {
    async fn start(
        alt_svc: Option<String>,
        body: &'static [u8],
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Self {
        let cert_der = cert;
        let key_der = key;
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("cert");

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((tcp, _)) = result else { break };
                        let acceptor = acceptor.clone();
                        let alt_svc = alt_svc.clone();
                        tokio::spawn(async move {
                            let Ok(tls) = acceptor.accept(tcp).await else { return };
                            let mut reader = BufReader::new(tls);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                                if line.trim().is_empty() { break; }
                                line.clear();
                            }
                            let alt_header = alt_svc
                                .as_ref()
                                .map(|v| format!("Alt-Svc: {v}\r\n"))
                                .unwrap_or_default();
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
                                body.len(),
                                alt_header,
                            );
                            let mut stream = reader.into_inner();
                            let _ = stream.write_all(resp.as_bytes()).await;
                            let _ = stream.write_all(body).await;
                            let _ = stream.flush().await;
                        });
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown }
    }

    /// Uses the certificate's name rather than the literal address, because a
    /// name is what a verifying client can validate.
    fn url(&self) -> String {
        format!("https://localhost:{}/", self.addr.port())
    }
}

impl Drop for AltSvcH1Server {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

// ---------------------------------------------------------------------------
// Caller-owned carrier route + dialer
// ---------------------------------------------------------------------------

/// Observable state shared between the harness and its routes.
#[derive(Default)]
struct CarrierProbe {
    dials: AtomicUsize,
    live_routes: AtomicUsize,
    observed_targets: Mutex<Vec<(String, u16)>>,
    /// Route id whose receive side should terminate, or 0 for none.
    kill_route: AtomicUsize,
}

impl CarrierProbe {
    fn dials(&self) -> usize {
        self.dials.load(Ordering::Relaxed)
    }

    fn live_routes(&self) -> usize {
        self.live_routes.load(Ordering::Relaxed)
    }

    fn targets(&self) -> Vec<(String, u16)> {
        self.observed_targets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// How a carrier route should misbehave, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CarrierFault {
    None,
    /// Refuse to carry datagrams larger than this many bytes.
    RejectOversize(usize),
    /// Delay every receive, to exercise bounded-queue back-pressure.
    SlowRecv,
}

/// One origin's physical carrier configuration.
#[derive(Clone, Copy, Debug)]
struct RoutePlan {
    peer: SocketAddr,
    fault: CarrierFault,
}

impl RoutePlan {
    fn direct(peer: SocketAddr) -> Self {
        Self {
            peer,
            fault: CarrierFault::None,
        }
    }
}

/// A loopback UDP carrier standing in for a CONNECT-UDP datagram stream.
///
/// The library owns no concrete carrier by design, so the seam's test double is
/// deliberately dumb: one fixed peer, one datagram per call, and no QUIC
/// awareness at all.
struct CarrierRoute {
    socket: UdpSocket,
    peer: SocketAddr,
    fault: CarrierFault,
    /// Distinguishes this route from later replacements of the same origin.
    route_id: usize,
    probe: Arc<CarrierProbe>,
}

impl CarrierRoute {
    async fn bind(plan: RoutePlan, route_id: usize, probe: Arc<CarrierProbe>) -> Arc<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind carrier");
        probe.live_routes.fetch_add(1, Ordering::Relaxed);
        Arc::new(Self {
            socket,
            peer: plan.peer,
            fault: plan.fault,
            route_id,
            probe,
        })
    }
}

impl Drop for CarrierRoute {
    fn drop(&mut self) {
        self.probe.live_routes.fetch_sub(1, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for CarrierRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately "leaks" a marker so tests can prove the bridge never
        // renders a provider's Debug output.
        f.debug_struct("CarrierRoute")
            .field("carrier_secret_marker", &"SHOULD-NEVER-ESCAPE")
            .finish()
    }
}

impl DatagramRoute for CarrierRoute {
    fn send<'a>(&'a self, payload: &'a [u8]) -> DatagramSendFuture<'a> {
        Box::pin(async move {
            if let CarrierFault::RejectOversize(limit) = self.fault {
                if payload.len() > limit {
                    return Err(DialError::new(
                        DialErrorKind::Connection,
                        "carrier cannot carry this datagram",
                    ));
                }
            }
            self.socket
                .send_to(payload, self.peer)
                .await
                .map(|_| ())
                .map_err(|e| {
                    DialError::new(DialErrorKind::Connection, format!("carrier send: {e}"))
                })
        })
    }

    fn recv<'a>(&'a self, buffer: &'a mut [u8]) -> DatagramRecvFuture<'a> {
        Box::pin(async move {
            if let CarrierFault::SlowRecv = self.fault {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            // Scoped to this exact route id so a replacement route for the same
            // origin is unaffected.
            if self.route_id == self.probe.kill_route.load(Ordering::Relaxed) {
                return Err(DialError::new(
                    DialErrorKind::Connection,
                    "carrier receive terminated",
                ));
            }
            let (len, _source) = self.socket.recv_from(buffer).await.map_err(|e| {
                DialError::new(DialErrorKind::Connection, format!("carrier recv: {e}"))
            })?;
            Ok(len)
        })
    }

    fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().expect("carrier local addr")
    }

    fn peer_addr(&self) -> SocketAddr {
        self.peer
    }
}

/// The client-scoped dialer under test.
///
/// Carrier selection is keyed on the *origin* port, which is what lets one test
/// prove that a connection-level failure evicts only its own generation.
struct CarrierDialer {
    plans: HashMap<u16, RoutePlan>,
    default_plan: Option<RoutePlan>,
    probe: Arc<CarrierProbe>,
    /// When set, `connect` fails with this kind instead of returning a route.
    fail_with: Option<DialErrorKind>,
    /// When true, `connect` never resolves, so only the deadline can end it.
    stall: bool,
}

impl CarrierDialer {
    /// Every origin port routes to the same peer.
    fn new(peer: SocketAddr, probe: Arc<CarrierProbe>) -> Self {
        Self {
            plans: HashMap::new(),
            default_plan: Some(RoutePlan::direct(peer)),
            probe,
            fail_with: None,
            stall: false,
        }
    }

    /// Per-origin carrier selection.
    fn per_origin(probe: Arc<CarrierProbe>) -> Self {
        Self {
            plans: HashMap::new(),
            default_plan: None,
            probe,
            fail_with: None,
            stall: false,
        }
    }

    fn with_peer(mut self, origin_port: u16, peer: SocketAddr) -> Self {
        self.plans.insert(origin_port, RoutePlan::direct(peer));
        self
    }

    fn with_fault(mut self, fault: CarrierFault) -> Self {
        let plan = self.default_plan.expect("default plan");
        self.default_plan = Some(RoutePlan {
            peer: plan.peer,
            fault,
        });
        self
    }

    fn with_fault_for(mut self, origin_port: u16, fault: CarrierFault) -> Self {
        let peer = self.plans[&origin_port].peer;
        self.plans.insert(origin_port, RoutePlan { peer, fault });
        self
    }

    fn failing(mut self, kind: DialErrorKind) -> Self {
        self.fail_with = Some(kind);
        self
    }

    fn stalled(mut self) -> Self {
        self.stall = true;
        self
    }

    fn plan_for(&self, port: u16) -> RoutePlan {
        self.plans
            .get(&port)
            .copied()
            .or(self.default_plan)
            .expect("carrier plan for origin port")
    }
}

impl DatagramDialer for CarrierDialer {
    fn connect(&self, target: DialTarget) -> DatagramDialFuture<'_> {
        self.probe
            .observed_targets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((target.host().to_owned(), target.port()));
        let route_id = self.probe.dials.fetch_add(1, Ordering::Relaxed) + 1;

        if self.stall {
            return Box::pin(futures_util::future::pending());
        }
        if let Some(kind) = self.fail_with {
            return Box::pin(async move { Err(DialError::new(kind, "carrier refused")) });
        }
        let plan = self.plan_for(target.port());
        let probe = Arc::clone(&self.probe);
        Box::pin(async move {
            Ok(CarrierRoute::bind(plan, route_id, probe).await as Arc<dyn DatagramRoute>)
        })
    }
}

// ---------------------------------------------------------------------------
// Client helpers
// ---------------------------------------------------------------------------

fn tls_config() -> TlsConfig {
    TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build()
}

fn routed_client(dialer: impl DatagramDialer) -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config())
        .automatic_decompression(false)
        .datagram_dialer(dialer)
        .build()
}

fn direct_client() -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config())
        .automatic_decompression(false)
        .build()
}

/// Asserts the error is connect-class, which is what makes a pre-commit route
/// failure safe for `Auto` fallback and Alt-Svc suppression.
fn assert_connect_class(error: &Error) {
    let rendered = format!("{error}");
    assert!(
        matches!(
            error,
            Error::Connect(_) | Error::H3Connect(_) | Error::Timeout { .. }
        ),
        "expected a connect-class failure, got {rendered}"
    );
}

/// Recovers the caller's broad [`DialErrorKind`] from a preserved
/// custom-transport error. Matched structurally, never parsed from text.
fn broad_kind(error: &Error) -> Option<DialErrorKind> {
    match error {
        Error::CustomTransport(dial_error) => Some(dial_error.kind()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests: the seam works and reuses generations
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn explicit_h3_request_completes_over_a_caller_route() {
    let server = QuicTestServer::start(b"routed h3".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(CarrierDialer::new(server.addr, Arc::clone(&probe)));

    let mut resp = client.get(&server.url()).unwrap().send().await.expect("H3");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.expect("body"), "routed h3");

    assert_eq!(probe.dials(), 1, "one generation means one route");
    assert_eq!(
        probe.targets(),
        vec![("127.0.0.1".to_owned(), server.addr.port())],
        "the dialer receives the original logical origin target"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn one_origin_reuses_a_single_caller_route_generation() {
    let server = QuicTestServer::start(b"reuse".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(CarrierDialer::new(server.addr, Arc::clone(&probe)));

    for _ in 0..3 {
        let mut resp = client.get(&server.url()).unwrap().send().await.expect("H3");
        assert_eq!(resp.text().await.expect("body"), "reuse");
    }
    assert_eq!(
        probe.dials(),
        1,
        "a cached H3 generation must reuse its caller route"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn alt_svc_h3_uses_the_same_caller_route_seam() {
    let ca = tls_fixtures::CertAuthority::new();
    let (cert, key) = ca.generate_server_cert(&["localhost"]);
    let h3 = QuicTestServer::start_signed(cert, key, b"alt h3".to_vec());
    let alt = format!("h3=\"127.0.0.1:{}\"; ma=60", h3.addr.port());
    let (cert, key) = ca.generate_server_cert(&["localhost"]);
    let h1 = AltSvcH1Server::start(Some(alt), b"alt h1", cert, key).await;

    let probe = Arc::new(CarrierProbe::default());
    let tls = TlsConfig::builder()
        .ca_certificate_pem(&ca.cert_pem())
        .expect("ca")
        .build();
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: true })
        .tls_config(tls)
        .automatic_decompression(false)
        .datagram_dialer(CarrierDialer::new(h3.addr, Arc::clone(&probe)))
        .build();

    let mut first = client.get(&h1.url()).unwrap().send().await.expect("H1");
    assert_eq!(first.text().await.expect("body"), "alt h1");
    assert_eq!(
        probe.dials(),
        0,
        "the TCP request must not touch the datagram seam"
    );

    let before = client.transport_metrics().snapshot();
    let mut second = client.get(&h1.url()).unwrap().send().await.expect("H3");
    let body = second.text().await.expect("body");
    let after = client.transport_metrics().snapshot();

    assert!(
        after.h3_route_attempted > before.h3_route_attempted,
        "the discovered Alt-Svc route must be attempted"
    );
    assert_eq!(
        probe.dials(),
        1,
        "Alt-Svc H3 goes through the same caller route seam"
    );
    assert_eq!(body, "alt h3", "Alt-Svc H3 must not fall back: {after:?}");

    let diagnostics = client.transport_metrics().h3_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.route, eggfetch_core::H3RouteKind::AltSvc);
    assert_eq!(diagnostic.alt_svc_generation, Some(1));
    assert_eq!(
        diagnostic.remote_address, h3.addr,
        "path metadata reports the route peer, not the origin"
    );
}

// ---------------------------------------------------------------------------
// Tests: failure classification and the absence of a direct fallback
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_route_never_falls_back_to_a_direct_socket() {
    let server = QuicTestServer::start(b"never-used".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client =
        routed_client(CarrierDialer::new(server.addr, probe).failing(DialErrorKind::Connection));

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("a refused route must not succeed");

    // The control: the very same URL is reachable over a direct socket. If the
    // client had silently opened one, this request would have succeeded.
    let mut control = direct_client();
    let mut control_resp = control
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("direct control must succeed");
    assert_eq!(control_resp.text().await.expect("body"), "never-used");

    assert_connect_class(&error);
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_provider_rejection_preserves_the_broad_kind() {
    let server = QuicTestServer::start(b"unreachable".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client =
        routed_client(CarrierDialer::new(server.addr, probe).failing(DialErrorKind::Rejected));

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("rejection must fail the request");
    assert_eq!(broad_kind(&error), Some(DialErrorKind::Rejected));
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_authentication_failure_preserves_the_broad_kind() {
    let server = QuicTestServer::start(b"unreachable".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(
        CarrierDialer::new(server.addr, probe).failing(DialErrorKind::Authentication),
    );

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("authentication failure must fail the request");
    assert_eq!(broad_kind(&error), Some(DialErrorKind::Authentication));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stalled_route_dial_is_bounded_by_the_connect_budget() {
    let server = QuicTestServer::start(b"unreachable".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let connect_timeout = Duration::from_millis(400);
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config())
        .automatic_decompression(false)
        .timeout(Timeout::builder().connect(connect_timeout).build())
        .datagram_dialer(CarrierDialer::new(server.addr, probe).stalled())
        .build();

    let started = std::time::Instant::now();
    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("a stalled route cannot complete");
    let elapsed = started.elapsed();

    assert_connect_class(&error);
    assert!(
        elapsed < Duration::from_secs(5),
        "the route dial must be bounded by the connect budget, took {elapsed:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_route_that_cannot_carry_quic_payloads_fails_establishment() {
    let server = QuicTestServer::start(b"too-big".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config())
        .automatic_decompression(false)
        .timeout(Timeout::builder().connect(Duration::from_secs(5)).build())
        // A QUIC Initial is far larger than this, so the very first handshake
        // datagram is refused and the generation never opens a stream.
        .datagram_dialer(
            CarrierDialer::new(server.addr, probe).with_fault(CarrierFault::RejectOversize(200)),
        )
        .build();

    let started = std::time::Instant::now();
    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("an undersized carrier must not establish");
    let elapsed = started.elapsed();

    assert_connect_class(&error);
    assert!(
        elapsed < Duration::from_secs(5),
        "an undersized carrier must fail establishment rather than wait, took {elapsed:?}"
    );

    let mut control = direct_client();
    let mut control_resp = control
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("the server itself is healthy");
    assert_eq!(control_resp.text().await.expect("body"), "too-big");
}

#[tokio::test(flavor = "multi_thread")]
async fn terminal_route_io_evicts_only_the_current_generation() {
    let first = QuicTestServer::start(b"first".to_vec());
    let second = QuicTestServer::start(b"second".to_vec());

    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(
        CarrierDialer::per_origin(Arc::clone(&probe))
            .with_peer(first.addr.port(), first.addr)
            .with_peer(second.addr.port(), second.addr),
    );

    let mut ok = client.get(&first.url()).unwrap().send().await.expect("H3");
    assert_eq!(ok.text().await.expect("body"), "first");
    let mut ok = client.get(&second.url()).unwrap().send().await.expect("H3");
    assert_eq!(ok.text().await.expect("body"), "second");
    assert_eq!(probe.dials(), 2, "one route per origin");
    let first_route_id = 1;

    // Terminate the receive side of the *first* origin's live route only.
    probe.kill_route.store(first_route_id, Ordering::Relaxed);

    // A terminal route I/O error is discovered lazily: the request riding the
    // doomed generation is the one that observes the failure, and that
    // observation is what evicts it. So the origin needs at most one further
    // attempt, which must succeed over a freshly dialed route.
    let mut recovered = None;
    for _ in 0..3 {
        match client.get(&first.url()).unwrap().send().await {
            Ok(mut resp) => {
                assert_eq!(resp.text().await.expect("body"), "first");
                recovered = Some(());
                break;
            }
            Err(error) => assert_connect_class(&error),
        }
    }
    assert!(
        recovered.is_some(),
        "a fresh route must recover the killed origin"
    );
    assert_eq!(
        probe.dials(),
        3,
        "a connection-level route failure evicts its own generation exactly once"
    );

    // The unrelated origin's generation was never touched, so it still reuses
    // its existing route instead of dialing again.
    let mut untouched = client.get(&second.url()).unwrap().send().await.expect("H3");
    assert_eq!(untouched.text().await.expect("body"), "second");
    assert_eq!(
        probe.dials(),
        3,
        "an unrelated origin must keep its cached generation"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn route_targets_are_the_logical_origins_not_the_carriers() {
    let first = QuicTestServer::start(b"one".to_vec());
    let second = QuicTestServer::start(b"two".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(
        CarrierDialer::per_origin(Arc::clone(&probe))
            .with_peer(first.addr.port(), first.addr)
            .with_peer(second.addr.port(), second.addr),
    );

    let _ = client.get(&first.url()).unwrap().send().await.expect("H3");
    let _ = client.get(&second.url()).unwrap().send().await.expect("H3");

    let targets = probe.targets();
    assert_eq!(targets.len(), 2);
    for (host, port) in &targets {
        assert_eq!(host, "127.0.0.1", "origin identity, not a relay name");
        assert!([first.addr.port(), second.addr.port()].contains(port));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_route_failures_suppress_the_broken_alt_svc_route() {
    let ca = tls_fixtures::CertAuthority::new();
    let (cert, key) = ca.generate_server_cert(&["localhost"]);
    let h3 = QuicTestServer::start_signed(cert, key, b"alt h3".to_vec());
    let alt = format!("h3=\"127.0.0.1:{}\"; ma=60", h3.addr.port());
    let (cert, key) = ca.generate_server_cert(&["localhost"]);
    let h1 = AltSvcH1Server::start(Some(alt), b"alt h1", cert, key).await;

    let probe = Arc::new(CarrierProbe::default());
    let tls = TlsConfig::builder()
        .ca_certificate_pem(&ca.cert_pem())
        .expect("ca")
        .build();
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: true })
        .tls_config(tls)
        .automatic_decompression(false)
        .datagram_dialer(CarrierDialer::new(h3.addr, probe).failing(DialErrorKind::Connection))
        .build();

    let mut first = client.get(&h1.url()).unwrap().send().await.expect("H1");
    assert_eq!(first.text().await.expect("body"), "alt h1");

    // A pre-commit route failure is connect-class, so the pipeline may fall
    // back to H1 within the same logical attempt.
    let before = client.transport_metrics().snapshot();
    let mut second = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("fallback");
    assert_eq!(second.text().await.expect("body"), "alt h1");
    let after_fallback = client.transport_metrics().snapshot();
    assert!(
        after_fallback.h3_route_attempted > before.h3_route_attempted,
        "the broken route must be attempted before it is judged"
    );

    // The same failure again must not keep paying for the attempt: the broken
    // alternative is suppressed and the request goes straight to H1.
    let mut third = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("suppressed");
    assert_eq!(third.text().await.expect("body"), "alt h1");
    let after_suppression = client.transport_metrics().snapshot();
    assert_eq!(
        after_suppression.h3_route_attempted, after_fallback.h3_route_attempted,
        "a repeated pre-commit route failure must suppress the alternative"
    );
    assert!(
        after_suppression.h3_route_suppressed > after_fallback.h3_route_suppressed,
        "suppression must be recorded for a repeated route failure"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn http3_only_never_selects_a_tcp_fallback_for_a_route_failure() {
    let server = QuicTestServer::start(b"strict".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = routed_client(
        CarrierDialer::new(server.addr, Arc::clone(&probe)).failing(DialErrorKind::Connection),
    );

    let before = client.transport_metrics().snapshot();
    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("strict H3 must not fall back to H1/H2");
    let after = client.transport_metrics().snapshot();

    assert_connect_class(&error);
    assert_eq!(
        after.h3_fallback_selected, before.h3_fallback_selected,
        "Http3Only must not select a TCP fallback"
    );
    assert_eq!(
        probe.dials(),
        1,
        "the request must have been attempted over the caller's route"
    );
}

// ---------------------------------------------------------------------------
// Tests: ownership, lifetime, back-pressure, and redaction
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_client_releases_every_route() {
    let server = QuicTestServer::start(b"drop-accounting".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    {
        let client = routed_client(CarrierDialer::new(server.addr, Arc::clone(&probe)));
        let mut resp = client.get(&server.url()).unwrap().send().await.expect("H3");
        assert_eq!(resp.text().await.expect("body"), "drop-accounting");
        assert_eq!(probe.live_routes(), 1);
    }

    // The route (and through it the bridge workers) must not outlive the
    // client that owns the generation.
    for _ in 0..200 {
        if probe.live_routes() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        probe.live_routes(),
        0,
        "dropping the client must release the caller route"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_carrier_backpressures_instead_of_dropping_datagrams() {
    // 512 KiB is far more than either bridge queue can hold, so a lossy
    // receive queue would corrupt or truncate the body here.
    let body: Vec<u8> = (0..(512 * 1024u32)).map(|i| (i % 251) as u8).collect();
    let expected = Bytes::from(body.clone());
    let server = QuicTestServer::start(body);
    let probe = Arc::new(CarrierProbe::default());
    let client =
        routed_client(CarrierDialer::new(server.addr, probe).with_fault(CarrierFault::SlowRecv));

    let mut resp = client.get(&server.url()).unwrap().send().await.expect("H3");
    let actual = resp.bytes().await.expect("body");
    assert_eq!(
        actual.len(),
        expected.len(),
        "a saturated receive queue must back-pressure, never truncate"
    );
    assert!(
        actual == expected,
        "response body must arrive byte-for-byte"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn route_debug_output_never_escapes_into_errors() {
    let server = QuicTestServer::start(b"redaction".to_vec());
    let probe = Arc::new(CarrierProbe::default());
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config())
        .automatic_decompression(false)
        .timeout(Timeout::builder().connect(Duration::from_secs(5)).build())
        .datagram_dialer(
            CarrierDialer::new(server.addr, probe).with_fault(CarrierFault::RejectOversize(200)),
        )
        .build();

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("an undersized carrier must fail");

    let rendered = format!("{error} {error:?}");
    assert!(
        !rendered.contains("SHOULD-NEVER-ESCAPE"),
        "provider Debug output must never reach an error: {rendered}"
    );
    assert!(
        !rendered.contains("CarrierRoute"),
        "provider type names must never reach an error: {rendered}"
    );
}
