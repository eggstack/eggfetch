#![allow(missing_docs, clippy::large_futures)]

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{
    DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer, Error, RetryPolicy,
    TransportHints,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

#[cfg(feature = "tls-rustls")]
mod tls_fixtures;

#[derive(Clone)]
struct RecordingDialer {
    address: std::net::SocketAddr,
    targets: Arc<Mutex<Vec<DialTarget>>>,
    fail: bool,
}

impl Dialer for RecordingDialer {
    fn dial(&self, target: DialTarget) -> DialFuture<'_> {
        let address = self.address;
        let targets = self.targets.clone();
        let fail = self.fail;
        Box::pin(async move {
            targets.lock().unwrap().push(target);
            if fail {
                return Err(DialError::new(
                    DialErrorKind::Rejected,
                    "synthetic route rejected",
                ));
            }
            let stream = TcpStream::connect(address).await.map_err(|error| {
                DialError::with_source(DialErrorKind::Connection, "synthetic route failed", error)
            })?;
            Ok(Box::new(stream) as DialStream)
        })
    }
}

async fn read_headers(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).await.unwrap();
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            return String::from_utf8(request).unwrap();
        }
    }
}

#[tokio::test]
async fn custom_dialer_preserves_logical_authority_and_effective_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_headers(&mut stream).await;
        assert!(
            request
                .to_ascii_lowercase()
                .contains("host: logical.invalid:"),
            "{request:?}"
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });

    let targets = Arc::new(Mutex::new(Vec::new()));
    let dialer = RecordingDialer {
        address,
        targets: targets.clone(),
        fail: false,
    };
    let client = eggfetch_core::Client::builder().dialer(dialer).build();
    let url = format!("http://logical.invalid:{}/", address.port());
    let mut response = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(response.bytes().await.unwrap(), "ok");
    assert_eq!(targets.lock().unwrap()[0].host(), "logical.invalid");
    assert_eq!(targets.lock().unwrap()[0].port(), address.port());
    server.await.unwrap();
}

#[tokio::test]
async fn dialer_failure_is_returned_without_direct_fallback() {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: targets.clone(),
            fail: true,
        })
        .build();
    let error = client
        .get("http://127.0.0.1:1/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::CustomTransport(_)));
    assert_eq!(targets.lock().unwrap().len(), 1);
    assert_eq!(
        client
            .transport_metrics()
            .snapshot()
            .direct_connector_attempts,
        0
    );

    assert!(matches!(client.get("not a URL"), Err(Error::InvalidUrl(_))));
    assert_eq!(targets.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn custom_dialer_uses_default_http_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = read_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();

    let mut response = client
        .get("http://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(response.bytes().await.unwrap(), "ok");
    assert_eq!(targets.lock().unwrap()[0].port(), 80);
    server.await.unwrap();
}

#[tokio::test]
async fn custom_dialer_keeps_logical_destination_with_wire_target_override() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_headers(&mut stream).await;
        assert!(request.starts_with("GET /override HTTP/1.1"), "{request:?}");
        assert!(request
            .to_ascii_lowercase()
            .contains("host: logical.invalid:"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();
    let hints = TransportHints {
        target: Some(Bytes::from_static(b"/override")),
        ..TransportHints::default()
    };
    let mut response = client
        .get(&format!(
            "http://logical.invalid:{}/original",
            address.port()
        ))
        .unwrap()
        .transport_hints(hints)
        .send()
        .await
        .unwrap();
    assert_eq!(response.bytes().await.unwrap(), "ok");
    assert_eq!(targets.lock().unwrap()[0].host(), "logical.invalid");
    server.await.unwrap();
}

#[tokio::test]
async fn custom_dialer_redirects_keep_logical_targets_and_strip_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let port = address.port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let server = tokio::spawn(async move {
        for response in [
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://origin-b.invalid:{port}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_owned(),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_headers(&mut stream).await;
            captured.lock().unwrap().push(request);
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });

    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .follow_redirects(true)
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();
    let mut response = client
        .get(&format!("http://origin-a.invalid:{port}/start"))
        .unwrap()
        .header("authorization", "Bearer secret")
        .header("cookie", "session=secret")
        .send()
        .await
        .unwrap();

    assert_eq!(response.text().await.unwrap(), "ok");
    {
        let targets = targets.lock().unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].host(), "origin-a.invalid");
        assert_eq!(targets[1].host(), "origin-b.invalid");
        let requests = requests.lock().unwrap();
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer secret"));
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("cookie: session=secret"));
        let second = requests[1].to_ascii_lowercase();
        assert!(!second.contains("authorization:"), "{second}");
        assert!(!second.contains("cookie:"), "{second}");
    }
    server.await.unwrap();
}

#[tokio::test]
async fn custom_dialer_is_reused_for_explicit_retries() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for (status, body) in [("503 Service Unavailable", "retry"), ("200 OK", "ok")] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_headers(&mut stream).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let targets = Arc::new(Mutex::new(Vec::new()));
    let policy = RetryPolicy::builder()
        .max_attempts(2)
        .initial_delay(Duration::ZERO)
        .max_delay(Duration::ZERO)
        .build();
    let client = eggfetch_core::Client::builder()
        .retry(policy)
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();

    let mut response = tokio::time::timeout(
        Duration::from_secs(2),
        client.get("http://logical.invalid/").unwrap().send(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.text().await.unwrap(), "ok");
    assert_eq!(targets.lock().unwrap().len(), 2);
    server.await.unwrap();
}

#[tokio::test]
async fn separate_clients_with_different_dialers_do_not_share_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_headers(&mut stream).await;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        }
    });
    let first_targets = Arc::new(Mutex::new(Vec::new()));
    let second_targets = Arc::new(Mutex::new(Vec::new()));
    let first = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address,
            targets: first_targets.clone(),
            fail: false,
        })
        .build();
    let second = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address,
            targets: second_targets.clone(),
            fail: false,
        })
        .build();
    let mut first_response = first
        .get(&format!("http://logical.invalid:{}/", address.port()))
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(first_response.bytes().await.unwrap(), "ok");
    let mut second_response = second
        .get(&format!("http://logical.invalid:{}/", address.port()))
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(second_response.bytes().await.unwrap(), "ok");
    assert_eq!(first_targets.lock().unwrap().len(), 1);
    assert_eq!(second_targets.lock().unwrap().len(), 1);
    server.await.unwrap();
}

#[tokio::test]
async fn custom_dialer_rejects_resolved_and_socket_routing_before_io() {
    let resolved_targets = Arc::new(Mutex::new(Vec::new()));
    let resolved_client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: resolved_targets.clone(),
            fail: false,
        })
        .build();
    let resolved_error = resolved_client
        .get("http://logical.invalid/")
        .unwrap()
        .resolved_addresses(["127.0.0.1:80".parse().unwrap()])
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(resolved_error, Error::Unsupported(message) if message.contains("resolved destinations"))
    );
    assert!(resolved_targets.lock().unwrap().is_empty());

    let socket_targets = Arc::new(Mutex::new(Vec::new()));
    let socket_client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: socket_targets.clone(),
            fail: false,
        })
        .local_address("127.0.0.1:0".parse().unwrap())
        .build();
    let socket_error = socket_client
        .get("http://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(socket_error, Error::Unsupported(message) if message.contains("local-address"))
    );
    assert!(socket_targets.lock().unwrap().is_empty());

    let option_targets = Arc::new(Mutex::new(Vec::new()));
    let option_client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: option_targets.clone(),
            fail: false,
        })
        .socket_options(vec![eggfetch_core::SocketOption {
            level: 6,
            option: 1,
            value: 1_i32.to_ne_bytes().to_vec(),
            kind: Some(eggfetch_core::SocketOptionKind::TcpNoDelay),
        }])
        .build();
    let option_error = option_client
        .get("http://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(option_error, Error::Unsupported(message) if message.contains("local-address"))
    );
    assert!(option_targets.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn custom_dialer_rejects_uds_before_io() {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: targets.clone(),
            fail: false,
        })
        .uds_path("/tmp/eggfetched-custom-dialer-conflict.sock".to_owned())
        .build();
    let error = client
        .get("http://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(message) if message.contains("Unix-domain")));
    assert!(targets.lock().unwrap().is_empty());
}

#[cfg(feature = "http3")]
#[tokio::test]
async fn custom_dialer_rejects_http3_only_before_io() {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: targets.clone(),
            fail: false,
        })
        .http_version_policy(eggfetch_core::HttpVersionPolicy::Http3Only)
        .build();
    let error = client
        .get("https://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(message) if message.contains("HTTP/3")));
    assert!(targets.lock().unwrap().is_empty());
}

#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn custom_dialer_keeps_eggfetched_tls_hostname_identity() {
    let ca = tls_fixtures::CertAuthority::new();
    let server = tls_fixtures::TlsTestServer::start(&ca, &["localhost"]).await;
    let server_url = url::Url::parse(&server.url()).unwrap();
    let address = format!("127.0.0.1:{}", server_url.port().unwrap())
        .parse()
        .unwrap();
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .tls_config(
            eggfetch_core::TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();
    let mut response = client
        .get(&format!("https://localhost:{}/", address.port()))
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "OK");
    assert_eq!(targets.lock().unwrap()[0].host(), "localhost");
}

#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn custom_dialer_honors_sni_override() {
    let ca = tls_fixtures::CertAuthority::new();
    let server = tls_fixtures::TlsTestServer::start(&ca, &["override.example"]).await;
    let address = format!(
        "127.0.0.1:{}",
        server.url().parse::<url::Url>().unwrap().port().unwrap()
    )
    .parse()
    .unwrap();
    let targets = Arc::new(Mutex::new(Vec::new()));
    let client = eggfetch_core::Client::builder()
        .tls_config(
            eggfetch_core::TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .dialer(RecordingDialer {
            address,
            targets: targets.clone(),
            fail: false,
        })
        .build();
    let hints = TransportHints {
        sni_hostname: Some("override.example".to_owned()),
        ..TransportHints::default()
    };
    let mut response = client
        .get(&format!("https://logical.invalid:{}/", address.port()))
        .unwrap()
        .transport_hints(hints)
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "OK");
    assert_eq!(targets.lock().unwrap()[0].host(), "logical.invalid");
    server.shutdown();
}

struct DropNotifyingStream {
    inner: TcpStream,
    drops: Arc<AtomicUsize>,
}

impl AsyncRead for DropNotifyingStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for DropNotifyingStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl Drop for DropNotifyingStream {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct CancellationDialer {
    address: std::net::SocketAddr,
    connected: Arc<Notify>,
    drops: Arc<AtomicUsize>,
}

impl Dialer for CancellationDialer {
    fn dial(&self, _target: DialTarget) -> DialFuture<'_> {
        let address = self.address;
        let connected = self.connected.clone();
        let drops = self.drops.clone();
        Box::pin(async move {
            let stream = TcpStream::connect(address).await.map_err(|error| {
                DialError::with_source(DialErrorKind::Connection, "connect", error)
            })?;
            connected.notify_one();
            Ok(Box::new(DropNotifyingStream {
                inner: stream,
                drops,
            }) as DialStream)
        })
    }
}

#[tokio::test]
async fn cancelling_a_request_drops_the_dialer_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
    });
    let connected = Arc::new(Notify::new());
    let drops = Arc::new(AtomicUsize::new(0));
    let client = eggfetch_core::Client::builder()
        .dialer(CancellationDialer {
            address,
            connected: connected.clone(),
            drops: drops.clone(),
        })
        .build();
    let request =
        tokio::spawn(async move { client.get("http://logical.invalid/").unwrap().send().await });
    tokio::time::timeout(Duration::from_secs(1), connected.notified())
        .await
        .unwrap();
    request.abort();
    let _ = request.await;
    tokio::time::timeout(Duration::from_secs(1), async {
        while drops.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    server.await.unwrap();
}

#[cfg(feature = "proxy")]
#[tokio::test]
async fn custom_dialer_and_proxy_fail_before_network_io() {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let proxy = eggfetch_core::Proxy::http("http://127.0.0.1:1").unwrap();
    let client = eggfetch_core::Client::builder()
        .dialer(RecordingDialer {
            address: "127.0.0.1:1".parse().unwrap(),
            targets: targets.clone(),
            fail: false,
        })
        .proxy(proxy)
        .build();
    let error = client
        .get("http://logical.invalid/")
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(message) if message.contains("custom dialing")));
    assert!(targets.lock().unwrap().is_empty());
}
