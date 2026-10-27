#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
)]

//! End-to-end assertion of the CLI's `--http2` negotiation-success path.
//!
//! `protocol_capability.rs` covers the rejection half: a `--http2` flag in a
//! build without the `http2` capability must fail closed before any
//! connection is attempted. This file covers the other half — a build that
//! *has* the capability must actually negotiate HTTP/2 with the server, and
//! the request must complete over that negotiated protocol rather than being
//! silently downgraded to HTTP/1.1.
//!
//! The fixture is a TLS server that advertises `h2` as its only ALPN
//! protocol and speaks HTTP/2 over it. It records the negotiated ALPN
//! protocol and the requests it served, so the test asserts the protocol
//! that was actually used instead of inferring it from the exit code.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use bytes::Bytes;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// The body the fixture serves over the negotiated HTTP/2 connection.
const H2_BODY: &str = "h2-ok";

/// The only ALPN protocol the fixture advertises.
const H2_ALPN: &[u8] = b"h2";

/// A TLS server that speaks HTTP/2 over ALPN and records what it served.
struct H2Server {
    port: u16,
    ca_pem: Vec<u8>,
    served: Arc<AtomicUsize>,
    negotiated_alpn: Arc<Mutex<Option<Vec<u8>>>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl H2Server {
    fn start() -> Self {
        let (ca_pem, cert_der, key_der) = issue_certificates();
        let mut server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("server config");
        server_config.alpn_protocols = vec![H2_ALPN.to_vec()];

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime");
        let listener =
            runtime.block_on(async { TcpListener::bind("127.0.0.1:0").await.expect("bind") });
        let port = listener.local_addr().expect("addr").port();

        let served = Arc::new(AtomicUsize::new(0));
        let negotiated_alpn = Arc::new(Mutex::new(None));
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        let served_task = served.clone();
        let alpn_task = negotiated_alpn.clone();
        let handle = thread::spawn(move || {
            runtime.block_on(async move {
                let acceptor = TlsAcceptor::from(Arc::new(server_config));
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        accepted = listener.accept() => {
                            let Ok((tcp, _peer)) = accepted else { break };
                            let acceptor = acceptor.clone();
                            let served = served_task.clone();
                            let negotiated_alpn = alpn_task.clone();
                            tokio::spawn(async move {
                                let Ok(tls) = acceptor.accept(tcp).await else { return };
                                let alpn = tls.get_ref().1.alpn_protocol().map(<[u8]>::to_vec);
                                let Ok(mut connection) = h2::server::handshake(tls).await else { return };
                                *negotiated_alpn.lock().expect("alpn lock") = alpn;
                                while let Some(result) = connection.accept().await {
                                    let Ok((_request, mut respond)) = result else { return };
                                    served.fetch_add(1, Ordering::SeqCst);
                                    let response = http::Response::builder()
                                        .status(200)
                                        .header("content-length", H2_BODY.len().to_string())
                                        .body(())
                                        .expect("response");
                                    let Ok(mut send) = respond.send_response(response, false) else { return };
                                    let _ = send.send_data(Bytes::from_static(H2_BODY.as_bytes()), true);
                                }
                            });
                        }
                    }
                }
            });
        });

        Self {
            port,
            ca_pem,
            served,
            negotiated_alpn,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    /// The PEM of the CA that signed this fixture's server certificate.
    ///
    /// The client must trust exactly this CA, so the value has to come from
    /// the running fixture: issuing a second CA would produce a different
    /// trust anchor and the request would fail for the wrong reason.
    fn ca_pem(&self) -> &[u8] {
        &self.ca_pem
    }

    /// The URL the CLI must request. The fixture certificate carries
    /// `127.0.0.1` as an IP subject alternative name, so the request needs no
    /// name resolution and no DNS-dependent host mapping.
    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }

    fn served_requests(&self) -> usize {
        self.served.load(Ordering::SeqCst)
    }

    fn negotiated_alpn(&self) -> Option<Vec<u8>> {
        self.negotiated_alpn.lock().expect("alpn lock").clone()
    }
}

impl Drop for H2Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Issue a private CA and a server certificate for `127.0.0.1` signed by it.
fn issue_certificates() -> (
    Vec<u8>,
    rustls::pki_types::CertificateDer<'static>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let mut ca_params = rcgen::CertificateParams::default();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = rcgen::KeyPair::generate().expect("ca key");
    let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");

    let mut server_params =
        rcgen::CertificateParams::new(vec!["127.0.0.1".to_string()]).expect("server params");
    server_params.is_ca = rcgen::IsCa::NoCa;
    let server_key = rcgen::KeyPair::generate().expect("server key");
    let server_cert = server_params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .expect("server cert");

    let cert_der = rustls::pki_types::CertificateDer::from(server_cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(server_key.serialize_der()),
    );
    (ca_cert.pem().into_bytes(), cert_der, key_der)
}

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("failed to get test binary path")
        .parent()
        .expect("failed to get test binary dir")
        .parent()
        .expect("failed to get target dir")
        .to_path_buf();
    path.push("eggfetch");
    #[cfg(windows)]
    {
        path.set_extension("exe");
    }
    path
}

fn run_cli(args: &[&str]) -> (String, String, Option<i32>) {
    let bin = binary_path();
    let output = std::process::Command::new(&bin)
        .args(args)
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {}: {e}", bin.display()));

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code(),
    )
}

/// Write the fixture's CA certificate to a PEM file and return the file.
///
/// The `NamedTempFile` is returned rather than just its path: dropping it
/// deletes the file, and the CLI reads it after this function returns.
fn write_ca_file(ca_pem: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("temp file");
    file.write_all(ca_pem).expect("write ca");
    file.flush().expect("flush ca");
    file
}

#[test]
#[cfg(feature = "http2")]
fn http2_flag_negotiates_http2_with_an_h2_server() {
    let server = H2Server::start();
    let ca_file = write_ca_file(server.ca_pem());
    let url = server.url();

    let (stdout, stderr, code) = run_cli(&[
        "--http2",
        "--cacert",
        &ca_file.path().to_string_lossy(),
        &url,
    ]);

    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert!(
        stdout.contains(H2_BODY),
        "the body must arrive over the negotiated connection: {stdout}"
    );
    assert_eq!(
        server.served_requests(),
        1,
        "the fixture must have served exactly one request"
    );
    assert_eq!(
        server.negotiated_alpn().as_deref(),
        Some(H2_ALPN),
        "the connection must have negotiated h2"
    );
}

#[test]
#[cfg(feature = "http2")]
fn http1_flag_cannot_use_the_h2_only_fixture() {
    // The negative control: the fixture advertises `h2` and nothing else, so
    // an HTTP/1.1-only client has no protocol to negotiate and must fail.
    // If this ever succeeded, the success test above would prove nothing.
    let server = H2Server::start();
    let ca_file = write_ca_file(server.ca_pem());
    let url = server.url();

    let (stdout, stderr, code) = run_cli(&[
        "--http1",
        "--cacert",
        &ca_file.path().to_string_lossy(),
        &url,
    ]);

    assert_ne!(
        code,
        Some(0),
        "an HTTP/1.1 client must not succeed against an h2-only server: {stdout}"
    );
    assert!(
        !stdout.contains(H2_BODY),
        "the fixture body must not be served over HTTP/1.1: {stdout}"
    );
    // The failure must come from the protocol mismatch, not from a broken
    // trust setup: a CA-bundle error here would make the control pass for
    // the wrong reason and hide a broken fixture.
    assert!(
        !stderr.contains("CA bundle"),
        "the control must fail on the protocol, not on the trust store: {stderr}"
    );
    assert_eq!(
        server.served_requests(),
        0,
        "the fixture must not have served an HTTP/1.1 request"
    );
}
