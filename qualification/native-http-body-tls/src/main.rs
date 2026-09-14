use std::convert::Infallible;
use std::sync::Arc;

use bytes::Bytes;
use eggfetch_core::{
    Client, DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer,
    ClientIdentity, NativeRequestOptions, TlsConfig, TrustStore,
};
use http_body::{Body, Frame};
use http_body_util::StreamBody;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct LocalDialer {
    address: std::net::SocketAddr,
}

impl Dialer for LocalDialer {
    fn dial(&self, _target: DialTarget) -> DialFuture<'_> {
        let address = self.address;
        Box::pin(async move {
            TcpStream::connect(address)
                .await
                .map(|stream| Box::new(stream) as DialStream)
                .map_err(|error| {
                    DialError::with_source(
                        DialErrorKind::Connection,
                        "fixture connect failed",
                        error,
                    )
                })
        })
    }
}

fn ca() -> (rcgen::Certificate, KeyPair) {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key = KeyPair::generate().expect("CA key");
    (params.self_signed(&key).expect("CA certificate"), key)
}

fn leaf(
    common_name: &str,
    issuer: &rcgen::Certificate,
    issuer_key: &KeyPair,
) -> (rcgen::Certificate, KeyPair) {
    let mut params = CertificateParams::new(vec![common_name.to_owned()]).expect("leaf params");
    params.is_ca = IsCa::NoCa;
    let key = KeyPair::generate().expect("leaf key");
    (
        params
            .signed_by(&key, issuer, issuer_key)
            .expect("leaf certificate"),
        key,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let process_provider_before = rustls::crypto::CryptoProvider::get_default()
        .map(|provider| Arc::as_ptr(provider));
    let (base_ca, _base_key) = ca();
    let (private_ca, private_key) = ca();
    let (server_cert, server_key) = leaf("fixture.invalid", &private_ca, &private_key);
    let (client_cert, client_key) = leaf("client.fixture.invalid", &private_ca, &private_key);
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());

    let mut client_roots = rustls::RootCertStore::empty();
    client_roots.add(rustls::pki_types::CertificateDer::from(
        private_ca.der().to_vec(),
    ))?;
    let client_verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(client_roots),
        provider.clone(),
    )
    .build()?;
    let server_config = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(
                server_cert.der().to_vec(),
            )],
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                server_key.serialize_der(),
            )),
        )?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut stream = acceptor.accept(socket).await?;
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(5).any(|window| window == b"0\r\n\r\n") {
                break;
            }
        }
        assert!(request.windows(5).any(|window| window == b"hello"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: x-fixture\r\n\r\n5\r\nfirst\r\n6\r\nsecond\r\n0\r\nx-fixture: passed\r\n\r\n",
            )
            .await?;
        stream.flush().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });

    let tls_config = TlsConfig::builder()
        .trust_store(TrustStore::Custom(vec![
            rustls::pki_types::CertificateDer::from(base_ca.der().to_vec()),
        ]))
        .additional_ca_certificate_der(vec![private_ca.der().to_vec()])?
        .client_identity(ClientIdentity::Pem {
            cert_chain: vec![rustls::pki_types::CertificateDer::from(
                client_cert.der().to_vec(),
            )],
            private_key_der: client_key.serialize_der(),
            key_label: "PRIVATE KEY".to_owned(),
        })
        .crypto_provider(provider)
        .build();
    let client = Client::builder()
        .tls_config(tls_config)
        .dialer(LocalDialer { address })
        .build();

    let body = StreamBody::new(futures_util::stream::iter([Ok::<_, Infallible>(
        Frame::data(Bytes::from_static(b"hello")),
    )]));
    let request = http::Request::post("https://fixture.invalid/")
        .body(body)
        .expect("fixture request");
    let response = client
        .execute_http_body(request, NativeRequestOptions::default())
        .await?;
    let mut data = Vec::new();
    let mut trailer = None;
    let mut body = Box::pin(response.into_body());
    while let Some(frame) = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        let frame = frame?;
        match frame.into_data() {
            Ok(bytes) => data.extend_from_slice(&bytes),
            Err(frame) => trailer = frame.into_trailers().ok(),
        }
    }
    assert_eq!(data, b"firstsecond");
    assert_eq!(trailer.unwrap().get("x-fixture").unwrap(), "passed");
    assert_eq!(
        process_provider_before,
        rustls::crypto::CryptoProvider::get_default().map(Arc::as_ptr),
        "explicit provider use must not mutate the process-global default",
    );
    server.await??;
    Ok(())
}
