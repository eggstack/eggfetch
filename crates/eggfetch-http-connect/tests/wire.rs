//! CONNECT wire primitive tests (plan workstream 6.1-6.3).

use eggfetch_http_connect::{
    basic_auth_value, encode_connect_request, read_connect_response_head, ConnectRequest,
    ConnectResponseLimits, ConnectTarget,
};
use tokio::io::{AsyncWriteExt, BufReader};

// ── Target ───────────────────────────────────────────────────────────────

#[test]
fn target_domain_authority() {
    let target = ConnectTarget::new("example.com", 443).unwrap();
    assert_eq!(target.authority(), "example.com:443");
}

#[test]
fn target_ipv4_authority() {
    let target = ConnectTarget::new("127.0.0.1", 443).unwrap();
    assert_eq!(target.authority(), "127.0.0.1:443");
}

#[test]
fn target_ipv6_authority() {
    let target = ConnectTarget::new("::1", 443).unwrap();
    assert_eq!(target.authority(), "[::1]:443");
    let bracketed = ConnectTarget::new("[::1]", 443).unwrap();
    assert_eq!(bracketed.authority(), "[::1]:443");
    assert_eq!(
        ConnectTarget::new("2001:db8::1", 8080).unwrap().authority(),
        "[2001:db8::1]:8080"
    );
}

#[test]
fn target_explicit_non_default_port() {
    let target = ConnectTarget::new("example.com", 8443).unwrap();
    assert_eq!(target.authority(), "example.com:8443");
}

#[test]
fn target_empty_rejected() {
    assert!(ConnectTarget::new("", 443).is_err());
}

#[test]
fn target_cr_lf_rejected() {
    assert!(ConnectTarget::new("example.com\r\nEvil: 1", 443).is_err());
    assert!(ConnectTarget::new("exam\rple.com", 443).is_err());
    assert!(ConnectTarget::new("exam\nple.com", 443).is_err());
}

#[test]
fn target_control_rejected() {
    assert!(ConnectTarget::new("example.com\x00", 443).is_err());
    assert!(ConnectTarget::new("example.com\x1f", 443).is_err());
    assert!(ConnectTarget::new("example.com\x7f", 443).is_err());
    assert!(ConnectTarget::new("example .com", 443).is_err());
}

// ── Request ──────────────────────────────────────────────────────────────

fn encode(
    host: &str,
    port: u16,
    auth: Option<&str>,
    extra: &[(String, Vec<u8>)],
    max: usize,
) -> Result<Vec<u8>, eggfetch_http_connect::ConnectError> {
    let target = ConnectTarget::new(host, port).unwrap();
    let request = ConnectRequest {
        target: &target,
        proxy_authorization: auth,
        extra_headers: extra,
        max_head_bytes: max,
    };
    encode_connect_request(&request)
}

#[test]
fn request_wire_shape() {
    let bytes = encode("example.com", 443, None, &[], 65_536).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(
        text,
        "CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n"
    );
}

#[test]
fn request_ipv6_bracketed() {
    let bytes = encode("::1", 8080, None, &[], 65_536).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with("CONNECT [::1]:8080 HTTP/1.1\r\n"));
    assert!(text.contains("Host: [::1]:8080\r\n"));
}

#[test]
fn request_basic_auth_exact_encoding() {
    let value = basic_auth_value("user", "pass").unwrap();
    assert_eq!(value, "Basic dXNlcjpwYXNz");
    let bytes = encode("example.com", 443, Some(&value), &[], 65_536).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"));
}

#[test]
fn request_credential_controls_rejected() {
    assert!(basic_auth_value("us:er", "pass").is_err());
    assert!(basic_auth_value("user\r", "pass").is_err());
    assert!(basic_auth_value("user", "pa\nss").is_err());
    assert!(basic_auth_value("user\x00", "pass").is_err());
    assert!(basic_auth_value("user", "pass\x7f").is_err());
}

#[test]
fn request_proxy_only_headers_serialized_once() {
    let extra = vec![("X-Custom-Proxy".to_owned(), b"value".to_vec())];
    let bytes = encode("example.com", 443, None, &extra, 65_536).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("X-Custom-Proxy: value").count(), 1);
}

#[test]
fn request_host_override_rejected() {
    let extra = vec![("Host".to_owned(), b"evil.example:443".to_vec())];
    assert!(encode("example.com", 443, None, &extra, 65_536).is_err());
    let extra_lower = vec![("host".to_owned(), b"evil.example:443".to_vec())];
    assert!(encode("example.com", 443, None, &extra_lower, 65_536).is_err());
}

#[test]
fn request_duplicate_auth_prevented() {
    let value = basic_auth_value("user", "pass").unwrap();
    let extra = vec![("Proxy-Authorization".to_owned(), b"Basic b3RoZXI=".to_vec())];
    assert!(encode("example.com", 443, Some(&value), &extra, 65_536).is_err());
    let two = vec![
        ("Proxy-Authorization".to_owned(), b"Basic YQ==".to_vec()),
        ("proxy-authorization".to_owned(), b"Basic Yg==".to_vec()),
    ];
    assert!(encode("example.com", 443, None, &two, 65_536).is_err());
}

#[test]
fn request_head_limit_exact_and_overflow() {
    let bytes = encode("example.com", 443, None, &[], 65_536).unwrap();
    let exact = bytes.len();
    assert!(encode("example.com", 443, None, &[], exact).is_ok());
    assert!(encode("example.com", 443, None, &[], exact - 1).is_err());
}

#[test]
fn request_header_injection_rejected() {
    let bad_name = vec![("X-Bad\r\nEvil".to_owned(), b"value".to_vec())];
    assert!(encode("example.com", 443, None, &bad_name, 65_536).is_err());
    let bad_value = vec![("X-Ok".to_owned(), b"a\r\nB: c".to_vec())];
    assert!(encode("example.com", 443, None, &bad_value, 65_536).is_err());
}

// ── Response ─────────────────────────────────────────────────────────────

async fn parse(
    raw: &[u8],
    limits: &ConnectResponseLimits,
) -> Result<eggfetch_http_connect::ConnectResponseHead, eggfetch_http_connect::ConnectError> {
    let stream = tokio::io::duplex(65_536);
    let (client, mut server) = stream;
    server.write_all(raw).await.unwrap();
    drop(server);
    let mut reader = BufReader::new(client);
    read_connect_response_head(&mut reader, limits).await
}

#[tokio::test]
async fn response_ordinary_200() {
    let head = parse(
        b"HTTP/1.1 200 Connection Established\r\n\r\n",
        &ConnectResponseLimits::default(),
    )
    .await
    .unwrap();
    assert_eq!(head.status, 200);
    assert!(head.headers.is_empty());
}

#[tokio::test]
async fn response_other_2xx_intact() {
    for status in [201, 204, 299] {
        let raw = format!("HTTP/1.1 {status} Whatever\r\n\r\n");
        let head = parse(raw.as_bytes(), &ConnectResponseLimits::default())
            .await
            .unwrap();
        assert_eq!(head.status, status);
    }
}

#[tokio::test]
async fn response_error_statuses_intact() {
    for status in [403, 407, 502, 504, 500, 301] {
        let raw = format!("HTTP/1.1 {status} Reason\r\nX-A: b\r\n\r\n");
        let head = parse(raw.as_bytes(), &ConnectResponseLimits::default())
            .await
            .unwrap();
        assert_eq!(head.status, status);
        assert_eq!(head.headers.len(), 1);
    }
}

#[tokio::test]
async fn response_malformed_status_rejected() {
    assert!(
        parse(b"NOT-A-STATUS\r\n\r\n", &ConnectResponseLimits::default())
            .await
            .is_err()
    );
    assert!(parse(
        b"HTTP/1.1 abc Reason\r\n\r\n",
        &ConnectResponseLimits::default()
    )
    .await
    .is_err());
    assert!(parse(b"\r\n\r\n", &ConnectResponseLimits::default())
        .await
        .is_err());
}

#[tokio::test]
async fn response_truncated_rejected() {
    assert!(parse(b"HTTP/1.1 200", &ConnectResponseLimits::default())
        .await
        .is_err());
    assert!(parse(
        b"HTTP/1.1 200 OK\r\nX-A: value",
        &ConnectResponseLimits::default()
    )
    .await
    .is_err());
    assert!(parse(b"", &ConnectResponseLimits::default()).await.is_err());
}

#[tokio::test]
async fn response_header_count_exact_and_overflow() {
    let limits = ConnectResponseLimits::new(4096, 8192, 1_000_000, 3);
    let mut raw = b"HTTP/1.1 200 OK\r\n".to_vec();
    for i in 0..3 {
        raw.extend_from_slice(format!("X-H{i}: v\r\n").as_bytes());
    }
    raw.extend_from_slice(b"\r\n");
    let head = parse(&raw, &limits).await.unwrap();
    assert_eq!(head.headers.len(), 3);
    let mut raw2 = raw.clone();
    // Insert a fourth header before the terminator.
    let pos = raw2.len() - 2;
    raw2.splice(pos..pos, b"X-Extra: v\r\n".to_vec());
    assert!(parse(&raw2, &limits).await.is_err());
}

#[tokio::test]
async fn response_aggregate_exact_and_overflow() {
    // Each header line without CRLF is 11 bytes ("X-A: 123456").
    assert_eq!(b"X-A: 123456".len(), 11);
    let limits = ConnectResponseLimits::new(4096, 8192, 22, 100);
    let raw = b"HTTP/1.1 200 OK\r\nX-A: 123456\r\nX-B: 123456\r\n\r\n";
    assert!(parse(raw, &limits).await.is_ok());
    let limits_one_less = ConnectResponseLimits::new(4096, 8192, 21, 100);
    assert!(parse(raw, &limits_one_less).await.is_err());
}

#[tokio::test]
async fn response_status_line_limit_enforced() {
    let limits = ConnectResponseLimits::new(16, 8192, 65_536, 100);
    assert!(
        parse(b"HTTP/1.1 200 Connection Established\r\n\r\n", &limits)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn response_header_line_limit_enforced() {
    let limits = ConnectResponseLimits::new(4096, 10, 65_536, 100);
    assert!(
        parse(b"HTTP/1.1 200 OK\r\nX-Long-Header: value\r\n\r\n", &limits)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn response_obs_text_preserved() {
    let mut raw = b"HTTP/1.1 200 OK\r\nX-Bin: ".to_vec();
    raw.extend_from_slice(&[0x80, 0xFF, b'a']);
    raw.extend_from_slice(b"\r\n\r\n");
    let head = parse(&raw, &ConnectResponseLimits::default())
        .await
        .unwrap();
    assert_eq!(head.headers[0].1, vec![0x80, 0xFF, b'a']);
}

#[tokio::test]
async fn response_malformed_header_name_rejected() {
    assert!(parse(
        b"HTTP/1.1 200 OK\r\nBad Header: v\r\n\r\n",
        &ConnectResponseLimits::default()
    )
    .await
    .is_err());
    assert!(parse(
        b"HTTP/1.1 200 OK\r\nNoColonHere\r\n\r\n",
        &ConnectResponseLimits::default()
    )
    .await
    .is_err());
}

#[tokio::test]
async fn response_duplicate_headers_preserved() {
    let raw = b"HTTP/1.1 200 OK\r\nX-Dup: one\r\nX-Dup: two\r\n\r\n";
    let head = parse(raw, &ConnectResponseLimits::default()).await.unwrap();
    assert_eq!(head.headers.len(), 2);
    assert_eq!(head.headers[0].1, b"one");
    assert_eq!(head.headers[1].1, b"two");
}

#[tokio::test]
async fn response_readahead_preserved() {
    let (client, mut server) = tokio::io::duplex(65_536);
    server
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\ntunneled-payload")
        .await
        .unwrap();
    drop(server);
    let mut reader = BufReader::new(client);
    let head = read_connect_response_head(&mut reader, &ConnectResponseLimits::default())
        .await
        .unwrap();
    assert_eq!(head.status, 200);
    {
        use tokio::io::AsyncReadExt;
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).await.unwrap();
        assert_eq!(rest, b"tunneled-payload");
    }
    let _ = client;
}

// ── Secrets ──────────────────────────────────────────────────────────────

#[test]
fn auth_debug_redacted() {
    let target = ConnectTarget::new("example.com", 443).unwrap();
    let secret = basic_auth_value("user", "s3cr3t").unwrap();
    let extra = vec![("Proxy-Authorization".to_owned(), b"Basic c2VjcmV0".to_vec())];
    let request = ConnectRequest {
        target: &target,
        proxy_authorization: Some(&secret),
        extra_headers: &extra,
        max_head_bytes: 65_536,
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("s3cr3t"));
    assert!(!debug.contains(&secret));
    assert!(!debug.contains("c2VjcmV0"));
}

#[test]
fn errors_never_include_credentials() {
    let secret = basic_auth_value("user", "s3cr3t").unwrap();
    // Duplicate auth error must not echo the secret.
    let target = ConnectTarget::new("example.com", 443).unwrap();
    let extra = vec![("Proxy-Authorization".to_owned(), b"Basic eA==".to_vec())];
    let request = ConnectRequest {
        target: &target,
        proxy_authorization: Some(&secret),
        extra_headers: &extra,
        max_head_bytes: 65_536,
    };
    let err = encode_connect_request(&request).unwrap_err();
    let text = err.to_string();
    assert!(!text.contains("s3cr3t"));
    assert!(!text.contains(&secret));
}

#[test]
fn invalid_input_errors_bounded() {
    // Long but otherwise valid hosts are accepted; the request-head bound
    // is what caps them. Invalid inputs must still produce fixed,
    // non-echoing diagnostics.
    let long_host = "a".repeat(5000);
    assert!(ConnectTarget::new(long_host, 443).is_ok());
    let err = ConnectTarget::new("bad\r\n".to_owned() + &"x".repeat(5000), 443).unwrap_err();
    assert!(!err.to_string().contains("xxxx"));
    assert!(err.to_string().len() < 200);
}
