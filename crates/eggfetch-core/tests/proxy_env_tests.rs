#![allow(
    missing_docs,
    dead_code,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls
)]
//! Opt-in environment proxy resolution tests.
//!
//! Uses supplied environment snapshots only — no process-environment
//! mutation — and proves the policy layer without public network access:
//! selection, precedence, fallback, bypass, fail-closed redaction,
//! snapshot stability, and builder opt-in. Transport through a real proxy
//! is already covered by `proxy_tests.rs` over explicit `Proxy` values.

#![cfg(feature = "proxy")]

use eggfetch_core::{Client, ProxyEnvironment, ProxyRule};

fn https_url(s: &str) -> url::Url {
    url::Url::parse(s).unwrap()
}

#[test]
fn env_https_proxy_selected_for_https_target() {
    let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://proxy.example:8080")]);
    let proxy = env
        .resolve(&https_url("https://example.com/file"))
        .unwrap()
        .expect("https proxy selected");
    assert_eq!(proxy.uri().host_str(), Some("proxy.example"));
    assert_eq!(proxy.rule(), ProxyRule::Https);
}

#[test]
fn env_lowercase_precedence_documented() {
    let env = ProxyEnvironment::from_map([
        ("HTTPS_PROXY", "http://upper.example:8080"),
        ("https_proxy", "http://lower.example:8080"),
    ]);
    let proxy = env
        .resolve(&https_url("https://example.com/file"))
        .unwrap()
        .unwrap();
    assert_eq!(proxy.uri().host_str(), Some("lower.example"));
}

#[test]
fn env_all_proxy_fallback_for_both_schemes() {
    let env = ProxyEnvironment::from_map([("all_proxy", "http://fallback.example:8080")]);
    assert_eq!(
        env.resolve(&https_url("https://example.com/a"))
            .unwrap()
            .unwrap()
            .uri()
            .host_str(),
        Some("fallback.example")
    );
    assert_eq!(
        env.resolve(&https_url("http://example.com/a"))
            .unwrap()
            .unwrap()
            .uri()
            .host_str(),
        Some("fallback.example")
    );
}

#[test]
fn env_no_proxy_bypass_exact_and_domain() {
    let env = ProxyEnvironment::from_map([
        ("HTTPS_PROXY", "http://proxy.example:8080"),
        ("NO_PROXY", "example.com, .internal.example"),
    ]);
    assert!(env
        .resolve(&https_url("https://example.com/a"))
        .unwrap()
        .is_none());
    assert!(env
        .resolve(&https_url("https://svc.internal.example/a"))
        .unwrap()
        .is_none());
    assert!(env
        .resolve(&https_url("https://other.example/a"))
        .unwrap()
        .is_some());
}

#[test]
fn env_invalid_proxy_fails_closed_and_redacts() {
    let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://user:proxy-env-secret-3@[::1")]);
    let err = env
        .resolve(&https_url("https://example.com/a"))
        .unwrap_err();
    assert!(matches!(err, eggfetch_core::Error::InvalidProxyUrl(_)));
    assert!(!err.to_string().contains("proxy-env-secret-3"));
}

#[test]
fn env_empty_means_direct_and_native_default_unchanged() {
    // No resolver configured -> direct behavior. The empty snapshot models
    // the native default: without `proxy_environment()` the client never
    // consults process environment.
    let env = ProxyEnvironment::new();
    assert!(env.is_empty());
    assert!(env
        .resolve(&https_url("https://example.com/a"))
        .unwrap()
        .is_none());
}

#[test]
fn env_snapshot_stable_after_source_mutation() {
    let mut source = vec![(
        "HTTPS_PROXY".to_owned(),
        "http://first.example:8080".to_owned(),
    )];
    let env = ProxyEnvironment::from_map(source.clone());
    source[0].1 = "http://second.example:8080".to_owned();
    let proxy = env
        .resolve(&https_url("https://example.com/a"))
        .unwrap()
        .unwrap();
    assert_eq!(proxy.uri().host_str(), Some("first.example"));
}

#[test]
fn builder_proxy_environment_opt_in_succeeds() {
    let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://proxy.example:8080")]);
    let _client: Client = Client::builder().proxy_environment(&env).unwrap().build();
}

#[test]
fn builder_proxy_environment_rejects_invalid_closed() {
    let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://user:bogus@[::1")]);
    let Err(err) = Client::builder().proxy_environment(&env) else {
        panic!("expected invalid proxy to fail closed")
    };
    assert!(matches!(err, eggfetch_core::Error::InvalidProxyUrl(_)));
}

#[test]
fn env_debug_never_logs_raw_values() {
    let env = ProxyEnvironment::from_map([(
        "HTTPS_PROXY",
        "http://user:proxy-env-secret-4@proxy.example:8080",
    )]);
    let rendered = format!("{env:?}");
    assert!(!rendered.contains("proxy-env-secret-4"));
    assert!(!rendered.contains("proxy.example"));
}
