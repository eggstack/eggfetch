#![allow(missing_docs)]

//! Lean standard-route profile coverage.
//!
//! Exercises the supported lean standard-route recipe
//! (`standard-http1` + `tls-rustls`, i.e. `transport-http1` +
//! `standard-route` + `high-level-url` + `tls-rustls`, without
//! `advanced-routing` and without the `logical-retry`/`redirects`/
//! `basic-auth` policy bundle) through stable APIs only. Every test here
//! must also pass under the full `http1`/`http2`/default compatibility
//! profiles, where the same calls take the ordinary standard Hyper path.
//!
//! Advanced routing (custom Dialer, resolved-target pinning, SNI override,
//! local-address/socket options, UDS) is intentionally absent from the lean
//! profile. Lean rejection of pinned/SNI hints is covered by
//! `lean_rejects_advanced_hints` below (which branches on
//! `advanced-routing` so the same file passes under both profiles).

mod test_server;
#[cfg(feature = "tls-rustls")]
mod tls_fixtures;

use std::time::Duration;

use eggfetch_core::{Client, Error, NetworkFailureKind, Timeout, TimeoutPhase};
use test_server::{TestServer, TestServerConfig};
use tokio::net::TcpListener;

#[tokio::test]
async fn lean_standard_http_loopback() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let client = Client::new();
    let mut response = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    assert_eq!(body, "OK");
    server.shutdown();
}

#[tokio::test]
async fn lean_standard_keepalive_reuse() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let client = Client::new();
    for _ in 0..3 {
        let mut response = client.get(&server.url()).unwrap().send().await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        response.bytes().await.unwrap();
    }
    server.shutdown();
}

#[tokio::test]
async fn lean_standard_dns_failure_typed() {
    // `.invalid` never resolves; the standard resolver must surface typed DNS.
    let failure = Client::new()
        .get("http://nonexistent.invalid/")
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();
    assert_eq!(
        failure.network_failure_kind(),
        Some(NetworkFailureKind::Dns)
    );
}

#[tokio::test]
async fn lean_standard_connection_refused_typed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let failure = Client::new()
        .get(&format!("http://{addr}/"))
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();
    assert_eq!(
        failure.network_failure_kind(),
        Some(NetworkFailureKind::ConnectionRefused)
    );
    assert_eq!(failure.into_error().kind(), "hyper_client");
}

#[tokio::test]
async fn lean_standard_total_timeout() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(50)),
            ..Timeout::default()
        })
        .build();
    let err = client.get(&server.url()).unwrap().send().await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "expected total timeout, got {err:?}"
    );
    server.shutdown();
}

#[tokio::test]
async fn lean_standard_body_cap() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(vec![b'x'; 4096]),
        ..Default::default()
    });
    let client = Client::builder().max_decoded_body_size(16).build();
    let err = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "decoded_body_too_large");
    server.shutdown();
}

#[tokio::test]
async fn lean_standard_cancellation() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let client = Client::new();
    let url = server.url();
    let handle = tokio::spawn(async move { client.get(&url).unwrap().send().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    handle.abort();
    let _ = handle.await;
    server.shutdown();
}

#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn lean_standard_https_with_test_ca() {
    use eggfetch_core::TlsConfig;
    use tls_fixtures::{CertAuthority, TlsTestServer};

    let ca = CertAuthority::new();
    let server = TlsTestServer::start(&ca, &["localhost", "127.0.0.1"]).await;
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let mut response = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");
}

#[tokio::test]
async fn lean_rejects_advanced_hints() {
    use eggfetch_core::TransportHints;

    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();
    // Use the server's effective port so `RequestBuilder::build()` port
    // validation passes and the pipeline's advanced-routing guard is the
    // authority that rejects the pin in lean profiles.
    let port: u16 = url
        .rsplit(':')
        .next()
        .unwrap()
        .trim_end_matches('/')
        .parse()
        .unwrap();
    let target: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let resolved = eggfetch_core::ResolvedTarget::new([target]).unwrap();

    // A pinned destination must never silently take the standard path in the
    // lean profile; it fails closed. Under full profiles the same hint takes
    // the advanced direct route (or fails for another typed reason if the
    // port mismatches), so branch the assertion on the feature.
    let hints = TransportHints {
        resolved_target: Some(resolved),
        ..Default::default()
    };
    let result = Client::new()
        .get(&url)
        .unwrap()
        .transport_hints(hints)
        .send()
        .await;
    #[cfg(not(feature = "advanced-routing"))]
    {
        let err = result.unwrap_err();
        assert_eq!(err.kind(), "unsupported");
        assert!(err.to_string().contains("advanced-routing"));
    }
    #[cfg(feature = "advanced-routing")]
    {
        // Full profile: pinned routing exists. With a matching port the
        // direct resolved route reaches the same loopback server.
        let mut response = result.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        response.bytes().await.unwrap();
    }

    // SNI override likewise fails closed in lean.
    let sni_hints = TransportHints {
        sni_hostname: Some("sni.example".to_owned()),
        ..Default::default()
    };
    let sni_result = Client::new()
        .get(&url)
        .unwrap()
        .transport_hints(sni_hints)
        .send()
        .await;
    #[cfg(not(feature = "advanced-routing"))]
    {
        let err = sni_result.unwrap_err();
        assert_eq!(err.kind(), "unsupported");
    }
    #[cfg(feature = "advanced-routing")]
    {
        // Full profile: SNI route exists; the loopback server has no TLS for
        // this plain-HTTP URL, but the request must not fail with the
        // lean-profile "requires advanced-routing" error.
        match sni_result {
            Ok(mut response) => {
                assert_eq!(response.status().as_u16(), 200);
                response.bytes().await.unwrap();
            }
            Err(err) => assert_ne!(err.kind(), "unsupported"),
        }
    }
    server.shutdown();
}

#[tokio::test]
async fn lean_standard_total_body_deadline() {
    // Lean `standard-http1` bypasses redirect policy but shares the common
    // finalizer: headers may arrive before total while body completion
    // exceeds it, and must report `Total`.
    let mut server = TestServer::start(&TestServerConfig {
        chunked: true,
        response_body: Some(b"0123456789ABCDEFGHIJ".to_vec()),
        chunk_delay_ms: 100,
        ..Default::default()
    });
    let url = server.url();
    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(250)),
            ..Timeout::default()
        })
        .build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "lean body must respect absolute total, got: {err:?}"
    );
    server.shutdown();
}
