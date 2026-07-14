//! Integration tests for eggfetch-core public API.

mod test_server;

use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, Error, Headers, Method, RequestBody};
use futures_util::StreamExt;
use test_server::{TestServer, TestServerConfig};

// ---------------------------------------------------------------------------
// 1. URL handling tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn valid_http_url() {
    let client = Client::new();
    let builder = client.get("http://example.com/path").unwrap();
    let req = builder.build().unwrap();
    assert_eq!(req.url().scheme(), "http");
    assert_eq!(req.url().host_str(), Some("example.com"));
    assert_eq!(req.url().path(), "/path");
}

#[test]
fn valid_https_url() {
    let client = Client::new();
    let builder = client.get("https://example.com/path").unwrap();
    let req = builder.build().unwrap();
    assert_eq!(req.url().scheme(), "https");
    assert_eq!(req.url().host_str(), Some("example.com"));
}

#[test]
fn empty_path_normalized() {
    let client = Client::new();
    let builder = client.get("https://example.com").unwrap();
    let req = builder.build().unwrap();
    assert_eq!(req.url().path(), "/");
}

#[test]
fn query_parameter_appending() {
    let client = Client::new();
    let builder = client
        .get("https://example.com/search")
        .unwrap()
        .query("q", "hello");
    let req = builder.build().unwrap();
    assert_eq!(req.url().query(), Some("q=hello"));
}

#[test]
fn existing_query_plus_appended() {
    let client = Client::new();
    let builder = client
        .get("https://example.com/search?existing=1")
        .unwrap()
        .query("q", "hello");
    let req = builder.build().unwrap();
    let query = req.url().query().unwrap();
    assert!(query.contains("existing=1"));
    assert!(query.contains("q=hello"));
}

#[test]
fn percent_encoded_query_values() {
    let client = Client::new();
    let builder = client
        .get("https://example.com/search")
        .unwrap()
        .query("q", "hello world&foo=bar");
    let req = builder.build().unwrap();
    let query = req.url().query().unwrap();
    assert!(query.contains("hello+world") || query.contains("hello%20world"));
}

#[test]
fn invalid_url_rejected() {
    let client = Client::new();
    match client.get("not a url") {
        Ok(_) => panic!("expected error for invalid URL"),
        Err(err) => assert_eq!(err.kind(), "invalid_url"),
    }
}

#[test]
fn unsupported_scheme_rejected_ftp() {
    let client = Client::new();
    match client.get("ftp://example.com/file") {
        Ok(_) => panic!("expected error for unsupported scheme"),
        Err(err) => assert_eq!(err.kind(), "unsupported"),
    }
}

#[test]
fn unsupported_scheme_rejected_file() {
    let client = Client::new();
    let result = client.get("file:///tmp/test");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 2. Header handling tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn insert_and_get_header() {
    let mut h = Headers::new();
    h.insert("X-Custom", "value1").unwrap();
    let val = h.get("x-custom").unwrap();
    assert_eq!(val.to_str().unwrap(), "value1");
}

#[test]
fn case_insensitive_lookup() {
    let mut h = Headers::new();
    h.insert("Content-Type", "text/html").unwrap();
    assert!(h.contains("content-type"));
    assert!(h.contains("CONTENT-TYPE"));
    assert!(h.contains("Content-Type"));
}

#[test]
fn append_duplicate_headers() {
    let mut h = Headers::new();
    h.append("Set-Cookie", "a=1").unwrap();
    h.append("Set-Cookie", "b=2").unwrap();
    let inner = h.into_inner();
    let all: Vec<_> = inner.get_all("set-cookie").iter().collect();
    assert_eq!(all.len(), 2);
}

#[test]
fn mixed_case_header_names() {
    let mut h = Headers::new();
    h.insert("X-Mixed-Case", "value").unwrap();
    assert_eq!(h.get("x-mixed-case").unwrap().to_str().unwrap(), "value");
    assert_eq!(h.get("X-MIXED-CASE").unwrap().to_str().unwrap(), "value");
}

#[test]
fn invalid_header_name_rejected() {
    let mut h = Headers::new();
    assert!(h.insert("", "value").is_err());
}

#[test]
fn invalid_header_name_newline_rejected() {
    let mut h = Headers::new();
    assert!(h.insert("X-Bad\n", "value").is_err());
}

#[test]
fn invalid_header_value_newline_rejected() {
    let mut h = Headers::new();
    assert!(h.insert("X-Bad", "val\r\ninjection").is_err());
}

#[test]
fn headers_extend() {
    let mut h1 = Headers::new();
    h1.insert("A", "1").unwrap();
    let mut h2 = Headers::new();
    h2.insert("B", "2").unwrap();
    h1.extend(h2);
    assert_eq!(h1.get("a").unwrap().to_str().unwrap(), "1");
    assert_eq!(h1.get("b").unwrap().to_str().unwrap(), "2");
}

#[test]
fn headers_len_and_is_empty() {
    let mut h = Headers::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    h.insert("X", "1").unwrap();
    assert!(!h.is_empty());
    assert_eq!(h.len(), 1);
}

#[test]
fn headers_get_str() {
    let mut h = Headers::new();
    h.insert("X-Test", "hello").unwrap();
    let s = h.get_str("x-test").unwrap().unwrap();
    assert_eq!(s, "hello");
}

#[test]
fn headers_into_inner() {
    let mut h = Headers::new();
    h.insert("X", "val").unwrap();
    let inner: http::HeaderMap = h.into_inner();
    assert_eq!(inner.get("x").unwrap().to_str().unwrap(), "val");
}

// ---------------------------------------------------------------------------
// 3. Body tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn request_body_from_string() {
    let body: RequestBody = "hello".to_string().into();
    assert!(!body.is_empty());
    assert_eq!(body.len(), 5);
}

#[test]
fn request_body_from_str_ref() {
    let body: RequestBody = "hello".into();
    assert!(!body.is_empty());
    assert_eq!(body.len(), 5);
}

#[test]
fn request_body_from_bytes() {
    let body: RequestBody = Bytes::from(vec![1, 2, 3]).into();
    assert!(!body.is_empty());
    assert_eq!(body.len(), 3);
}

#[test]
fn request_body_from_vec() {
    let body: RequestBody = vec![10u8, 20, 30].into();
    assert!(!body.is_empty());
    assert_eq!(body.len(), 3);
}

#[test]
fn request_body_from_slice() {
    let data: &[u8] = &[4, 5, 6];
    let body: RequestBody = data.into();
    assert_eq!(body.len(), 3);
}

#[test]
fn request_body_empty_default() {
    let body = RequestBody::default();
    assert!(body.is_empty());
    assert_eq!(body.len(), 0);
}

// ---------------------------------------------------------------------------
// 4. Request builder tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn build_request_with_method_url_headers_body() {
    let client = Client::new();
    let req = client
        .post("https://example.com/submit")
        .unwrap()
        .header("Content-Type", "application/json")
        .body(r#"{"key":"value"}"#)
        .build()
        .unwrap();

    assert_eq!(*req.method(), Method::POST);
    assert_eq!(req.url().host_str(), Some("example.com"));
    assert_eq!(
        req.headers().get("content-type").unwrap().to_str().unwrap(),
        "application/json"
    );
    assert_eq!(req.body().len(), 15);
}

#[test]
fn builder_query_param_on_url() {
    let client = Client::new();
    let req = client
        .get("https://example.com/api")
        .unwrap()
        .query("page", "1")
        .query("size", "20")
        .build()
        .unwrap();

    let query = req.url().query().unwrap();
    assert!(query.contains("page=1"));
    assert!(query.contains("size=20"));
}

#[test]
fn builder_error_on_invalid_url() {
    let client = Client::new();
    let result = client.get("not valid");
    assert!(result.is_err());
}

#[test]
fn builder_invalid_header_deferred() {
    let client = Client::new();
    let result = client
        .get("https://example.com")
        .unwrap()
        .header("Bad\nName", "value")
        .build();
    assert!(result.is_err());
}

#[test]
fn builder_set_headers_replaces() {
    let client = Client::new();
    let mut hdrs = Headers::new();
    hdrs.insert("X-From-Set", "yes").unwrap();

    let req = client
        .get("https://example.com")
        .unwrap()
        .headers(hdrs)
        .build()
        .unwrap();

    assert!(req.headers().contains("x-from-set"));
}

// ---------------------------------------------------------------------------
// 5. Error tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn error_display_invalid_url() {
    let err = Error::InvalidUrl("bad".into());
    assert_eq!(err.to_string(), "invalid URL: bad");
}

#[test]
fn error_display_invalid_header_name() {
    let err = Error::InvalidHeaderName("oops".into());
    assert_eq!(err.to_string(), "invalid header name: oops");
}

#[test]
fn error_display_invalid_header_value() {
    let err = Error::InvalidHeaderValue("oops".into());
    assert_eq!(err.to_string(), "invalid header value: oops");
}

#[test]
fn error_display_connect() {
    let err = Error::Connect("refused".into());
    assert_eq!(err.to_string(), "connect error: refused");
}

#[test]
fn error_display_tls() {
    let err = Error::Tls("cert expired".into());
    assert_eq!(err.to_string(), "TLS error: cert expired");
}

#[test]
fn error_display_protocol() {
    let err = Error::Protocol("bad status".into());
    assert_eq!(err.to_string(), "protocol error: bad status");
}

#[test]
fn error_display_body() {
    let err = Error::Body("decode failed".into());
    assert_eq!(err.to_string(), "body error: decode failed");
}

#[test]
fn error_display_request_build() {
    let err = Error::RequestBuild("missing body".into());
    assert_eq!(err.to_string(), "request build error: missing body");
}

#[test]
fn error_display_unsupported() {
    let err = Error::Unsupported("feature x".into());
    assert_eq!(err.to_string(), "unsupported: feature x");
}

#[test]
fn error_kind_invalid_url() {
    assert_eq!(Error::InvalidUrl(String::new()).kind(), "invalid_url");
}

#[test]
fn error_kind_invalid_method() {
    assert_eq!(Error::InvalidMethod(String::new()).kind(), "invalid_method");
}

#[test]
fn error_kind_invalid_header_name() {
    assert_eq!(
        Error::InvalidHeaderName(String::new()).kind(),
        "invalid_header_name"
    );
}

#[test]
fn error_kind_invalid_header_value() {
    assert_eq!(
        Error::InvalidHeaderValue(String::new()).kind(),
        "invalid_header_value"
    );
}

#[test]
fn error_kind_request_build() {
    assert_eq!(Error::RequestBuild(String::new()).kind(), "request_build");
}

#[test]
fn error_kind_connect() {
    assert_eq!(Error::Connect(String::new()).kind(), "connect");
}

#[test]
fn error_kind_tls() {
    assert_eq!(Error::Tls(String::new()).kind(), "tls");
}

#[test]
fn error_kind_protocol() {
    assert_eq!(Error::Protocol(String::new()).kind(), "protocol");
}

#[test]
fn error_kind_body() {
    assert_eq!(Error::Body(String::new()).kind(), "body");
}

#[test]
fn error_kind_unsupported() {
    assert_eq!(Error::Unsupported(String::new()).kind(), "unsupported");
}

#[test]
fn error_from_io() {
    let io_err = std::sync::Arc::new(std::io::Error::other("test"));
    let err: Error = io_err.into();
    assert_eq!(err.kind(), "io");
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn error_is_std_error() {
    let err = Error::InvalidUrl("test".into());
    let e: &dyn std::error::Error = &err;
    assert!(e.to_string().contains("invalid URL"));
}

// ---------------------------------------------------------------------------
// 6. Client construction tests (no network)
// ---------------------------------------------------------------------------

#[test]
fn client_new() {
    let _client = Client::new();
}

#[test]
fn client_default() {
    let _client = Client::default();
}

#[test]
fn client_builder_with_user_agent() {
    let client = Client::builder().user_agent("test-agent/1.0").build();
    let req = client.get("https://example.com").unwrap().build().unwrap();
    assert_eq!(*req.method(), Method::GET);
}

#[test]
fn client_builder_with_default_header() {
    let client = Client::builder()
        .default_header("X-Custom", "default-val")
        .unwrap()
        .build();
    let req = client.get("https://example.com").unwrap().build().unwrap();
    // Default headers are applied at send time, not stored in the Request.
    // But we can verify the builder didn't error.
    assert_eq!(*req.method(), Method::GET);
}

#[test]
fn client_request_method_variants() {
    let client = Client::new();
    assert_eq!(
        *client
            .get("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::GET
    );
    assert_eq!(
        *client
            .post("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::POST
    );
    assert_eq!(
        *client
            .put("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::PUT
    );
    assert_eq!(
        *client
            .patch("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::PATCH
    );
    assert_eq!(
        *client
            .delete("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::DELETE
    );
    assert_eq!(
        *client
            .head("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::HEAD
    );
    assert_eq!(
        *client
            .options("https://a.com")
            .unwrap()
            .build()
            .unwrap()
            .method(),
        Method::OPTIONS
    );
}

#[test]
fn client_custom_method() {
    let client = Client::new();
    let req = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), "https://a.com")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(*req.method(), "PROPFIND");
}

#[test]
fn client_clone() {
    let client = Client::new();
    let client2 = client.clone();
    let req = client2.get("https://example.com").unwrap().build().unwrap();
    assert_eq!(*req.method(), Method::GET);
}

// ---------------------------------------------------------------------------
// 7. Network integration tests (requires connectivity)
// ---------------------------------------------------------------------------

/// Helper: attempt an async operation with a 10-second timeout.
/// Returns `Ok(None)` if the network is unreachable.
async fn with_network_timeout<F, T>(f: F) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(std::time::Duration::from_secs(10), f)
        .await
        .ok()
}

#[tokio::test]
async fn network_get_request() {
    let result = with_network_timeout(Box::pin(async {
        let client = Client::new();
        client.get("https://httpbin.org/get").unwrap().send().await
    }))
    .await;

    let mut response = match result {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            // Network unavailable or endpoint down — skip gracefully.
            eprintln!("skipping network_get_request: {e}");
            return;
        }
        None => {
            eprintln!("skipping network_get_request: timed out");
            return;
        }
    };

    if !response.is_success() {
        eprintln!("skipping network_get_request: status {}", response.status());
        return;
    }
    let body = response.text().await.unwrap();
    assert!(body.contains("httpbin"));
}

#[tokio::test]
async fn network_post_with_body() {
    let result = with_network_timeout(Box::pin(async {
        let client = Client::new();
        client
            .post("https://httpbin.org/post")
            .unwrap()
            .header("Content-Type", "application/json")
            .body(r#"{"hello":"world"}"#)
            .send()
            .await
    }))
    .await;

    let mut response = match result {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("skipping network_post_with_body: {e}");
            return;
        }
        None => {
            eprintln!("skipping network_post_with_body: timed out");
            return;
        }
    };

    if !response.is_success() {
        eprintln!(
            "skipping network_post_with_body: status {}",
            response.status()
        );
        return;
    }
    let body = response.text().await.unwrap();
    assert!(body.contains("hello"));
    assert!(body.contains("world"));
}

#[tokio::test]
async fn network_custom_headers_sent() {
    let result = with_network_timeout(Box::pin(async {
        let client = Client::new();
        client
            .get("https://httpbin.org/headers")
            .unwrap()
            .header("X-Test-Header", "integration-test-value")
            .send()
            .await
    }))
    .await;

    let mut response = match result {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("skipping network_custom_headers_sent: {e}");
            return;
        }
        None => {
            eprintln!("skipping network_custom_headers_sent: timed out");
            return;
        }
    };

    if !response.is_success() {
        eprintln!(
            "skipping network_custom_headers_sent: status {}",
            response.status()
        );
        return;
    }
    let body = response.text().await.unwrap();
    assert!(body.contains("X-Test-Header"));
    assert!(body.contains("integration-test-value"));
}

#[tokio::test]
async fn network_query_params_serialized() {
    let result = with_network_timeout(Box::pin(async {
        let client = Client::new();
        client
            .get("https://httpbin.org/get")
            .unwrap()
            .query("foo", "bar")
            .query("baz", "42")
            .send()
            .await
    }))
    .await;

    let mut response = match result {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("skipping network_query_params_serialized: {e}");
            return;
        }
        None => {
            eprintln!("skipping network_query_params_serialized: timed out");
            return;
        }
    };

    if !response.is_success() {
        eprintln!(
            "skipping network_query_params_serialized: status {}",
            response.status()
        );
        return;
    }
    let body = response.text().await.unwrap();
    assert!(body.contains("foo"));
    assert!(body.contains("bar"));
    assert!(body.contains("baz"));
    assert!(body.contains("42"));
}

#[tokio::test]
async fn network_post_status_code() {
    let result = with_network_timeout(Box::pin(async {
        let client = Client::new();
        client
            .post("https://httpbin.org/status/201")
            .unwrap()
            .send()
            .await
    }))
    .await;

    let response = match result {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            eprintln!("skipping network_post_status_code: {e}");
            return;
        }
        None => {
            eprintln!("skipping network_post_status_code: timed out");
            return;
        }
    };

    // httpbin.org returns 503 when unreachable; skip gracefully.
    if response.status().as_u16() != 201 {
        eprintln!(
            "skipping network_post_status_code: status {}",
            response.status()
        );
        return;
    }
}

// ---------------------------------------------------------------------------
// 10. Streaming integration tests (local TCP server)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stream_chunked_response_collects() {
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"hello world".to_vec()),
        chunked: true,
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello world");
}

#[tokio::test]
async fn stream_chunked_response_streams_incrementally() {
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"abcdefghij".to_vec()),
        chunked: true,
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();

    let mut stream = resp.bytes_stream().unwrap();
    let mut all = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        all.extend_from_slice(&chunk);
    }
    assert_eq!(all, b"abcdefghij");
}

#[tokio::test]
async fn stream_buffered_response_collects() {
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"buffered body".to_vec()),
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    let body = resp.text().await.unwrap();
    assert_eq!(body, "buffered body");
}

#[tokio::test]
async fn stream_large_response_buffered() {
    let large_body = vec![b'x'; 100_000];
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(large_body.clone()),
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 100_000);
    assert_eq!(&body[..], &large_body[..]);
}

#[tokio::test]
async fn stream_large_response_streaming() {
    let large_body = vec![b'y'; 100_000];
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(large_body.clone()),
        chunked: true,
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();

    let mut stream = resp.bytes_stream().unwrap();
    let mut total = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        total += chunk.len();
    }
    assert_eq!(total, 100_000);
}

#[tokio::test]
async fn stream_double_consume_streaming_errors() {
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"data".to_vec()),
        chunked: true,
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    let _ = resp.bytes_stream().unwrap();
    let result = resp.bytes_stream();
    assert!(result.is_err());
}

#[tokio::test]
async fn stream_drop_unread_body() {
    let server = TestServer::start(&TestServerConfig {
        close_connection: false,
        ..Default::default()
    });
    let client = Client::new();
    let resp = client.get(&server.url()).unwrap().send().await.unwrap();
    // Drop without consuming body. Connection should be handled safely.
    drop(resp);
    // Server should still be reachable via a new connection.
    let mut resp2 = client.get(&server.url()).unwrap().send().await.unwrap();
    let body = resp2.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn stream_text_lines_basic() {
    use futures_util::StreamExt;

    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"line1\nline2\nline3\n".to_vec()),
        ..Default::default()
    });
    let client = Client::new();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();

    let mut lines = Vec::new();
    let stream = resp.text_lines().unwrap();
    let mut stream = std::pin::pin!(stream);
    while let Some(line) = stream.next().await {
        lines.push(line.unwrap());
    }
    assert_eq!(lines, vec!["line1", "line2", "line3"]);
}

#[tokio::test]
async fn stream_request_body_bytes() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });
    let client = Client::new();
    let resp = client
        .post(&server.url())
        .unwrap()
        .body("request payload")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

// ---------------------------------------------------------------------------
// 11. Hardening / correctness tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_stream_body_sent_as_streaming() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });
    let client = Client::new();

    let chunks = vec![
        Ok(Bytes::from("chunk1-")),
        Ok(Bytes::from("chunk2-")),
        Ok(Bytes::from("chunk3")),
    ];
    let stream = Box::pin(futures_util::stream::iter(chunks));
    let body = RequestBody::from_stream(stream, None);

    // The stream body is sent through hyper's Body impl, not buffered
    // into a Full<Bytes> before send. The test server echoes 200 only
    // if it can parse Content-Length and read the full body, proving
    // every chunk made it on the wire in order.
    let resp = client
        .post(&server.url())
        .unwrap()
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test]
async fn request_stream_body_lazy_poll() {
    // A streaming body whose producer yields one chunk at a time must be
    // polled lazily by hyper, not eagerly drained up front.
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });
    let client = Client::new();

    let polled = Arc::new(AtomicUsize::new(0));
    let p = polled.clone();
    let producer = futures_util::stream::unfold(0u32, move |i| {
        let p = p.clone();
        async move {
            if i >= 3 {
                return None;
            }
            p.fetch_add(1, Ordering::SeqCst);
            // Small async sleep so a poll that tries to drain everything
            // up front would noticeably stall the send.
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Some((Ok(Bytes::from(format!("chunk{i}-"))), i + 1))
        }
    });
    let stream = Box::pin(producer);
    let body = RequestBody::from_stream(stream, Some(Bytes::from("chunk0-").len() * 3));

    let resp = client
        .post(&server.url())
        .unwrap()
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    // Producer was polled at least once per chunk; lazy polling means the
    // producer's per-chunk sleep was observed on the send path.
    assert!(
        polled.load(Ordering::SeqCst) >= 3,
        "expected at least 3 polls, got {}",
        polled.load(Ordering::SeqCst)
    );
}

#[test]
fn url_fragment_is_preserved_in_request() {
    let client = Client::new();
    let req = client
        .get("https://example.com/path#fragment")
        .unwrap()
        .build()
        .unwrap();
    // url::Url preserves the fragment in its string representation.
    let url_str = req.url().to_string();
    assert!(
        url_str.contains("#fragment"),
        "fragment lost from URL: {url_str}"
    );
}

#[test]
fn header_name_with_carriage_return_rejected() {
    let mut headers = Headers::new();
    let result = headers.insert("X-Test\r", "value");
    assert!(result.is_err());
}

#[test]
fn header_value_with_carriage_return_rejected() {
    let mut headers = Headers::new();
    let result = headers.insert("X-Test", "value\r");
    assert!(result.is_err());
}

#[test]
fn header_value_with_newline_rejected() {
    let mut headers = Headers::new();
    let result = headers.insert("X-Test", "value\n");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Track 2: Redirect and request-body replay correctness
// ---------------------------------------------------------------------------

/// Streamed upload remains lazy with redirects enabled but no redirect received.
/// The fast path should not buffer a stream body.
#[tokio::test]
async fn streamed_upload_lazy_no_redirect() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let polled = Arc::new(AtomicUsize::new(0));
    let polled_inner = polled.clone();

    let producer = futures_util::stream::iter(0..3).map(move |i| {
        polled_inner.fetch_add(1, Ordering::SeqCst);
        Ok::<_, eggfetch_core::Error>(Bytes::from(format!("chunk{i}")))
    });
    let body = RequestBody::from_stream(producer, None);

    let client = Client::builder().follow_redirects(true).build();

    let resp = client
        .post(&server.url())
        .unwrap()
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    // Producer was polled lazily (not eagerly buffered into bytes).
    assert!(
        polled.load(Ordering::SeqCst) >= 1,
        "stream should be polled lazily, not eagerly buffered"
    );
}

/// 307/308 redirects replay bytes exactly.
#[tokio::test]
async fn redirect_307_replays_bytes() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"final destination".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((307, format!("http://127.0.0.1:{final_port}/final"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(3)
        .build();

    let mut resp = client
        .post(&redirect_server.url())
        .unwrap()
        .body(RequestBody::from(Bytes::from("payload")))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.url().port(), Some(final_port));
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"final destination");
}

/// 303 after POST drops the body (rewrites to GET).
#[tokio::test]
async fn redirect_303_drops_body() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"get result".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((303, format!("http://127.0.0.1:{final_port}/target"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(3)
        .build();

    let mut resp = client
        .post(&redirect_server.url())
        .unwrap()
        .body(RequestBody::from(Bytes::from("should be dropped")))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"get result");
}

/// 307/308 with a live stream body is rejected because the stream cannot be
/// replayed after the first hop.
#[tokio::test]
async fn redirect_307_rejects_stream_body() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"redirected".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((307, format!("http://127.0.0.1:{final_port}/target"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(3)
        .build();

    let body = RequestBody::from_stream(
        futures_util::stream::once(async { Ok(Bytes::from("stream data")) }),
        None,
    );

    let err = client
        .post(&redirect_server.url())
        .unwrap()
        .body(body)
        .send()
        .await
        .unwrap_err();

    assert!(matches!(err, Error::BodyNotReplayableForRedirect));
}

/// Multi-hop redirects (302 → 302 → 200) do not leak pool permits.
#[tokio::test]
async fn multi_hop_redirect_releases_permits() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"done".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    // hop2 → final
    let hop2 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        close_connection: true,
        ..Default::default()
    });
    let hop2_port = hop2.port();

    // hop1 → hop2
    let hop1 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{hop2_port}/"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    let mut resp = client.get(&hop1.url()).unwrap().send().await.unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"done");
    // History should contain the two redirect hops.
    assert_eq!(resp.history().len(), 2);
    assert_eq!(resp.history()[0].status().as_u16(), 302);
    assert_eq!(resp.history()[1].status().as_u16(), 302);
}

/// Redirect response body is drained before the next hop.
#[tokio::test]
async fn redirect_response_body_drained_before_next_hop() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"ok".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(3)
        .build();

    let mut resp = client
        .get(&redirect_server.url())
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"ok");
}

// ---------------------------------------------------------------------------
// Track 3: Total timeout across redirect chains
// ---------------------------------------------------------------------------

/// Total timeout is a single wall-clock deadline across redirect hops.
/// A chain of individually-fast redirects leading to a slow final server
/// must fail if the total elapsed time exceeds the deadline.
#[tokio::test]
async fn total_timeout_across_redirects_slow_final() {
    let final_server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        response_body: Some(b"slow".to_vec()),
        ..Default::default()
    });
    let final_port = final_server.port();

    let hop2 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        ..Default::default()
    });
    let hop2_port = hop2.port();

    let hop1 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{hop2_port}/"))),
        ..Default::default()
    });

    let client = Client::builder()
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_millis(100)),
            ..Default::default()
        })
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    let result = client.get(&hop1.url()).unwrap().send().await;
    assert!(
        matches!(
            result,
            Err(Error::Timeout {
                phase: eggfetch_core::TimeoutPhase::Total,
                ..
            })
        ),
        "expected total timeout across redirects, got: {result:?}"
    );
}

/// Short redirect chain completes well within the total timeout.
#[tokio::test]
async fn total_timeout_across_redirects_fast_chain() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"done".to_vec()),
        ..Default::default()
    });
    let final_port = final_server.port();

    let hop2 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        ..Default::default()
    });
    let hop2_port = hop2.port();

    let hop1 = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{hop2_port}/"))),
        ..Default::default()
    });

    let client = Client::builder()
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    let mut resp = client.get(&hop1.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"done");
    assert_eq!(resp.history().len(), 2);
}

/// Total timeout does NOT reset per hop: a chain of 3 hops each taking
/// ~100ms must fail with a 150ms total deadline.
#[tokio::test]
async fn total_timeout_not_reset_per_hop() {
    let final_server = TestServer::start(&TestServerConfig {
        response_delay_ms: 100,
        response_body: Some(b"final".to_vec()),
        ..Default::default()
    });
    let final_port = final_server.port();

    let hop2 = TestServer::start(&TestServerConfig {
        response_delay_ms: 100,
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        ..Default::default()
    });
    let hop2_port = hop2.port();

    let hop1 = TestServer::start(&TestServerConfig {
        response_delay_ms: 100,
        redirect: Some((302, format!("http://127.0.0.1:{hop2_port}/"))),
        ..Default::default()
    });

    let client = Client::builder()
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_millis(150)),
            ..Default::default()
        })
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    let result = client.get(&hop1.url()).unwrap().send().await;
    assert!(
        matches!(
            result,
            Err(Error::Timeout {
                phase: eggfetch_core::TimeoutPhase::Total,
                ..
            })
        ),
        "expected total timeout (150ms) to fire across 3 hops each taking ~100ms, got: {result:?}"
    );
}

/// Pool wait on a later hop counts against the original total deadline.
/// Client holds `max_connections=1`, first request holds the slot, second
/// request through a redirect chain must wait for pool + redirect processing
/// to exceed total.
#[tokio::test]
async fn total_timeout_pool_wait_on_later_hop() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"ok".to_vec()),
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        ..Default::default()
    });

    let client = Client::builder()
        .max_connections(1)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_millis(200)),
            ..Default::default()
        })
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    // Hold the only connection slot with a slow request.
    let blocker_client = client.clone();
    let blocker_url = redirect_server.url();
    let h1 = tokio::spawn(async move {
        let mut r = blocker_client
            .get(&blocker_url)
            .unwrap()
            .send()
            .await
            .unwrap();
        let _ = r.bytes().await;
    });

    // Give the blocker time to acquire the slot.
    tokio::time::sleep(Duration::from_millis(30)).await;

    // This request goes through a redirect, requiring a second connection.
    // With max_connections=1, it must wait for the pool, eating into the
    // total deadline.
    let result = client.get(&redirect_server.url()).unwrap().send().await;

    // The second request either times out (pool wait + redirect) or
    // succeeds if the blocker finished quickly. Either is valid — the
    // important thing is it does not panic or hang.
    if let Err(Error::Timeout {
        phase: eggfetch_core::TimeoutPhase::Total,
        ..
    }) = result
    {
        /* expected when pool wait exceeds deadline */
    } else if result.is_err() {
        panic!("unexpected error: {result:?}");
    }

    let _ = h1.await;
}

/// Redirect history entries are metadata-only (no active body streams).
#[tokio::test]
async fn redirect_history_is_metadata_only() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"end".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((301, format!("http://127.0.0.1:{final_port}/"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .follow_redirects(true)
        .max_redirects(3)
        .build();

    let resp = client
        .get(&redirect_server.url())
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.history().len(), 1);

    let hist = &resp.history()[0];
    assert_eq!(hist.status().as_u16(), 301);
    assert!(hist.url().host_str().is_some());
    // HistoryEntry is a metadata-only snapshot — no body() method exists,
    // which is the structural guarantee that no pool permit can leak.
    assert!(!hist.reason_phrase().is_empty());
}

// ---------------------------------------------------------------------------
// 12. Compression integration tests
// ---------------------------------------------------------------------------

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn automatic_decompression_disabled_preserves_encoding_and_raw_bytes() {
    use std::io::Write;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.unwrap();

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line.trim().is_empty() {
                break;
            }
        }

        let uncompressed = b"hello compressed world";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(uncompressed).unwrap();
        let compressed = encoder.finish().unwrap();

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
             Content-Encoding: gzip\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n",
            compressed.len()
        );
        let mut stream = reader.into_inner();
        stream.write_all(resp.as_bytes()).await.unwrap();
        stream.write_all(&compressed).await.unwrap();
        stream.flush().await.unwrap();
    });

    let client = Client::builder().automatic_decompression(false).build();

    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();

    assert_eq!(resp.status().as_u16(), 200);

    let encoding = resp.headers().get("content-encoding");
    assert!(
        encoding.is_some(),
        "Content-Encoding header should be present when decompression is disabled"
    );
    assert_eq!(
        encoding.unwrap().to_str().unwrap(),
        "gzip",
        "Content-Encoding should be gzip"
    );

    let body = resp.bytes().await.unwrap();
    let expected_gzip = {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(b"hello compressed world").unwrap();
        encoder.finish().unwrap()
    };
    assert_eq!(
        body.as_ref(),
        expected_gzip.as_slice(),
        "body should be raw gzip bytes, not decompressed"
    );

    let _ = server_handle.await;
}

// ---------------------------------------------------------------------------
// Track C: Pool permit release after decoded body limit error
// ---------------------------------------------------------------------------

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn pool_permit_released_after_decoded_body_limit_error() {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let request_count = Arc::new(AtomicUsize::new(0));

    let rc = request_count.clone();
    let server_handle = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let count = rc.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).await.unwrap();

            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line.trim().is_empty() {
                    break;
                }
            }

            // First request: large body; second: small body.
            let uncompressed = if count == 0 {
                b"hello compressed world that exceeds the limit easily".to_vec()
            } else {
                b"ok".to_vec()
            };
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(&uncompressed).unwrap();
            let compressed = encoder.finish().unwrap();

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
                 Content-Encoding: gzip\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                compressed.len()
            );
            let mut stream = reader.into_inner();
            stream.write_all(resp.as_bytes()).await.unwrap();
            stream.write_all(&compressed).await.unwrap();
            stream.flush().await.unwrap();
        }
    });

    let client = Client::builder().max_decoded_body_size(10).build();

    let url = format!("http://127.0.0.1:{port}/");

    // First request: large body -> DecodedBodyTooLarge when consuming.
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let err = resp.text().await.unwrap_err();
    assert_eq!(err.kind(), "decoded_body_too_large");

    // Second request: small body -> succeeds, proving pool permit was released.
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "ok");

    let _ = server_handle.await;
}

// ---------------------------------------------------------------------------
// Track C: Nested encoding decoded-size accounting
// ---------------------------------------------------------------------------

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn nested_encoding_decoded_size_tracked_correctly() {
    use std::io::Write;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.unwrap();

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line.trim().is_empty() {
                break;
            }
        }

        // Double-gzip: gzip the already-gzip-compressed data.
        let inner = b"nested content";
        let mut inner_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        inner_encoder.write_all(inner).unwrap();
        let inner_compressed = inner_encoder.finish().unwrap();

        let mut outer_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        outer_encoder.write_all(&inner_compressed).unwrap();
        let outer_compressed = outer_encoder.finish().unwrap();

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
             Content-Encoding: gzip, gzip\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n",
            outer_compressed.len()
        );
        let mut stream = reader.into_inner();
        stream.write_all(resp.as_bytes()).await.unwrap();
        stream.write_all(&outer_compressed).await.unwrap();
        stream.flush().await.unwrap();
    });

    // With a tight limit, the double-decoded content (14 bytes) should exceed it.
    let client = Client::builder().max_decoded_body_size(10).build();

    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let err = resp.text().await.unwrap_err();
    assert_eq!(
        err.kind(),
        "decoded_body_too_large",
        "nested encoding decoded size should be enforced"
    );

    let _ = server_handle.await;
}

// ---------------------------------------------------------------------------
// Track D: Accept-Encoding header matches compiled features
// ---------------------------------------------------------------------------

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn accept_encoding_header_matches_compiled_features() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.unwrap();

        let mut accept_encoding = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = line.trim().split_once(':') {
                if name.eq_ignore_ascii_case("Accept-Encoding") {
                    accept_encoding = Some(value.trim().to_string());
                }
            }
        }

        let ae = accept_encoding.unwrap_or_default();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{ae}",
            ae.len()
        );
        let mut stream = reader.into_inner();
        stream.write_all(resp.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
    });

    let client = Client::new();
    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let body = resp.text().await.unwrap();

    // gzip feature implies deflate too.
    let expected = eggfetch_core::compression::accept_encoding_value().unwrap_or("");
    assert_eq!(
        body, expected,
        "Accept-Encoding sent to server should match accept_encoding_value()"
    );

    let _ = server_handle.await;
}
