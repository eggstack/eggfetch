#![allow(missing_docs, clippy::large_futures)]

use std::sync::{Arc, Mutex};

use eggfetch_core::{DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer, Error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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
