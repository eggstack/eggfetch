#![allow(missing_docs, dead_code, unused_mut, clippy::all)]
#![cfg(feature = "tls-rustls")]

mod tls_fixtures;

use eggfetch_core::{Client, TlsConfig, TlsVersion};
use tls_fixtures::{CertAuthority, MtlsTestServer, TlsTestServer};

#[tokio::test]
async fn default_verified_https_succeeds() {
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
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn untrusted_cert_fails() {
    let server_ca = CertAuthority::new();
    let server = TlsTestServer::start(&server_ca, &["localhost"]).await;

    let other_ca = CertAuthority::new();
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&other_ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let err = client.get(&server.url()).unwrap().send().await.unwrap_err();
    assert!(
        err.kind() == "tls" || err.kind() == "hyper_client" || err.kind() == "connect",
        "expected TLS/connection error, got kind={}: {err}",
        err.kind()
    );
}

#[tokio::test]
async fn hostname_mismatch_fails() {
    let ca = CertAuthority::new();
    let server = TlsTestServer::start(&ca, &["wrong-hostname"]).await;
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let err = client.get(&server.url()).unwrap().send().await.unwrap_err();
    assert!(
        err.kind() == "tls" || err.kind() == "hyper_client" || err.kind() == "connect",
        "expected TLS/hostname verification error, got kind={}: {err}",
        err.kind()
    );
}

#[tokio::test]
async fn custom_ca_succeeds() {
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
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn verify_false_succeeds_with_self_signed() {
    let server = TlsTestServer::start_self_signed(&["localhost", "127.0.0.1"]).await;
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_certs(true)
                .build(),
        )
        .build();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn client_certificate_accepted_by_mtls_server() {
    let ca = CertAuthority::new();
    let server = MtlsTestServer::start(&ca, &["localhost", "127.0.0.1"]).await;

    let client_cert = ca.generate_client_cert();

    let cert_path = tempfile::NamedTempFile::new().unwrap();
    let key_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(cert_path.path(), &client_cert.cert_pem).unwrap();
    std::fs::write(key_path.path(), &client_cert.key_pem).unwrap();

    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .client_cert_path(cert_path.path(), key_path.path())
                .unwrap()
                .build(),
        )
        .build();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn client_certificate_rejected_by_mtls_server() {
    let ca = CertAuthority::new();
    let server = MtlsTestServer::start(&ca, &["localhost", "127.0.0.1"]).await;

    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let err = client.get(&server.url()).unwrap().send().await.unwrap_err();
    assert!(
        err.kind() == "tls" || err.kind() == "hyper_client" || err.kind() == "connect",
        "expected TLS/certificate error from mTLS rejection, got kind={}: {err}",
        err.kind()
    );
}

#[tokio::test]
async fn tls_version_policy() {
    let ca = CertAuthority::new();
    let server = TlsTestServer::start(&ca, &["localhost", "127.0.0.1"]).await;
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .min_tls_version(TlsVersion::Tls13)
                .max_tls_version(TlsVersion::Tls13)
                .build(),
        )
        .build();
    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn no_fallback_after_validation_failure() {
    let ca = CertAuthority::new();
    let server = TlsTestServer::start(&ca, &["wrong-hostname"]).await;
    let client = Client::builder()
        .tls_config(
            TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .build();
    let result = client.get(&server.url()).unwrap().send().await;
    assert!(result.is_err(), "expected failure, got success");
    let err = result.unwrap_err();
    assert!(
        err.kind() == "tls" || err.kind() == "hyper_client" || err.kind() == "connect",
        "expected certificate/hostname verification error, got kind={}: {err}",
        err.kind()
    );
}

#[tokio::test]
async fn debug_output_no_private_key() {
    let ca = CertAuthority::new();
    let client_cert = ca.generate_client_cert();

    let identity = eggfetch_core::ClientIdentity::Pem {
        cert_chain: vec![client_cert.cert_der],
        private_key_der: client_cert.key_bytes,
        key_label: "PRIVATE KEY".to_string(),
    };

    let tls_config = TlsConfig::builder()
        .ca_certificate_pem(&ca.cert_pem())
        .unwrap()
        .client_identity(identity)
        .build();

    let debug = format!("{tls_config:?}");
    assert!(
        debug.contains("has_client_identity: true"),
        "debug output should indicate client identity presence: {debug}"
    );

    let lower = debug.to_lowercase();
    assert!(
        !lower.contains("private"),
        "debug output should not contain private key data: {debug}"
    );
    assert!(
        !lower.contains("pkcs8"),
        "debug output should not contain PKCS8 data: {debug}"
    );
}
