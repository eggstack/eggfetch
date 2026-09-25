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
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args
)]
//! Hermetic Windows-TLS response-completeness reproducer and isolation matrix.
//!
//! Corrective fixture for `plans/implementation/core-transport-policy/006-`
//! `windows-tls-response-completeness-corrective.md` (WP1-WP6, required tests
//! 1-20). Uses only local loopback: a generated CA/server certificate and an
//! HTTP/1.1 TLS origin with configurable framing and shutdown semantics.
//!
//! Shutdown vocabulary used throughout:
//!
//! - **graceful**: the origin flushes the full response and performs an async
//!   TLS shutdown so rustls emits `close_notify` before the TCP close.
//! - **abrupt**: the origin flushes the full response bytes and then drops
//!   the TLS/TCP stream without `close_notify`.
//! - **keep-alive**: the origin keeps the TLS connection open after the full
//!   response; the client closes after consuming exactly `Content-Length`.
//! - **truncated**: the origin deliberately sends fewer than the declared
//!   `Content-Length` bytes and then closes abruptly. This must always error.
//!
//! Verdict recorded here (see the M006 closure record): on Linux all
//! graceful/abrupt-after-complete cases pass because the kernel delivers the
//! flushed bytes with FIN; the downstream Windows truncation (full
//! `write_all` at the TLS layer, short body observed, total response landing
//! near a 128 KiB transport boundary) is the fixture-teardown class from
//! plan §6.4 — a `write_all` + bare drop without `close_notify`/linger lets
//! the Windows transport discard the unsent tail. No production transport
//! change is made by this corrective.

#![cfg(feature = "tls-rustls")]

mod tls_fixtures;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{
    Client, DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer, Timeout,
    TlsConfig,
};
use futures_util::StreamExt;
use tls_fixtures::CertAuthority;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

const KIB: usize = 1024;
const BODY_64KIB: usize = 64 * KIB;
const BODY_128KIB: usize = 128 * KIB;
const BODY_256KIB: usize = 256 * KIB;
const BODY_300KIB: usize = 300 * KIB;
/// One transport chunk past the 128 KiB body mark.
const BODY_128KIB_PLUS_CHUNK: usize = 128 * KIB + 8192;
/// Slightly under 128 KiB so head + body straddles the boundary from below.
const BODY_128KIB_MINUS_HEADROOM: usize = 128 * KIB - 512;

/// Deterministic ASCII payload (`a..z` cycling, valid UTF-8 for `text()`).
fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| b'a' + (i % 26) as u8).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownMode {
    Graceful,
    Abrupt,
    KeepAlive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    FixedLength,
    Chunked,
    CloseDelimited,
}

#[derive(Debug, Clone, Copy)]
struct OriginOpts {
    body_len: usize,
    framing: Framing,
    shutdown: ShutdownMode,
    /// Deliberately send only this many body bytes while declaring the full
    /// `body_len`. `None` means send the complete body.
    truncate_to: Option<usize>,
}

impl OriginOpts {
    fn fixed(body_len: usize, shutdown: ShutdownMode) -> Self {
        Self {
            body_len,
            framing: Framing::FixedLength,
            shutdown,
            truncate_to: None,
        }
    }
}

struct CompletenessOrigin {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
    /// Response-head byte length of the last served response.
    head_len: Arc<std::sync::atomic::AtomicUsize>,
    /// Body bytes produced by the origin for the last response.
    produced: Arc<std::sync::atomic::AtomicUsize>,
}

impl CompletenessOrigin {
    async fn start(ca: &CertAuthority, opts: OriginOpts) -> Self {
        let (cert_der, key_der) = ca.generate_server_cert(&["localhost", "127.0.0.1"]);
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        Self::start_with_server_config(server_config, opts).await
    }

    async fn start_with_server_config(
        server_config: rustls::ServerConfig,
        opts: OriginOpts,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
        let head_len = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let produced = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let head_len_task = head_len.clone();
        let produced_task = produced.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((tcp_stream, _)) = result else { break };
                        let acceptor = acceptor.clone();
                        let head_len = head_len_task.clone();
                        let produced = produced_task.clone();
                        tokio::spawn(async move {
                            let tls_stream = match acceptor.accept(tcp_stream).await {
                                Ok(s) => s,
                                Err(_) => return,
                            };
                            serve_connection(tls_stream, opts, &head_len, &produced).await;
                        });
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        Self {
            port,
            shutdown_tx,
            head_len,
            produced,
        }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }

    fn head_len(&self) -> usize {
        self.head_len.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn produced(&self) -> usize {
        self.produced.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for CompletenessOrigin {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[allow(clippy::too_many_lines)]
async fn serve_connection(
    tls_stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    opts: OriginOpts,
    head_len: &Arc<std::sync::atomic::AtomicUsize>,
    produced: &Arc<std::sync::atomic::AtomicUsize>,
) {
    use std::sync::atomic::Ordering::SeqCst;
    let mut reader = BufReader::new(tls_stream);
    loop {
        // Read the request head.
        let mut request_line = String::new();
        match reader.read_line(&mut request_line).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if request_line.trim().is_empty() {
            return;
        }
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => return,
                Ok(_) if line.trim().is_empty() => break,
                Ok(_) => {}
            }
        }

        let body = payload(opts.body_len);
        let send_len = opts.truncate_to.unwrap_or(opts.body_len);
        let send_body = &body[..send_len.min(body.len())];

        match opts.framing {
            Framing::FixedLength => {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n",
                    opts.body_len,
                    if opts.shutdown == ShutdownMode::KeepAlive {
                        "keep-alive"
                    } else {
                        "close"
                    },
                );
                head_len.store(head.len(), SeqCst);
                let stream = reader.get_mut();
                if stream.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                if stream.write_all(send_body).await.is_err() {
                    return;
                }
                if stream.flush().await.is_err() {
                    return;
                }
                produced.store(send_len, SeqCst);
            }
            Framing::Chunked => {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\nConnection: {}\r\n\r\n",
                    if opts.shutdown == ShutdownMode::KeepAlive {
                        "keep-alive"
                    } else {
                        "close"
                    },
                );
                head_len.store(head.len(), SeqCst);
                let stream = reader.get_mut();
                if stream.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                // 16 KiB chunks; truncation cuts the chunk stream short.
                let mut remaining = send_len;
                let mut offset = 0;
                while remaining > 0 {
                    let n = remaining
                        .min(16 * KIB)
                        .min(send_body.len().saturating_sub(offset));
                    if n == 0 {
                        break;
                    }
                    let chunk_head = format!("{n:x}\r\n");
                    if stream.write_all(chunk_head.as_bytes()).await.is_err()
                        || stream
                            .write_all(&send_body[offset..offset + n])
                            .await
                            .is_err()
                        || stream.write_all(b"\r\n").await.is_err()
                    {
                        return;
                    }
                    offset += n;
                    remaining -= n;
                }
                let terminator = if opts.truncate_to.is_some() {
                    // Deliberate truncation: no terminating chunk.
                    &b""[..]
                } else {
                    &b"0\r\n\r\n"[..]
                };
                if stream.write_all(terminator).await.is_err() || stream.flush().await.is_err() {
                    return;
                }
                produced.store(send_len, SeqCst);
            }
            Framing::CloseDelimited => {
                let head =
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n";
                head_len.store(head.len(), SeqCst);
                let stream = reader.get_mut();
                if stream.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                if stream.write_all(send_body).await.is_err() {
                    return;
                }
                if stream.flush().await.is_err() {
                    return;
                }
                produced.store(send_len, SeqCst);
            }
        }

        match opts.shutdown {
            ShutdownMode::Graceful => {
                // Emit TLS `close_notify` so the peer sees a clean shutdown
                // even for large bodies whose TCP tail is still in flight.
                let mut stream = reader.into_inner();
                let _ = stream.shutdown().await;
                // Brief linger so the client can drain before the socket
                // fully closes (mirrors well-behaved origins).
                tokio::time::sleep(Duration::from_millis(200)).await;
                return;
            }
            ShutdownMode::Abrupt => {
                // Full bytes flushed, no `close_notify`: the teardown class
                // at the heart of the Windows investigation.
                return;
            }
            ShutdownMode::KeepAlive => {
                // Serve the next request on the same TLS connection.
            }
        }
    }
}

fn test_client(ca: &CertAuthority) -> Client {
    Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build()
}

/// Direct-connector client: a bound local address routes through the
/// specialized-direct connector instead of the standard Hyper path.
fn direct_client(ca: &CertAuthority) -> Client {
    Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .local_address("127.0.0.1:0".parse().unwrap())
        .build()
}

#[derive(Clone)]
struct PassthroughDialer {
    address: std::net::SocketAddr,
}

impl Dialer for PassthroughDialer {
    fn dial(&self, _target: DialTarget) -> DialFuture<'_> {
        let address = self.address;
        Box::pin(async move {
            let stream = tokio::net::TcpStream::connect(address)
                .await
                .map_err(|error| {
                    DialError::with_source(DialErrorKind::Connection, "dial failed", error)
                })?;
            Ok(Box::new(stream) as DialStream)
        })
    }
}

fn dialer_client(ca: &CertAuthority, address: std::net::SocketAddr) -> Client {
    Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .dialer(PassthroughDialer { address })
        .build()
}

fn rustls_client_config(ca: &CertAuthority) -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.cert_der()).unwrap();
    Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

/// Consume via buffered `bytes()` and report head/body accounting.
async fn fetch_bytes(url: &str, client: &Client) -> (usize, Vec<u8>) {
    let mut response = client.get(url).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let bytes = response.bytes().await.unwrap();
    (bytes.len(), bytes.to_vec())
}

// --- Required test 1: fixed-length HTTPS completes at 64 KiB (graceful). ---

#[tokio::test]
async fn fixed_https_completes_at_64kib_graceful() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::Graceful)).await;
    let client = test_client(&ca);
    let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
    assert_eq!(len, BODY_64KIB);
    assert_eq!(bytes, payload(BODY_64KIB));
    assert_eq!(origin.produced(), BODY_64KIB);
    origin.shutdown();
}

// --- Required test 2: around the 128 KiB total-response boundary. ---

#[tokio::test]
async fn fixed_https_completes_around_128kib_total_boundary() {
    let ca = CertAuthority::new();
    for body_len in [BODY_128KIB_MINUS_HEADROOM, BODY_128KIB] {
        let origin =
            CompletenessOrigin::start(&ca, OriginOpts::fixed(body_len, ShutdownMode::Graceful))
                .await;
        let client = test_client(&ca);
        let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
        assert_eq!(len, body_len, "body_len={body_len}");
        assert_eq!(bytes, payload(body_len));
        // The regression that motivated M006 landed at an apparent 128 KiB
        // *total-response* (head + body) boundary; record both sides.
        let total = origin.head_len() + origin.produced();
        assert!(
            total >= 128 * KIB - 1024,
            "expected total response near/above 128 KiB, got head={} body={}",
            origin.head_len(),
            origin.produced()
        );
        origin.shutdown();
    }
}

// --- Required test 3: above 128 KiB. ---

#[tokio::test]
async fn fixed_https_completes_above_128kib() {
    let ca = CertAuthority::new();
    let origin = CompletenessOrigin::start(
        &ca,
        OriginOpts::fixed(BODY_128KIB_PLUS_CHUNK, ShutdownMode::Graceful),
    )
    .await;
    let client = test_client(&ca);
    let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
    assert_eq!(len, BODY_128KIB_PLUS_CHUNK);
    assert_eq!(bytes, payload(BODY_128KIB_PLUS_CHUNK));
    origin.shutdown();
}

// --- Required test 4: at/above 256 KiB (incl. the 300 KiB envelope). ---

#[tokio::test]
async fn fixed_https_completes_at_and_above_256kib() {
    let ca = CertAuthority::new();
    for body_len in [BODY_256KIB, BODY_300KIB] {
        let origin =
            CompletenessOrigin::start(&ca, OriginOpts::fixed(body_len, ShutdownMode::Graceful))
                .await;
        let client = test_client(&ca);
        let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
        assert_eq!(len, body_len, "body_len={body_len}");
        assert_eq!(bytes, payload(body_len));
        origin.shutdown();
    }
}

// --- Required test 5: graceful TLS close case (explicit). ---

#[tokio::test]
async fn graceful_tls_close_delivers_complete_body() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_128KIB, ShutdownMode::Graceful))
            .await;
    let client = test_client(&ca);
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.len(), BODY_128KIB);
    origin.shutdown();
}

// --- Required test 6: abrupt TLS close after a complete body. ---
//
// On Linux the flushed bytes are delivered with FIN and Hyper completes the
// already-framed body. On Windows the same origin pattern (write_all + bare
// drop) can lose the unsent tail, which then — correctly — surfaces as a
// body error rather than a short success. This test pins the Linux half of
// that contract; the closure record carries the teardown verdict.

#[tokio::test]
async fn abrupt_tls_close_after_complete_body_still_completes() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_128KIB, ShutdownMode::Abrupt)).await;
    let client = test_client(&ca);
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.len(), BODY_128KIB);
    assert_eq!(bytes.as_ref(), payload(BODY_128KIB).as_slice());
    assert_eq!(origin.produced(), BODY_128KIB);
    origin.shutdown();
}

// --- Required test 7: deliberately truncated body still errors. ---

#[tokio::test]
async fn truncated_fixed_length_body_still_errors() {
    let ca = CertAuthority::new();
    let opts = OriginOpts {
        body_len: BODY_128KIB,
        framing: Framing::FixedLength,
        shutdown: ShutdownMode::Abrupt,
        truncate_to: Some(BODY_128KIB - 4096),
    };
    let origin = CompletenessOrigin::start(&ca, opts).await;
    let client = test_client(&ca);
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let err = response.bytes().await.unwrap_err();
    assert_eq!(
        err.kind(),
        "body",
        "truncation must stay a body error: {err}"
    );
    assert_eq!(origin.produced(), BODY_128KIB - 4096);
    origin.shutdown();
}

// --- Required test 8: keep-alive-after-response control. ---

#[tokio::test]
async fn keep_alive_after_response_control() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::KeepAlive))
            .await;
    let client = test_client(&ca);
    for _ in 0..2 {
        let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
        assert_eq!(len, BODY_64KIB);
        assert_eq!(bytes, payload(BODY_64KIB));
    }
    origin.shutdown();
}

// --- Required test 9: plain HTTP control. ---

#[tokio::test]
async fn plain_http_control_matches_https() {
    use tokio::io::AsyncReadExt;
    let body = payload(BODY_128KIB);
    let body_match = body.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1];
        let mut head = Vec::new();
        loop {
            stream.read_exact(&mut buf).await.unwrap();
            head.push(buf[0]);
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body_match.len()
        );
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(&body_match).await.unwrap();
        stream.flush().await.unwrap();
    });
    let client = Client::builder().build();
    let mut response = client
        .get(&format!("http://127.0.0.1:{port}/"))
        .unwrap()
        .send()
        .await
        .unwrap();
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), body.as_slice());
}

// --- Required test 10: chunked HTTPS control. ---

#[tokio::test]
async fn chunked_https_control_completes() {
    let ca = CertAuthority::new();
    let opts = OriginOpts {
        body_len: BODY_128KIB,
        framing: Framing::Chunked,
        shutdown: ShutdownMode::Graceful,
        truncate_to: None,
    };
    let origin = CompletenessOrigin::start(&ca, opts).await;
    let client = test_client(&ca);
    let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
    assert_eq!(len, BODY_128KIB);
    assert_eq!(bytes, payload(BODY_128KIB));
    origin.shutdown();
}

// --- Required tests 11-13: bytes()/text()/bytes_stream() agree. ---

#[tokio::test]
async fn bytes_text_and_stream_consumers_agree() {
    let ca = CertAuthority::new();
    let expected = payload(BODY_64KIB);

    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::Graceful)).await;
    let client = test_client(&ca);
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        expected.as_slice()
    );
    origin.shutdown();

    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::Graceful)).await;
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let text = response.text().await.unwrap();
    assert_eq!(text.as_bytes(), expected.as_slice());
    origin.shutdown();

    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::Graceful)).await;
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let mut stream = response.bytes_stream().unwrap();
    let mut collected = Vec::new();
    while let Some(chunk) = stream.next().await {
        collected.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(collected, expected);
    origin.shutdown();
}

// --- Required tests 14-15: direct connector and custom dialer agree. ---

#[tokio::test]
async fn direct_connector_and_custom_dialer_agree() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_128KIB, ShutdownMode::Graceful))
            .await;
    let expected = payload(BODY_128KIB);

    let direct = direct_client(&ca);
    let mut response = direct.get(&origin.url()).unwrap().send().await.unwrap();
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        expected.as_slice(),
        "direct connector path"
    );

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", origin.port).parse().unwrap();
    let dialer = dialer_client(&ca, addr);
    let mut response = dialer.get(&origin.url()).unwrap().send().await.unwrap();
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        expected.as_slice(),
        "custom dialer path"
    );
    origin.shutdown();
}

// --- Required test 16: raw tokio-rustls isolation probe (below Hyper). ---
//
// Reads the origin directly over tokio-rustls with no Hyper, timeout, or
// lease wrappers. Failure here would implicate rustls/the fixture boundary;
// success moves the boundary upward.

#[tokio::test]
async fn raw_tokio_rustls_probe_completes() {
    use tokio::io::AsyncReadExt;
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_128KIB, ShutdownMode::Graceful))
            .await;
    let connector = tokio_rustls::TlsConnector::from(rustls_client_config(&ca));
    let tcp = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", origin.port))
        .await
        .unwrap();
    let server_name = rustls::pki_types::ServerName::try_from("127.0.0.1".to_owned()).unwrap();
    let mut tls = connector.connect(server_name, tcp).await.unwrap();
    tls.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    tls.flush().await.unwrap();
    let mut raw = Vec::new();
    tls.read_to_end(&mut raw).await.unwrap();
    let head_end = find_head_end(&raw).expect("response head");
    let body = &raw[head_end..];
    assert_eq!(body.len(), BODY_128KIB, "raw TLS probe body length");
    assert_eq!(body, payload(BODY_128KIB).as_slice());
    origin.shutdown();
}

fn find_head_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

// --- Required test 17: minimal Hyper isolation probe (no EggFetch). ---
//
// A minimal hyper-util H1 client over the same TLS stream consumes the body
// without EggFetch `ResponseBody`, timeout, decompression, or lease
// wrappers. Success here with EggFetch success pins ownership away from
// Hyper; failure here would implicate Hyper/the IO adapter.

#[tokio::test]
async fn minimal_hyper_probe_completes() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_128KIB, ShutdownMode::Graceful))
            .await;
    let tls_config = rustls_client_config(&ca);
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config((*tls_config).clone())
        .https_only()
        .enable_http1()
        .build();
    let client: hyper_util::client::legacy::Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        http_body_util::Full<Bytes>,
    > = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(https);
    let uri: http::Uri = origin.url().parse().unwrap();
    let request = http::Request::builder()
        .method(http::Method::GET)
        .uri(uri)
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();
    let response = client.request(request).await.unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    let collected = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(collected.len(), BODY_128KIB);
    assert_eq!(collected.as_ref(), payload(BODY_128KIB).as_slice());
    origin.shutdown();
}

// --- Required test 18: timeout regression (read timeout still fires). ---

#[tokio::test]
async fn read_timeout_still_fires_on_stalled_body() {
    let ca = CertAuthority::new();
    let (cert_der, key_der) = ca.generate_server_cert(&["localhost", "127.0.0.1"]);
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    let Ok((tcp, _)) = result else { break };
                    let acceptor = acceptor.clone();
                    tokio::spawn(async move {
                        let Ok(tls) = acceptor.accept(tcp).await else { return };
                        let mut reader = BufReader::new(tls);
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.is_err() { return; }
                        loop {
                            let mut header = String::new();
                            match reader.read_line(&mut header).await {
                                Ok(_) if header.trim().is_empty() => break,
                                Ok(_) => {}
                                Err(_) => return,
                            }
                        }
                        let stream = reader.get_mut();
                        let head = "HTTP/1.1 200 OK\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n";
                        if stream.write_all(head.as_bytes()).await.is_err() { return; }
                        if stream.write_all(b"partial").await.is_err() { return; }
                        if stream.flush().await.is_err() { return; }
                        // Stall past the client read budget, then hold the
                        // connection open so the timeout (not EOF) fires.
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    });
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    });
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .timeout(Timeout {
            read: Some(Duration::from_millis(300)),
            ..Timeout::disabled()
        })
        .build();
    let mut response = client
        .get(&format!("https://127.0.0.1:{port}/"))
        .unwrap()
        .send()
        .await
        .unwrap();
    let err = response.bytes().await.unwrap_err();
    assert_eq!(
        err.kind(),
        "timeout_read",
        "read stall must stay a read timeout: {err}"
    );
    let _ = shutdown_tx.send(true);
}

// --- Required test 19: pool reuse after a successful complete body. ---

#[tokio::test]
async fn pool_reuses_connection_after_complete_body() {
    let ca = CertAuthority::new();
    let origin =
        CompletenessOrigin::start(&ca, OriginOpts::fixed(BODY_64KIB, ShutdownMode::KeepAlive))
            .await;
    let client = test_client(&ca);
    for _ in 0..2 {
        let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
        assert_eq!(response.bytes().await.unwrap().len(), BODY_64KIB);
    }
    origin.shutdown();
}

// --- Required test 20: no poisoned reuse after a truncation error. ---
//
// A truncated body must error without poisoning the pool: the next request
// on the same client succeeds.

#[tokio::test]
async fn no_reuse_after_truncation_error() {
    let ca = CertAuthority::new();
    // Keep-alive origin so the pool *could* reuse; the error path must not.
    let bad_opts = OriginOpts {
        body_len: BODY_64KIB,
        framing: Framing::FixedLength,
        shutdown: ShutdownMode::Abrupt,
        truncate_to: Some(1024),
    };
    let bad = CompletenessOrigin::start(&ca, bad_opts).await;
    let client = test_client(&ca);
    let mut first = client.get(&bad.url()).unwrap().send().await.unwrap();
    assert_eq!(first.bytes().await.unwrap_err().kind(), "body");
    let good = CompletenessOrigin::start(&ca, OriginOpts::fixed(2, ShutdownMode::KeepAlive)).await;
    let mut second = client.get(&good.url()).unwrap().send().await.unwrap();
    // Different origin port: proves the client still dispatches cleanly
    // after the truncation error instead of hanging on a dead pooled slot.
    assert_eq!(
        second.bytes().await.unwrap().as_ref(),
        payload(2).as_slice()
    );
    bad.shutdown();
    good.shutdown();
}

// --- Close-delimited control (framing matrix). ---
//
// Close-delimited framing makes EOF the terminator, so the origin MUST shut
// down gracefully: an abrupt close without TLS `close_notify` is
// indistinguishable from truncation and rustls/Hyper correctly report a body
// error. This pair pins both halves of that contract.

#[tokio::test]
async fn close_delimited_https_control_completes() {
    let ca = CertAuthority::new();
    let opts = OriginOpts {
        body_len: BODY_64KIB,
        framing: Framing::CloseDelimited,
        shutdown: ShutdownMode::Graceful,
        truncate_to: None,
    };
    let origin = CompletenessOrigin::start(&ca, opts).await;
    let client = test_client(&ca);
    let (len, bytes) = fetch_bytes(&origin.url(), &client).await;
    assert_eq!(len, BODY_64KIB);
    assert_eq!(bytes, payload(BODY_64KIB));
    origin.shutdown();
}

#[tokio::test]
async fn close_delimited_https_abrupt_close_errors() {
    let ca = CertAuthority::new();
    let opts = OriginOpts {
        body_len: BODY_64KIB,
        framing: Framing::CloseDelimited,
        shutdown: ShutdownMode::Abrupt,
        truncate_to: None,
    };
    let origin = CompletenessOrigin::start(&ca, opts).await;
    let client = test_client(&ca);
    let mut response = client.get(&origin.url()).unwrap().send().await.unwrap();
    let err = response.bytes().await.unwrap_err();
    assert_eq!(
        err.kind(),
        "body",
        "close-delimited abrupt close must stay a body error: {err}"
    );
    origin.shutdown();
}
