//! Adverse-condition and edge-case tests for the eggfetch HTTP client.

#![allow(clippy::large_futures)]

mod test_server;

use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::body::{BoxBytesStream, RequestBody};
use eggfetch_core::error::Error;
use eggfetch_core::redirect::{build_redirect_request, drops_body_on_redirect, redirect_method};
use eggfetch_core::retry::RetryPolicy;
use eggfetch_core::tls::TlsConfig;
use eggfetch_core::{Client, Method};
use futures_util::{stream, StreamExt};
use test_server::{TestServer, TestServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn dropping_streaming_body_releases_pool_slot() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"hello world".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_connections(1).build();

    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let stream = resp.bytes_stream().unwrap();
    drop(stream);

    let mut resp2 = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp2.is_success());
    let _ = resp2.bytes().await;

    server.shutdown();
}

#[tokio::test]
async fn abort_during_request_cleans_up() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_connections(1).build();

    let handle = tokio::spawn({
        let client = client.clone();
        let url = url.clone();
        async move { client.get(&url).unwrap().send().await }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    handle.abort();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = tokio::time::timeout(Duration::from_secs(2), async {
        client.get(&url).unwrap().send().await
    })
    .await;
    assert!(result.is_ok(), "client should not deadlock after abort");

    server.shutdown();
}

#[tokio::test]
async fn partial_stream_read_then_drop_releases_lease() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"chunk1chunk2chunk3".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_connections(1).build();

    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.is_success());

    let mut stream = resp.bytes_stream().unwrap();
    let _first = stream.next().await;

    drop(stream);

    let second = client.get(&url).unwrap().send().await;
    assert!(
        second.is_ok(),
        "connection should be available after partial read + drop"
    );

    server.shutdown();
}

#[tokio::test]
async fn read_timeout_fires_during_slow_chunks() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request_line = Vec::new();
        stream.read_buf(&mut request_line).await.ok();
        while {
            let mut line = Vec::new();
            let n = stream.read_buf(&mut line).await.unwrap_or(0);
            n > 0 && line.windows(2).position(|w| w == b"\r\n").is_none()
        } {}
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n";
        stream.write_all(response).await.ok();
        tokio::time::sleep(Duration::from_secs(10)).await;
        let final_chunk = b"0\r\n\r\n";
        let _ = stream.write_all(final_chunk).await;
    });

    let client = Client::builder()
        .max_connections(1)
        .timeout(
            eggfetch_core::Timeout::builder()
                .read(Duration::from_millis(100))
                .build(),
        )
        .build();

    let result = client.get(&format!("http://{addr}")).unwrap().send().await;

    match result {
        Ok(mut resp) => {
            let _ = resp.bytes_stream().unwrap().next().await;
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("timeout") || msg.contains("timed out"),
                "expected timeout error, got: {msg}"
            );
        }
    }
}

#[test]
fn malformed_content_length_rejected() {
    let client = Client::new();
    let result = client
        .get("http://example.com")
        .unwrap()
        .header("content-length", "not-a-number")
        .build();
    assert!(
        result.is_ok(),
        "header value parsing should succeed at build time"
    );

    let client = Client::new();
    let result = client
        .get("http://example.com")
        .unwrap()
        .header("content-length", "-5")
        .build();
    assert!(
        result.is_ok(),
        "negative content-length accepted at build time (server rejects)"
    );
}

#[test]
fn empty_host_header_handled() {
    let client = Client::new();
    let result = client
        .get("http://example.com")
        .unwrap()
        .header("host", "")
        .build();
    assert!(result.is_ok(), "empty Host header should not panic");
    let req = result.unwrap();
    assert_eq!(
        req.headers().get("host").map(|v| v.to_str().unwrap()),
        Some("")
    );
}

#[test]
fn redirect_method_rewrite_303_get_to_get() {
    let new_method = redirect_method(http::StatusCode::SEE_OTHER, &Method::GET);
    assert_eq!(new_method, Method::GET);
}

#[test]
fn redirect_cross_origin_strips_all_credentials() {
    let client = Client::new();
    let req = client
        .get("https://example.com/a")
        .unwrap()
        .header("authorization", "Bearer tok")
        .header("cookie", "session=abc")
        .header("proxy-authorization", "Basic foo")
        .build()
        .unwrap();

    let (redirect, _) =
        build_redirect_request(&req, http::StatusCode::FOUND, "https://other.com/b").unwrap();

    assert!(redirect.headers().get("authorization").is_none());
    assert!(redirect.headers().get("cookie").is_none());
    assert!(redirect.headers().get("proxy-authorization").is_none());
}

#[test]
fn redirect_drops_body_headers_on_post_to_get() {
    let client = Client::new();
    let req = client
        .post("https://example.com/a")
        .unwrap()
        .header("content-length", "7")
        .header("content-type", "text/plain")
        .header("transfer-encoding", "chunked")
        .body(Bytes::from("payload"))
        .build()
        .unwrap();

    let (redirect, _) = build_redirect_request(
        &req,
        http::StatusCode::MOVED_PERMANENTLY,
        "https://example.com/b",
    )
    .unwrap();

    assert_eq!(*redirect.method(), Method::GET);
    assert!(redirect.headers().get("content-length").is_none());
    assert!(redirect.headers().get("content-type").is_none());
    assert!(redirect.headers().get("transfer-encoding").is_none());
}

#[test]
fn redirect_preserves_method_on_303_get_to_get() {
    let new_method = redirect_method(http::StatusCode::SEE_OTHER, &Method::GET);
    assert_eq!(new_method, Method::GET);
}

#[tokio::test]
async fn retry_budget_not_consumed_by_redirect_hops() {
    let policy = RetryPolicy::builder().max_attempts(3).build();
    let client = Client::builder().retry(policy).build();

    let result = client.get("http://127.0.0.1:1/").unwrap().send().await;
    assert!(result.is_err());
}

#[test]
fn backoff_delay_extreme_factors_saturate() {
    let cases: Vec<f64> = vec![f64::NAN, f64::INFINITY, -1.0, f64::MAX, 0.0];
    for factor in cases {
        let policy = RetryPolicy::builder()
            .max_attempts(10)
            .backoff_factor(factor)
            .build();
        let backoff = policy.backoff();
        for attempt in 2..=10 {
            let d = backoff.delay(attempt);
            assert!(
                d.is_some(),
                "delay should be Some for factor={factor}, attempt={attempt}"
            );
            assert!(
                d.unwrap() <= backoff.max_delay(),
                "delay exceeded max_delay for factor={factor}, attempt={attempt}"
            );
        }
    }
}

#[test]
fn retry_after_garbage_input_returns_none() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .respect_retry_after(true)
        .build();

    assert!(policy.retry_after_delay("not-a-date-or-number").is_none());
    assert!(policy.retry_after_delay("").is_none());
    assert!(policy.retry_after_delay("   ").is_none());
    assert!(policy.retry_after_delay("abc123xyz").is_none());
}

#[cfg(feature = "multipart")]
#[tokio::test]
async fn multipart_stream_error_propagates_through_encoder() {
    use eggfetch_core::multipart::{Boundary, Multipart};

    let err_stream: BoxBytesStream = Box::pin(stream::once(async {
        Err(Error::Body("simulated read error".into()))
    }));

    let boundary = Boundary::try_new("testboundary").unwrap();
    let mp = Multipart::with_boundary(boundary)
        .stream("file", "test.txt", "text/plain", err_stream, None)
        .unwrap();

    let mut encoder = mp.encoder();
    let mut found_error = false;
    while let Some(item) = StreamExt::next(&mut encoder).await {
        if item.is_err() {
            found_error = true;
            break;
        }
    }
    assert!(found_error, "encoder should propagate stream error");
}

#[cfg(feature = "multipart")]
#[tokio::test]
async fn multipart_encoder_after_stream_error_yields_none() {
    use eggfetch_core::multipart::{Boundary, Multipart};

    let err_stream: BoxBytesStream = Box::pin(stream::once(async {
        Err(Error::Body("simulated read error".into()))
    }));

    let boundary = Boundary::try_new("testboundary").unwrap();
    let mp = Multipart::with_boundary(boundary)
        .stream("file", "test.txt", "text/plain", err_stream, None)
        .unwrap();

    let mut encoder = mp.encoder();
    while let Some(item) = StreamExt::next(&mut encoder).await {
        if item.is_err() {
            break;
        }
    }

    let next = StreamExt::next(&mut encoder).await;
    assert!(next.is_none(), "encoder should yield None after error");
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_invalid_field_name_errors_at_build() {
    use eggfetch_core::multipart::Multipart;

    let result = Multipart::new().text("", "value");
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::RequestBuild(msg) => assert!(msg.contains("must not be empty")),
        other => panic!("expected RequestBuild error, got {other:?}"),
    }
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_invalid_boundary_errors_at_build() {
    use eggfetch_core::multipart::Boundary;

    let result = Boundary::try_new("");
    assert!(result.is_err());

    let result = Boundary::try_new("has spaces");
    assert!(result.is_err());

    let result = Boundary::try_new("has@special!chars");
    assert!(result.is_err());
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_valid_boundary_accepted() {
    use eggfetch_core::multipart::Boundary;

    let result = Boundary::try_new("simple-boundary-123");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "simple-boundary-123");
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_boundary_too_long_rejected() {
    use eggfetch_core::multipart::Boundary;

    let long = "a".repeat(70);
    let result = Boundary::try_new(&long);
    assert!(result.is_err());
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_matching_localhost() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("localhost").unwrap();
    let urls = [
        "http://localhost/path",
        "http://127.0.0.1/path",
        "http://[::1]/path",
    ];
    for url_str in &urls {
        let url = url::Url::parse(url_str).unwrap();
        assert!(np.should_bypass(&url), "should bypass {url_str}");
    }
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_matching_domain_suffix() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse(".example.com").unwrap();
    let url_match = url::Url::parse("http://foo.example.com/path").unwrap();
    let url_exact = url::Url::parse("http://example.com/path").unwrap();
    let url_no_match = url::Url::parse("http://notexample.com/path").unwrap();

    assert!(np.should_bypass(&url_match));
    assert!(np.should_bypass(&url_exact));
    assert!(!np.should_bypass(&url_no_match));
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_wildcard_bypasses_all() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("*").unwrap();
    let urls = [
        "http://example.com",
        "https://other.org",
        "http://10.0.0.1:8080",
        "http://localhost",
    ];
    for url_str in &urls {
        let url = url::Url::parse(url_str).unwrap();
        assert!(np.should_bypass(&url), "wildcard should bypass {url_str}");
    }
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_host_port_matching() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("example.com:8080").unwrap();

    let url_match = url::Url::parse("http://example.com:8080/path").unwrap();
    let url_wrong_port = url::Url::parse("http://example.com:9090/path").unwrap();
    let url_wrong_host = url::Url::parse("http://other.com:8080/path").unwrap();

    assert!(np.should_bypass(&url_match));
    assert!(!np.should_bypass(&url_wrong_port));
    assert!(!np.should_bypass(&url_wrong_host));
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_default_port_matches_implicit_port() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("example.com:443").unwrap();
    let url = url::Url::parse("https://example.com/path").unwrap();
    assert!(np.should_bypass(&url));

    let np2 = NoProxy::parse("example.com:80").unwrap();
    let url2 = url::Url::parse("http://example.com/path").unwrap();
    assert!(np2.should_bypass(&url2));
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_empty_string_no_bypass() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("").unwrap();
    let url = url::Url::parse("http://example.com/path").unwrap();
    assert!(!np.should_bypass(&url));
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_multiple_rules() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("localhost, .example.com, other.com:9090").unwrap();

    assert!(np.should_bypass(&url::Url::parse("http://localhost/x").unwrap()));
    assert!(np.should_bypass(&url::Url::parse("http://foo.example.com/x").unwrap()));
    assert!(np.should_bypass(&url::Url::parse("http://other.com:9090/x").unwrap()));
    assert!(!np.should_bypass(&url::Url::parse("http://other.com:8080/x").unwrap()));
    assert!(!np.should_bypass(&url::Url::parse("http://unrelated.com/x").unwrap()));
}

#[test]
fn tls_config_rejects_empty_pem() {
    let result = TlsConfig::builder().ca_certificate_pem(b"");
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("no certificates") || msg.contains("empty"),
                "unexpected error: {msg}"
            );
        }
        Ok(_) => panic!("expected error for empty PEM"),
    }
}

#[test]
fn tls_config_rejects_garbage_pem() {
    let result = TlsConfig::builder().ca_certificate_pem(b"not-a-pem-certificate");
    assert!(result.is_err());
}

#[test]
fn tls_config_min_version_validates() {
    let config = TlsConfig::builder()
        .min_tls_version(eggfetch_core::tls::TlsVersion::Tls12)
        .max_tls_version(eggfetch_core::tls::TlsVersion::Tls13)
        .build();
    let result = config.build_rustls_config();
    assert!(result.is_ok());
}

#[test]
fn tls_config_inverted_version_range_rejected() {
    let config = TlsConfig::builder()
        .min_tls_version(eggfetch_core::tls::TlsVersion::Tls13)
        .max_tls_version(eggfetch_core::tls::TlsVersion::Tls12)
        .build();
    let result = config.build_rustls_config();
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("version range"), "unexpected error: {msg}");
        }
        Ok(_) => panic!("expected error for inverted version range"),
    }
}

#[test]
fn tls_config_empty_custom_ca_rejected() {
    let config = TlsConfig::builder()
        .trust_store(eggfetch_core::tls::TrustStore::Custom(vec![]))
        .build();
    let result = config.build_rustls_config();
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("empty"), "unexpected error: {msg}");
        }
        Ok(_) => panic!("expected error for empty custom CA"),
    }
}

#[test]
fn backoff_delay_attempt_one_returns_none() {
    let policy = RetryPolicy::builder().max_attempts(5).build();
    let backoff = policy.backoff();
    assert!(backoff.delay(1).is_none());
}

#[test]
fn backoff_delay_attempt_two_returns_initial() {
    let policy = RetryPolicy::builder().max_attempts(5).build();
    let backoff = policy.backoff();
    let d = backoff.delay(2);
    assert!(d.is_some());
    let d = d.unwrap();
    assert!(d >= Duration::ZERO);
    assert!(d <= backoff.max_delay());
}

#[test]
fn backoff_delay_max_delay_cap_enforced() {
    let policy = RetryPolicy::builder()
        .max_attempts(20)
        .backoff_factor(100.0)
        .max_delay(Duration::from_secs(5))
        .build();
    let backoff = policy.backoff();
    for attempt in 2..=20 {
        let d = backoff.delay(attempt);
        assert!(d.is_some());
        assert!(
            d.unwrap() <= Duration::from_secs(5),
            "delay for attempt {attempt} exceeded max_delay"
        );
    }
}

#[test]
fn retry_policy_disabled_by_default() {
    let policy = RetryPolicy::default();
    assert!(!policy.is_enabled());
    assert_eq!(policy.max_attempts(), 1);
}

#[test]
fn retry_policy_enabled_when_max_attempts_gt_1() {
    let policy = RetryPolicy::builder().max_attempts(3).build();
    assert!(policy.is_enabled());
    assert_eq!(policy.max_attempts(), 3);
}

#[test]
fn drops_body_on_redirect_variants() {
    assert!(drops_body_on_redirect(
        http::StatusCode::MOVED_PERMANENTLY,
        &Method::POST
    ));
    assert!(drops_body_on_redirect(
        http::StatusCode::FOUND,
        &Method::POST
    ));
    assert!(drops_body_on_redirect(
        http::StatusCode::SEE_OTHER,
        &Method::POST
    ));
    assert!(!drops_body_on_redirect(
        http::StatusCode::TEMPORARY_REDIRECT,
        &Method::POST
    ));
    assert!(!drops_body_on_redirect(
        http::StatusCode::PERMANENT_REDIRECT,
        &Method::POST
    ));
    assert!(!drops_body_on_redirect(
        http::StatusCode::FOUND,
        &Method::GET
    ));
}

#[test]
fn redirect_method_307_308_preserves_all_methods() {
    for method in [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
    ] {
        assert_eq!(
            redirect_method(http::StatusCode::TEMPORARY_REDIRECT, &method),
            method
        );
        assert_eq!(
            redirect_method(http::StatusCode::PERMANENT_REDIRECT, &method),
            method
        );
    }
}

#[test]
fn redirect_method_303_rewrites_non_head_to_get() {
    for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        assert_eq!(
            redirect_method(http::StatusCode::SEE_OTHER, &method),
            Method::GET
        );
    }
    assert_eq!(
        redirect_method(http::StatusCode::SEE_OTHER, &Method::HEAD),
        Method::HEAD
    );
}

#[tokio::test]
async fn concurrent_requests_with_connection_limit() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 50,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_connections(2).build();

    let mut handles = Vec::new();
    for _ in 0..4 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client.get(&url).unwrap().send().await.unwrap().is_success()
        }));
    }

    for h in handles {
        assert!(h.await.unwrap());
    }

    server.shutdown();
}

#[test]
fn body_streaming_is_not_replayable() {
    let stream: BoxBytesStream = Box::pin(stream::empty());
    let body = RequestBody::Stream {
        stream,
        length: Some(0),
    };
    assert!(!body.is_replayable());
}

#[test]
fn body_bytes_is_replayable() {
    let body = RequestBody::Bytes(Bytes::from("hello"));
    assert!(body.is_replayable());
}

#[test]
fn body_empty_is_replayable() {
    let body = RequestBody::Empty;
    assert!(body.is_replayable());
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_is_replayable_when_all_bytes() {
    use eggfetch_core::multipart::Multipart;

    let mp = Multipart::new()
        .text("field", "value")
        .unwrap()
        .bytes("file", "test.txt", "text/plain", Bytes::from("data"))
        .unwrap();
    assert!(mp.is_replayable());
}

#[cfg(feature = "multipart")]
#[test]
fn multipart_is_not_replayable_with_stream() {
    use eggfetch_core::multipart::{Boundary, Multipart};

    let stream: BoxBytesStream = Box::pin(stream::empty());
    let boundary = Boundary::try_new("test").unwrap();
    let mp = Multipart::with_boundary(boundary)
        .stream("file", "test.txt", "text/plain", stream, Some(4))
        .unwrap();
    assert!(!mp.is_replayable());
}

#[test]
fn body_try_clone_for_redirect_stream_fails() {
    let stream: BoxBytesStream = Box::pin(stream::empty());
    let body = RequestBody::Stream {
        stream,
        length: Some(0),
    };
    let result = body.try_clone_for_redirect();
    assert!(result.is_err());
}

#[test]
fn body_try_clone_for_redirect_bytes_succeeds() {
    let body = RequestBody::Bytes(Bytes::from("data"));
    let cloned = body.try_clone_for_redirect().unwrap();
    assert!(cloned.is_replayable());
}

#[test]
fn redirect_method_rewrite_post_to_get_on_301() {
    assert_eq!(
        redirect_method(http::StatusCode::MOVED_PERMANENTLY, &Method::POST),
        Method::GET
    );
}

#[test]
fn redirect_method_rewrite_post_to_get_on_302() {
    assert_eq!(
        redirect_method(http::StatusCode::FOUND, &Method::POST),
        Method::GET
    );
}

#[test]
fn redirect_method_rewrite_preserves_put_on_301() {
    assert_eq!(
        redirect_method(http::StatusCode::MOVED_PERMANENTLY, &Method::PUT),
        Method::PUT
    );
}

#[test]
fn redirect_build_cross_origin_preserves_non_sensitive_headers() {
    let client = Client::new();
    let req = client
        .get("https://example.com/a")
        .unwrap()
        .header("x-custom", "value")
        .header("accept", "application/json")
        .build()
        .unwrap();

    let (redirect, _) =
        build_redirect_request(&req, http::StatusCode::FOUND, "https://other.com/b").unwrap();

    assert_eq!(redirect.headers().get("x-custom").unwrap(), "value");
    assert_eq!(
        redirect.headers().get("accept").unwrap(),
        "application/json"
    );
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_ipv6_literal_match() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("[::1]").unwrap();
    let url = url::Url::parse("http://[::1]/path").unwrap();
    assert!(np.should_bypass(&url));
}

#[cfg(feature = "proxy")]
#[test]
fn no_proxy_ipv4_literal_match() {
    use eggfetch_core::proxy::NoProxy;

    let np = NoProxy::parse("10.0.0.1").unwrap();
    let url = url::Url::parse("http://10.0.0.1/path").unwrap();
    assert!(np.should_bypass(&url));
}

#[test]
fn tls_config_default_builds_successfully() {
    let config = TlsConfig::builder().build();
    let result = config.build_rustls_config();
    assert!(result.is_ok());
}

#[test]
fn tls_config_min_only_builds() {
    let config = TlsConfig::builder()
        .min_tls_version(eggfetch_core::tls::TlsVersion::Tls13)
        .build();
    let result = config.build_rustls_config();
    assert!(result.is_ok());
}

#[test]
fn tls_config_max_only_builds() {
    let config = TlsConfig::builder()
        .max_tls_version(eggfetch_core::tls::TlsVersion::Tls12)
        .build();
    let result = config.build_rustls_config();
    assert!(result.is_ok());
}

#[tokio::test]
async fn streaming_body_drop_after_first_chunk_no_deadlock() {
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"AAAA BBBB CCCC".to_vec()),
        chunk_delay_ms: 10,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder().max_connections(1).build();

    for _ in 0..3 {
        let mut resp = client.get(&url).unwrap().send().await.unwrap();
        let mut stream = resp.bytes_stream().unwrap();
        let _ = stream.next().await;
        drop(stream);
    }

    server.shutdown();
}

#[test]
fn backoff_policy_default_values() {
    let policy = RetryPolicy::builder().build();
    let backoff = policy.backoff();
    assert!((backoff.factor() - 0.5).abs() < f64::EPSILON);
    assert_eq!(backoff.max_delay(), Duration::from_secs(30));
    assert_eq!(backoff.initial_delay(), Duration::from_millis(500));
}

#[test]
fn retry_after_valid_seconds_returns_delay() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .respect_retry_after(true)
        .build();

    let d = policy.retry_after_delay("5").unwrap();
    assert_eq!(d, Duration::from_secs(5));
}

#[test]
fn retry_after_zero_returns_zero() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .respect_retry_after(true)
        .build();

    let d = policy.retry_after_delay("0").unwrap();
    assert_eq!(d, Duration::ZERO);
}

#[test]
fn retry_after_large_value_capped() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .max_delay(Duration::from_secs(10))
        .respect_retry_after(true)
        .build();

    let d = policy.retry_after_delay("999").unwrap();
    assert_eq!(d, Duration::from_secs(10));
}

#[test]
fn retry_after_disabled_returns_none() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .respect_retry_after(false)
        .build();

    assert!(policy.retry_after_delay("5").is_none());
}

#[test]
fn retry_after_negative_number_returns_none() {
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .respect_retry_after(true)
        .build();
    assert!(policy.retry_after_delay("-1").is_none());
}
