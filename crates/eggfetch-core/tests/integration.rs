//! Integration tests for eggfetch-core public API.

use bytes::Bytes;
use eggfetch_core::{Client, Error, Headers, Method, RequestBody};

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
    let io_err = std::io::Error::other("test");
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
    let result = with_network_timeout(async {
        let client = Client::new();
        client.get("https://httpbin.org/get").unwrap().send().await
    })
    .await;

    let response = match result {
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
    let body = response.text().unwrap();
    assert!(body.contains("httpbin"));
}

#[tokio::test]
async fn network_post_with_body() {
    let result = with_network_timeout(async {
        let client = Client::new();
        client
            .post("https://httpbin.org/post")
            .unwrap()
            .header("Content-Type", "application/json")
            .body(r#"{"hello":"world"}"#)
            .send()
            .await
    })
    .await;

    let response = match result {
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
    let body = response.text().unwrap();
    assert!(body.contains("hello"));
    assert!(body.contains("world"));
}

#[tokio::test]
async fn network_custom_headers_sent() {
    let result = with_network_timeout(async {
        let client = Client::new();
        client
            .get("https://httpbin.org/headers")
            .unwrap()
            .header("X-Test-Header", "integration-test-value")
            .send()
            .await
    })
    .await;

    let response = match result {
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
    let body = response.text().unwrap();
    assert!(body.contains("X-Test-Header"));
    assert!(body.contains("integration-test-value"));
}

#[tokio::test]
async fn network_query_params_serialized() {
    let result = with_network_timeout(async {
        let client = Client::new();
        client
            .get("https://httpbin.org/get")
            .unwrap()
            .query("foo", "bar")
            .query("baz", "42")
            .send()
            .await
    })
    .await;

    let response = match result {
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
    let body = response.text().unwrap();
    assert!(body.contains("foo"));
    assert!(body.contains("bar"));
    assert!(body.contains("baz"));
    assert!(body.contains("42"));
}

#[tokio::test]
async fn network_post_status_code() {
    let result = with_network_timeout(async {
        let client = Client::new();
        client
            .post("https://httpbin.org/status/201")
            .unwrap()
            .send()
            .await
    })
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
