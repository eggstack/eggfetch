#![allow(missing_docs, dead_code)]
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

pub struct CertAuthority {
    pub cert: rcgen::Certificate,
    pub key: rcgen::KeyPair,
}

pub struct ClientCertBundle {
    pub cert_der: CertificateDer<'static>,
    pub key_der: PrivateKeyDer<'static>,
    pub key_bytes: Vec<u8>,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

impl CertAuthority {
    pub fn new() -> Self {
        let mut params = rcgen::CertificateParams::default();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        Self { cert, key }
    }

    pub fn cert_der(&self) -> CertificateDer<'static> {
        CertificateDer::from(self.cert.der().to_vec())
    }

    pub fn cert_pem(&self) -> Vec<u8> {
        self.cert.pem().into_bytes()
    }

    pub fn generate_server_cert(
        &self,
        hostnames: &[&str],
    ) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        let sans: Vec<String> = hostnames.iter().map(|s| s.to_string()).collect();
        let mut params = rcgen::CertificateParams::new(sans).unwrap();
        params.is_ca = rcgen::IsCa::NoCa;
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.cert, &self.key).unwrap();
        let cert_der = CertificateDer::from(cert.der().to_vec());
        let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        (cert_der, key_der)
    }

    pub fn generate_client_cert(&self) -> ClientCertBundle {
        let mut params =
            rcgen::CertificateParams::new(vec!["client.example.com".to_string()]).unwrap();
        params.is_ca = rcgen::IsCa::NoCa;
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.cert, &self.key).unwrap();
        let key_bytes = key.serialize_der();
        let cert_der = CertificateDer::from(cert.der().to_vec());
        let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_bytes.clone()));
        ClientCertBundle {
            cert_der,
            key_der,
            key_bytes,
            cert_pem: cert.pem().into_bytes(),
            key_pem: key.serialize_pem().into_bytes(),
        }
    }
}

pub struct TlsTestServer {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
}

impl TlsTestServer {
    pub async fn start(ca: &CertAuthority, hostnames: &[&str]) -> Self {
        let (cert_der, key_der) = ca.generate_server_cert(hostnames);
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        Self::start_with_config(server_config).await
    }

    pub async fn start_self_signed(hostnames: &[&str]) -> Self {
        let sans: Vec<String> = hostnames.iter().map(|s| s.to_string()).collect();
        let mut params = rcgen::CertificateParams::new(sans).unwrap();
        params.is_ca = rcgen::IsCa::NoCa;
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let cert_der = CertificateDer::from(cert.der().to_vec());
        let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        Self::start_with_config(server_config).await
    }

    async fn start_with_config(server_config: rustls::ServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((tcp_stream, _)) => {
                                let acceptor = acceptor.clone();
                                tokio::spawn(async move {
                                    let tls_stream = match acceptor.accept(tcp_stream).await {
                                        Ok(s) => s,
                                        Err(_) => return,
                                    };
                                    let mut buf_reader = BufReader::new(tls_stream);
                                    let mut request_line = String::new();
                                    if buf_reader.read_line(&mut request_line).await.is_err() {
                                        return;
                                    }
                                    loop {
                                        let mut line = String::new();
                                        if buf_reader.read_line(&mut line).await.is_err() || line.trim().is_empty() {
                                            break;
                                        }
                                    }
                                    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
                                    let mut stream = buf_reader.into_inner();
                                    let _ = stream.write_all(response).await;
                                    let _ = stream.flush().await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self { port, shutdown_tx }
    }

    pub fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for TlsTestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct MtlsTestServer {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
}

impl MtlsTestServer {
    pub async fn start(ca: &CertAuthority, hostnames: &[&str]) -> Self {
        let (cert_der, key_der) = ca.generate_server_cert(hostnames);

        let mut root_store = rustls::RootCertStore::empty();
        root_store.add(ca.cert_der()).unwrap();

        let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .unwrap();

        let server_config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((tcp_stream, _)) => {
                                let acceptor = acceptor.clone();
                                tokio::spawn(async move {
                                    let tls_stream = match acceptor.accept(tcp_stream).await {
                                        Ok(s) => s,
                                        Err(_) => return,
                                    };
                                    let mut buf_reader = BufReader::new(tls_stream);
                                    let mut request_line = String::new();
                                    if buf_reader.read_line(&mut request_line).await.is_err() {
                                        return;
                                    }
                                    loop {
                                        let mut line = String::new();
                                        if buf_reader.read_line(&mut line).await.is_err() || line.trim().is_empty() {
                                            break;
                                        }
                                    }
                                    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
                                    let mut stream = buf_reader.into_inner();
                                    let _ = stream.write_all(response).await;
                                    let _ = stream.flush().await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self { port, shutdown_tx }
    }

    pub fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for MtlsTestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
