//! Stable compile contracts for the supported `eggfetch-core` profiles.
//!
//! These are deliberately small use-site checks.  The exact public API
//! snapshots live under `compat/rust-public-api/`; this fixture keeps the
//! highest-value import paths covered by ordinary stable Cargo checks.

use eggfetch_core::{
    Client, ClientBuilder, HttpVersionPolicy, Limits, NativeHttpService, NativeRequestOptions,
    Timeout, TransportHints,
};

#[test]
fn core_root_contracts_compile() {
    let _: fn() -> Client = Client::new;
    let _: fn() -> ClientBuilder = Client::builder;
    let client = Client::new();
    let _service = NativeHttpService::new(client);
    let _ = HttpVersionPolicy::default();
    let _ = Limits::default();
    let _ = Timeout::disabled();
    let _ = Timeout::builder();
    let _ = NativeRequestOptions::default();
    let _ = TransportHints::default();
}

#[cfg(feature = "high-level-url")]
#[test]
fn high_level_request_contracts_compile() {
    let client = Client::new();
    let request = client.get("https://example.com").expect("valid URL");
    let _ = request.header("user-agent", "eggfetch");
    let _ = client.request(http::Method::GET, "https://example.com");
}

#[cfg(feature = "advanced-routing")]
#[test]
fn advanced_routing_contracts_compile() {
    use std::net::SocketAddr;

    let target = eggfetch_core::ResolvedTarget::new(["127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("socket address")])
    .expect("valid resolved target");
    let hints = eggfetch_core::TransportHints {
        resolved_target: Some(target),
        sni_hostname: Some("example.com".to_owned()),
        ..Default::default()
    };
    let _ = hints;
}

#[cfg(feature = "proxy")]
#[test]
fn proxy_contracts_compile() {
    let _ = eggfetch_core::Proxy::all("http://proxy.example:8080");
    let _ = eggfetch_core::NoProxy::parse("localhost,127.0.0.1");
}

#[cfg(feature = "tls-rustls")]
#[test]
fn tls_contracts_compile() {
    let _ = eggfetch_core::TlsConfig::default();
    let _ = eggfetch_core::TlsConfigBuilder::new();
    let _ = eggfetch_core::TrustStore::WebPkiOnly;
}

#[cfg(feature = "logical-retry")]
#[test]
fn retry_contracts_compile() {
    let _ = eggfetch_core::RetryPolicy::default();
    let _ = eggfetch_core::RetryPolicyBuilder::new();
}

#[cfg(feature = "redirects")]
#[test]
fn redirect_contracts_compile() {
    let _ = eggfetch_core::RedirectPolicy::default();
}
