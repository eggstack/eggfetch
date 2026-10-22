//! Shared support for the lean/redirect outgoing-request equivalence fixture.
//!
//! The lean request path and the redirect request path are mutually
//! exclusive at compile time, so the fixture is split into two
//! feature-gated test targets. Both capture the outgoing request their path
//! puts on the wire and compare it against the single expectation in
//! [`expected_request`], which is what proves the two paths cannot diverge.
//!
//! The crate-internal fixture in `pipeline::hop` compares the *prepared
//! hop* — including the merged timeout and the transport hints, which are
//! not observable on the wire. This fixture complements it by comparing
//! the bytes that actually reach the server.
#![allow(dead_code)]
#![allow(
    clippy::missing_panics_doc,
    reason = "test-support helpers fail fast on an impossible fixture setup; they are not library API"
)]
// The file-level gate must come after the allow block: when `#![cfg(false)]`
// strips the crate, every attribute below it is stripped too, so an allow
// placed below the cfg would never apply.
#![cfg(feature = "high-level-url")]

use std::collections::BTreeMap;

use bytes::Bytes;
use eggfetch_core::auth::AuthScheme;
use eggfetch_core::headers::Headers;
use eggfetch_core::timeout::Timeout;
use eggfetch_core::transport_hints::TransportHints;
use eggfetch_core::{Client, RequestBuilder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The header names the fixture compares.
///
/// Transport-owned headers (`host`, `user-agent`, `accept-encoding`) are
/// deliberately excluded: they are added by the transport preparation
/// stage, not by the request preparation policy under test, and `host`
/// carries the ephemeral port of the capture server.
pub const COMPARED_HEADERS: &[&str] = &[
    "authorization",
    "content-length",
    "content-type",
    "x-custom",
    "x-default",
];

/// The normalized outgoing request captured from the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedRequest {
    /// The request method.
    pub method: String,
    /// The wire request target.
    pub target: String,
    /// The compared headers, lower-cased and sorted by name.
    pub headers: BTreeMap<String, String>,
    /// The request body.
    pub body: Vec<u8>,
}

/// The expected outgoing request for the fixture scenario.
pub fn expected_request() -> CapturedRequest {
    let mut headers = BTreeMap::new();
    headers.insert(
        "authorization".to_owned(),
        "Bearer request-token".to_owned(),
    );
    headers.insert("content-length".to_owned(), "5".to_owned());
    headers.insert("x-custom".to_owned(), "keep".to_owned());
    headers.insert("x-default".to_owned(), "d".to_owned());
    CapturedRequest {
        method: "POST".to_owned(),
        target: "/wire?q=1".to_owned(),
        headers,
        body: b"hello".to_vec(),
    }
}

/// The client used by the fixture scenario.
///
/// It carries a default header, a client-level auth scheme, and a
/// client-level timeout, so the default/request header merge, the auth
/// precedence rule, and the timeout merge are all exercised.
pub fn scenario_client() -> Client {
    let mut default_headers = Headers::new();
    default_headers
        .insert("x-default", "d")
        .expect("valid default header");
    Client::builder()
        .default_headers(default_headers)
        .auth(AuthScheme::bearer("client-token").expect("valid auth"))
        .timeout(Timeout {
            connect: Some(std::time::Duration::from_secs(1)),
            read: Some(std::time::Duration::from_secs(2)),
            total: Some(std::time::Duration::from_secs(3)),
            ..Timeout::default()
        })
        .build()
}

/// Apply the fixture scenario's request fields to a builder.
pub fn scenario_request(builder: RequestBuilder) -> RequestBuilder {
    builder
        .header("x-custom", "keep")
        .bytes("hello")
        .timeout(Timeout {
            write: Some(std::time::Duration::from_secs(5)),
            ..Timeout::default()
        })
        .auth(AuthScheme::bearer("request-token").expect("valid auth"))
        .transport_hints(TransportHints {
            target: Some(Bytes::from_static(b"/wire?q=1")),
            ..Default::default()
        })
}

/// Start a one-shot server that captures one request and answers `200 OK`.
///
/// Returns the URL to request and the accept task, which resolves to the
/// captured request.
pub async fn spawn_capture_server() -> (String, tokio::task::JoinHandle<CapturedRequest>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let address = listener.local_addr().expect("local address");
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept connection");
        let raw = read_request(&mut socket).await;
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await;
        let _ = socket.flush().await;
        parse_request(&raw)
    });
    (format!("http://{address}/"), handle)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(head_end) = head_end(&buffer) {
            let body_len = content_length(&buffer);
            if buffer.len() >= head_end + body_len {
                break;
            }
        }
        let read = socket.read(&mut chunk).await.expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    buffer
}

/// The offset just past the end of the request head, if it has arrived.
fn head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// The declared body length of the request head.
fn content_length(buffer: &[u8]) -> usize {
    let head = String::from_utf8_lossy(buffer);
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

/// Parse a raw HTTP/1.1 request into the normalized fixture shape.
fn parse_request(raw: &[u8]) -> CapturedRequest {
    let head_end = head_end(raw).expect("complete request head");
    let head = String::from_utf8_lossy(&raw[..head_end]);
    let mut lines = head.lines();
    let request_line = lines.next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("request method").to_owned();
    let target = parts.next().expect("request target").to_owned();

    let mut headers = BTreeMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if !COMPARED_HEADERS.contains(&name.as_str()) {
            continue;
        }
        headers.insert(name, value.trim().to_owned());
    }

    let body_len = content_length(raw);
    let body = raw[head_end..head_end + body_len].to_vec();

    CapturedRequest {
        method,
        target,
        headers,
        body,
    }
}
