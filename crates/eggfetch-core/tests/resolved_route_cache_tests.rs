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
//! Resolved-target route cache and connection reuse regressions.
//!
//! Same logical origin + same ordered physical snapshot + same SNI must reuse
//! one configured Hyper client so H1 keep-alive and H2 multiplexing survive
//! across independent requests. Any change in those dimensions must select a
//! different entry (false hits are security defects; false misses are only
//! performance).

mod test_server;
#[cfg(feature = "tls-rustls")]
mod tls_fixtures;

use eggfetch_core::{Client, Error};
use std::time::Duration;
use test_server::{TestServer, TestServerConfig};

/// H1 same-key reuse: repeated sequential requests with one logical origin
/// and identical resolved snapshot must reuse a persistent TCP connection
/// when the server keeps it alive.
#[tokio::test]
async fn resolved_same_key_reuses_h1_keepalive_connection() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let client = Client::builder().build();
    let url = format!("http://pinned.invalid:{port}/");

    for _ in 0..3 {
        let mut response = client
            .get(&url)
            .unwrap()
            .resolved_addresses([address])
            .send()
            .await
            .unwrap();
        assert!(response.is_success());
        let _ = response.bytes().await.unwrap();
    }

    assert_eq!(
        server.connections_accepted(),
        1,
        "same resolved route must reuse one H1 keep-alive connection"
    );
    assert_eq!(server.requests_served(), 3);
    assert_eq!(
        client.transport_metrics().snapshot().direct_dns_attempts,
        0,
        "resolved routing must never fall back to DNS"
    );

    server.shutdown();
}

/// Path/query/fragment never participate in route identity: same origin and
/// snapshot with different request targets still reuses one connection.
#[tokio::test]
async fn resolved_path_query_fragment_reuse_same_connection() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder().build();

    for path in ["/a", "/b?x=1", "/c#frag"] {
        let url = format!("http://pinned.invalid:{port}{path}");
        let mut response = client
            .get(&url)
            .unwrap()
            .resolved_addresses([address])
            .send()
            .await
            .unwrap();
        assert!(response.is_success());
        let _ = response.bytes().await.unwrap();
    }

    assert_eq!(
        server.connections_accepted(),
        1,
        "path/query/fragment must not fragment the resolved route"
    );
    server.shutdown();
}

/// Different snapshot isolation: same logical origin with `[good]` versus
/// `[good, good]` (distinct ordered snapshots) must not share a pool, even
/// though both reach the same listener. An idle A connection must never
/// serve B.
#[tokio::test]
async fn resolved_different_snapshot_does_not_share_pool() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let good: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder().build();
    let url = format!("http://pinned.invalid:{port}/");

    let mut first = client
        .get(&url)
        .unwrap()
        .resolved_addresses([good])
        .send()
        .await
        .unwrap();
    assert!(first.is_success());
    let _ = first.bytes().await.unwrap();

    // Same origin, same first candidate, but a different ordered snapshot
    // (extra duplicate candidate). Must be a distinct cache entry.
    let mut second = client
        .get(&url)
        .unwrap()
        .resolved_addresses([good, good])
        .send()
        .await
        .unwrap();
    assert!(second.is_success());
    let _ = second.bytes().await.unwrap();

    assert_eq!(
        server.connections_accepted(),
        2,
        "different snapshots must not share a resolved-route pool"
    );
    assert_eq!(client.transport_metrics().snapshot().direct_dns_attempts, 0);
    server.shutdown();
}

/// Address-order isolation: `[good, bad]` and `[bad, good]` are distinct
/// route identities even though both ultimately reach the same listener.
#[tokio::test]
async fn resolved_address_order_fragments_route_identity() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let good: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    // Same port, different loopback IP, nothing listens there: failover to
    // the second candidate is fast (refused) rather than a timeout.
    let bad: std::net::SocketAddr = format!("127.0.0.2:{port}").parse().unwrap();
    let client = Client::builder().build();
    let url = format!("http://pinned.invalid:{port}/");

    let mut first = client
        .get(&url)
        .unwrap()
        .resolved_addresses([good, bad])
        .send()
        .await
        .unwrap();
    assert!(first.is_success());
    let _ = first.bytes().await.unwrap();

    let mut second = client
        .get(&url)
        .unwrap()
        .resolved_addresses([bad, good])
        .send()
        .await
        .unwrap();
    assert!(second.is_success());
    let _ = second.bytes().await.unwrap();

    assert_eq!(
        server.connections_accepted(),
        2,
        "reordered snapshots must not share a resolved-route pool"
    );
    server.shutdown();
}

/// Origin isolation: two logical origins mapped to the same physical address
/// must not share a configured resolved-route client.
#[tokio::test]
async fn resolved_different_origins_do_not_share_pool() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder().build();

    for host in ["a.invalid", "b.invalid"] {
        let url = format!("http://{host}:{port}/");
        let mut response = client
            .get(&url)
            .unwrap()
            .resolved_addresses([address])
            .send()
            .await
            .unwrap();
        assert!(response.is_success());
        let _ = response.bytes().await.unwrap();
    }

    assert_eq!(
        server.connections_accepted(),
        2,
        "different logical origins must not share a resolved-route pool"
    );
    server.shutdown();
}

/// SNI isolation over cleartext: identical origin/addresses with `None` vs an
/// override must not share a pool. TLS identity is proven separately below.
#[tokio::test]
async fn resolved_sni_override_fragments_pool() {
    use eggfetch_core::TransportHints;

    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder().build();
    let url = format!("http://pinned.invalid:{port}/");

    let mut plain = client
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap();
    assert!(plain.is_success());
    let _ = plain.bytes().await.unwrap();

    let target = eggfetch_core::ResolvedTarget::new([address]).unwrap();
    let hints = TransportHints {
        resolved_target: Some(target),
        sni_hostname: Some("sni-a.example".to_owned()),
        ..Default::default()
    };
    let mut with_sni = client
        .get(&url)
        .unwrap()
        .transport_hints(hints)
        .send()
        .await
        .unwrap();
    assert!(with_sni.is_success());
    let _ = with_sni.bytes().await.unwrap();

    assert_eq!(
        server.connections_accepted(),
        2,
        "SNI None vs override must not share a resolved-route pool"
    );
    server.shutdown();
}

/// SNI correctness over TLS: the exact override controls certificate
/// verification for the resolved route. A matching SAN succeeds; a mismatch
/// fails closed.
#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn resolved_sni_override_controls_tls_identity() {
    use eggfetch_core::{TlsConfig, TransportHints};

    let ca = tls_fixtures::CertAuthority::new();
    let server = tls_fixtures::TlsTestServer::start(&ca, &["sni-a.example"]).await;
    let server_url = url::Url::parse(&server.url()).unwrap();
    let port = server_url.port().unwrap();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let url = format!("https://pinned.invalid:{port}/");

    // Matching SNI: handshake verifies against the override, not the logical
    // hostname.
    let target = eggfetch_core::ResolvedTarget::new([address]).unwrap();
    let matching = TransportHints {
        resolved_target: Some(target),
        sni_hostname: Some("sni-a.example".to_owned()),
        ..Default::default()
    };
    let mut ok = client
        .get(&url)
        .unwrap()
        .transport_hints(matching)
        .send()
        .await
        .unwrap();
    assert!(ok.is_success());
    let _ = ok.bytes().await.unwrap();

    // Mismatched SNI: certificate verification must fail closed.
    let target = eggfetch_core::ResolvedTarget::new([address]).unwrap();
    let mismatched = TransportHints {
        resolved_target: Some(target),
        sni_hostname: Some("sni-b.example".to_owned()),
        ..Default::default()
    };
    let err = client
        .get(&url)
        .unwrap()
        .transport_hints(mismatched)
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Tls(_) | Error::Connect(_) | Error::HyperClient(_)
        ),
        "SNI mismatch must fail TLS verification, got {err:?}"
    );
    assert_eq!(
        client.transport_metrics().snapshot().direct_dns_attempts,
        0,
        "TLS resolved routing must not use DNS"
    );
}

/// Cached resolved-target hits still perform no origin DNS lookup, including
/// for unresolvable logical hostnames.
#[tokio::test]
async fn resolved_cached_hits_never_use_dns() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let client = Client::builder().build();
    let url = format!("http://never-resolved.invalid:{}/", address.port());
    let err = client
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Connect(_) | Error::HyperClient(_)));
    assert_eq!(
        client.transport_metrics().snapshot().direct_dns_attempts,
        0,
        "failed resolved connect must not fall back to DNS"
    );
}

/// Same-origin redirect retention: a 302 to the same origin retains the
/// snapshot and may reuse the same route entry (one keep-alive connection
/// for both hops).
#[tokio::test]
async fn resolved_same_origin_redirect_reuses_route() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let accepted_clone = accepted.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        accepted_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        for first in [true, false] {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0, "expected a request on the kept-alive connection");
            if first {
                stream
                    .write_all(
                        b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n",
                    )
                    .await
                    .unwrap();
            } else {
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: keep-alive\r\n\r\ndone",
                    )
                    .await
                    .unwrap();
            }
        }
    });

    let client = Client::builder().follow_redirects(true).build();
    let url = format!("http://pinned.invalid:{}/start", address.port());
    let mut response = client
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap();
    assert_eq!(response.bytes().await.unwrap(), "done");
    server.await.unwrap();
    assert_eq!(
        accepted.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "same-origin redirect should reuse the resolved-route connection"
    );
}

/// Cross-origin redirect with a resolved target still fails closed before
/// second-origin dispatch.
#[tokio::test]
async fn resolved_cross_origin_redirect_fails_closed() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf).await;
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://other.invalid/\r\nContent-Length: 0\r\n\r\n",
            )
            .await
            .unwrap();
    });

    let client = Client::builder().follow_redirects(true).build();
    let url = format!("http://pinned.invalid:{}/start", address.port());
    let err = client
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::ResolvedTargetRedirect));
    server.await.unwrap();
}

/// Eviction safety: filling the bounded cache beyond capacity must not make a
/// later different-key request reuse an evicted route's connection. Both
/// servers stay reachable with their own bodies after eviction pressure.
#[tokio::test]
async fn resolved_eviction_does_not_cross_routes() {
    let mut server_a = TestServer::start(&TestServerConfig {
        response_body: Some(b"server-a".to_vec()),
        ..Default::default()
    });
    let mut server_b = TestServer::start(&TestServerConfig {
        response_body: Some(b"server-b".to_vec()),
        ..Default::default()
    });
    let addr_a: std::net::SocketAddr = format!("127.0.0.1:{}", server_a.port()).parse().unwrap();
    let addr_b: std::net::SocketAddr = format!("127.0.0.1:{}", server_b.port()).parse().unwrap();
    let client = Client::builder().build();

    let url_a = format!("http://a.invalid:{}/", server_a.port());
    let url_b = format!("http://b.invalid:{}/", server_b.port());
    for (url, addr, expect) in [
        (url_a.clone(), addr_a, "server-a"),
        (url_b.clone(), addr_b, "server-b"),
    ] {
        let mut resp = client
            .get(&url)
            .unwrap()
            .resolved_addresses([addr])
            .send()
            .await
            .unwrap();
        assert_eq!(resp.bytes().await.unwrap(), expect);
    }

    // Exceed the 64-entry bound with distinct origins pinned to server A.
    // Each is a distinct key (origin differs), forcing arbitrary eviction.
    for index in 0..80 {
        let url = format!("http://evict-{index}.invalid:{}/", server_a.port());
        let mut resp = client
            .get(&url)
            .unwrap()
            .resolved_addresses([addr_a])
            .send()
            .await
            .unwrap();
        let _ = resp.bytes().await.unwrap();
    }

    // Both original routes must still reach their own correct listener with
    // their own bodies, never crossing pools after eviction.
    let mut again_a = client
        .get(&url_a)
        .unwrap()
        .resolved_addresses([addr_a])
        .send()
        .await
        .unwrap();
    assert_eq!(again_a.bytes().await.unwrap(), "server-a");
    let mut again_b = client
        .get(&url_b)
        .unwrap()
        .resolved_addresses([addr_b])
        .send()
        .await
        .unwrap();
    assert_eq!(again_b.bytes().await.unwrap(), "server-b");

    server_a.shutdown();
    server_b.shutdown();
}

/// Construction failure is not cached: an invalid TLS policy fails the
/// resolved dispatch, while an independently built valid client with the same
/// route still succeeds.
#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn resolved_construction_failure_is_not_cached() {
    use eggfetch_core::{TlsConfig, TlsVersion};

    let mut server = TestServer::start(&TestServerConfig::default());
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let url = format!("http://pinned.invalid:{port}/");

    let invalid = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .min_tls_version(TlsVersion::Tls13)
                .max_tls_version(TlsVersion::Tls12)
                .build(),
        )
        .build();
    let err = invalid
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Tls(_)),
        "invalid TLS policy must surface at dispatch, got {err:?}"
    );
    // A second attempt fails the same way (no poisoned success entry).
    let err = invalid
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Tls(_)));

    // A valid client for the same route is unaffected.
    let valid = Client::builder().build();
    let mut ok = valid
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap();
    assert!(ok.is_success());
    let _ = ok.bytes().await.unwrap();

    server.shutdown();
}

/// Cancellation/drop safety: aborting one resolved request and dropping a
/// streaming body must not corrupt the cached client for later requests.
#[tokio::test]
async fn resolved_cancellation_and_drop_keep_cache_usable() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let port = server.port();
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let client = Client::builder().max_connections(4).build();
    let url = format!("http://pinned.invalid:{port}/");

    let delayed = tokio::spawn({
        let client = client.clone();
        let url = url.clone();
        async move {
            client
                .get(&url)
                .unwrap()
                .resolved_addresses([address])
                .send()
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    delayed.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Dropping a streaming body mid-read releases the logical permit without
    // corrupting the cached Hyper client.
    {
        let mut resp = client
            .get(&url)
            .unwrap()
            .resolved_addresses([address])
            .send()
            .await
            .unwrap();
        let mut stream = resp.bytes_stream().unwrap();
        let _ = futures_util::StreamExt::next(&mut stream).await;
        drop(stream);
    }

    let mut ok = client
        .get(&url)
        .unwrap()
        .resolved_addresses([address])
        .send()
        .await
        .unwrap();
    assert!(ok.is_success());
    let _ = ok.bytes().await.unwrap();

    server.shutdown();
}

/// H2 retained-client reuse: same-key resolved requests multiplex through one
/// H2 connection instead of building a fresh route client per request.
#[cfg(feature = "http2")]
#[tokio::test]
async fn resolved_same_key_reuses_h2_connection() {
    use eggfetch_core::HttpVersionPolicy;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let accepted_clone = accepted.clone();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        accepted_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut conn = h2::server::handshake(socket).await.unwrap();
        while let Some(req) = conn.accept().await {
            let (_req, mut respond) = req.unwrap();
            let response = http::Response::builder().status(200).body(()).unwrap();
            let mut send = respond.send_response(response, false).unwrap();
            send.send_data(bytes::Bytes::from("hi"), true).unwrap();
        }
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http2Only)
        .build();
    let url = format!("http://pinned.invalid:{}/", address.port());
    for _ in 0..3 {
        let mut resp = client
            .get(&url)
            .unwrap()
            .resolved_addresses([address])
            .send()
            .await
            .unwrap();
        assert_eq!(resp.version(), http::Version::HTTP_2);
        assert_eq!(resp.bytes().await.unwrap(), "hi");
    }

    // Give the server a moment to observe the single accepted socket.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        accepted.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "same resolved H2 route must reuse one connection"
    );
    server.abort();
}
