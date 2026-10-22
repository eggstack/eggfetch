//! Closure evidence for the core duplicate-helper consolidation (M010).
//!
//! The tests in this file are *source-level* evidence, not behavioral
//! fixtures. Each cluster of the consolidation moved one rule into one
//! shared helper; the tests below count how many places in `eggfetch-core`
//! still spell that rule out, so a future edit that reintroduces a second
//! copy fails here instead of silently drifting.
//!
//! The counts are made over the crate's `src/` tree only. The frozen
//! diagnostic texts live in this file, which is not part of the scanned
//! tree, so a test that pins a text does not look like a second copy of
//! the rule.
//!
//! The file also carries the public-API evidence for cluster D (a
//! non-HTTP(S) scheme never reaches the origin normalization) and cluster G
//! (a non-UTF-8 wire target extension keeps its error kind and message).
#![cfg(feature = "high-level-url")]
#![allow(
    clippy::missing_panics_doc,
    reason = "evidence fixtures fail fast on an impossible source-tree setup; they are not library API"
)]

use eggfetch_core::TransportHints;

/// The `eggfetch-core` source files the duplication counts are made over.
///
/// Every file that could hold a copy of a consolidated rule is listed, so
/// a rule that moved to a file outside this list still fails the counts.
const SOURCE_FILES: &[&str] = &[
    "../src/client.rs",
    "../src/client/config.rs",
    "../src/client/connectors.rs",
    "../src/client/routes.rs",
    "../src/http_origin.rs",
    "../src/network_stream.rs",
    "../src/pipeline/custom_dialing.rs",
    "../src/pipeline/finalize.rs",
    "../src/pipeline/h3_dispatch.rs",
    "../src/pipeline/hyper_dispatch.rs",
    "../src/pipeline/hop.rs",
    "../src/pipeline/lean.rs",
    "../src/pipeline/mod.rs",
    "../src/pipeline/prepare.rs",
    "../src/pipeline/proxy_dispatch.rs",
    "../src/pipeline/redirect.rs",
    "../src/pipeline/retry.rs",
    "../src/pipeline/route.rs",
    "../src/pool.rs",
    "../src/proxy.rs",
    "../src/request.rs",
    "../src/tls.rs",
    "../src/transport/connect.rs",
    "../src/transport/direct_connector.rs",
    "../src/transport/hyper_client.rs",
    "../src/transport/mod.rs",
    "../src/transport/proxy.rs",
    "../src/transport/socks.rs",
    "../src/transport/uds.rs",
    "../src/transport_hints.rs",
];

/// The scanned source text of one file.
fn source(path: &str) -> &'static str {
    match path {
        "../src/client.rs" => include_str!("../src/client.rs"),
        "../src/client/config.rs" => include_str!("../src/client/config.rs"),
        "../src/client/connectors.rs" => include_str!("../src/client/connectors.rs"),
        "../src/client/routes.rs" => include_str!("../src/client/routes.rs"),
        "../src/http_origin.rs" => include_str!("../src/http_origin.rs"),
        "../src/network_stream.rs" => include_str!("../src/network_stream.rs"),
        "../src/pipeline/custom_dialing.rs" => include_str!("../src/pipeline/custom_dialing.rs"),
        "../src/pipeline/finalize.rs" => include_str!("../src/pipeline/finalize.rs"),
        "../src/pipeline/h3_dispatch.rs" => include_str!("../src/pipeline/h3_dispatch.rs"),
        "../src/pipeline/hyper_dispatch.rs" => include_str!("../src/pipeline/hyper_dispatch.rs"),
        "../src/pipeline/hop.rs" => include_str!("../src/pipeline/hop.rs"),
        "../src/pipeline/lean.rs" => include_str!("../src/pipeline/lean.rs"),
        "../src/pipeline/mod.rs" => include_str!("../src/pipeline/mod.rs"),
        "../src/pipeline/prepare.rs" => include_str!("../src/pipeline/prepare.rs"),
        "../src/pipeline/proxy_dispatch.rs" => include_str!("../src/pipeline/proxy_dispatch.rs"),
        "../src/pipeline/redirect.rs" => include_str!("../src/pipeline/redirect.rs"),
        "../src/pipeline/retry.rs" => include_str!("../src/pipeline/retry.rs"),
        "../src/pipeline/route.rs" => include_str!("../src/pipeline/route.rs"),
        "../src/pool.rs" => include_str!("../src/pool.rs"),
        "../src/proxy.rs" => include_str!("../src/proxy.rs"),
        "../src/request.rs" => include_str!("../src/request.rs"),
        "../src/tls.rs" => include_str!("../src/tls.rs"),
        "../src/transport/connect.rs" => include_str!("../src/transport/connect.rs"),
        "../src/transport/direct_connector.rs" => {
            include_str!("../src/transport/direct_connector.rs")
        }
        "../src/transport/hyper_client.rs" => include_str!("../src/transport/hyper_client.rs"),
        "../src/transport/mod.rs" => include_str!("../src/transport/mod.rs"),
        "../src/transport/proxy.rs" => include_str!("../src/transport/proxy.rs"),
        "../src/transport/socks.rs" => include_str!("../src/transport/socks.rs"),
        "../src/transport/uds.rs" => include_str!("../src/transport/uds.rs"),
        "../src/transport_hints.rs" => include_str!("../src/transport_hints.rs"),
        other => panic!("unknown source file {other}"),
    }
}

/// Count how many times `needle` occurs across the scanned source tree.
fn occurrences(needle: &str) -> usize {
    SOURCE_FILES
        .iter()
        .map(|path| count(source(path), needle))
        .sum()
}

/// Count how many times `needle` occurs in `haystack`.
fn count(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Count how many times `needle` occurs in one file.
fn occurrences_in(path: &str, needle: &str) -> usize {
    count(source(path), needle)
}

/// Cluster B: the effective-port rule and its diagnostics live once.
#[test]
fn cluster_b_effective_port_rule_has_one_home() {
    assert_eq!(
        occurrences("all resolved destinations must use the URL's effective port"),
        1,
        "the resolved-destination port diagnostic must live once"
    );
    assert_eq!(
        occurrences("all proxied destinations must use the URL's effective port"),
        1,
        "the proxied-destination port diagnostic must live once"
    );
    assert_eq!(
        occurrences("all resolved proxy peers must use the proxy URL's effective port"),
        1,
        "the proxy-peer port diagnostic must live once"
    );
    assert_eq!(
        occurrences("resolved destinations require an HTTP or HTTPS URL"),
        1,
        "the resolved-destination URL diagnostic must live once"
    );
    assert_eq!(
        occurrences("proxied destinations require an HTTP or HTTPS URL"),
        1,
        "the proxied-destination URL diagnostic must live once"
    );
    // One comparison, one helper.
    assert_eq!(
        occurrences("fn validate_effective_port("),
        1,
        "the effective-port comparison must be implemented once"
    );
    assert_eq!(
        occurrences("fn validate_url_effective_port("),
        1,
        "the URL effective-port helper must be implemented once"
    );
    assert_eq!(
        occurrences("any(|address| address.port() != expected_port)"),
        1,
        "the port comparison must be written once"
    );
}

/// Cluster C: the custom-dialing ladder lives once, with five arms.
#[test]
fn cluster_c_custom_dialing_ladder_has_one_home() {
    assert_eq!(
        occurrences("fn custom_dialing_conflict("),
        1,
        "the ladder must be implemented once"
    );
    for message in [
        "custom dialing is incompatible with caller-supplied resolved destinations",
        "custom dialing is incompatible with local-address or socket-option routing",
        "custom dialing is incompatible with Unix-domain routing",
        "custom dialing is incompatible with HTTP/3",
        "custom dialing is incompatible with built-in proxy routing",
    ] {
        assert_eq!(
            occurrences(message),
            1,
            "the ladder arm `{message}` must live once"
        );
    }
}

/// Cluster D: the origin normalization lives once.
#[test]
fn cluster_d_origin_normalization_has_one_home() {
    assert_eq!(
        occurrences_in("../src/pool.rs", "fn from_url_with_proxy_scheme("),
        1,
        "the proxied origin key must be built once"
    );
    // The pool origin key is built through the shared origin normalization.
    assert_eq!(
        occurrences_in(
            "../src/pool.rs",
            "crate::http_origin::HttpOrigin::from_url(url)?"
        ),
        1,
        "the pool origin key must go through the shared normalization"
    );
}

/// Cluster E: the post-handshake ALPN detection lives once.
#[test]
fn cluster_e_alpn_detection_has_one_home() {
    assert_eq!(
        occurrences("fn negotiated_http2("),
        1,
        "the ALPN detection helper must be implemented once"
    );
    // The four transports that used to repeat the inline comparison all call
    // the shared helper, and each keeps inspecting its own ALPN payload.
    assert_eq!(
        occurrences("super::negotiated_http2("),
        4,
        "the four transports must all use the shared helper"
    );
    assert_eq!(
        occurrences("alpn_protocol()"),
        5,
        "the four transports must keep inspecting their own ALPN payload"
    );
    assert_eq!(
        occurrences("alpn.is_some_and(|protocol| protocol == ALPN_HTTP2)"),
        1,
        "the negotiated-protocol comparison must be written once"
    );
}

/// Cluster F: one `HyperClientPolicy` constructor.
#[test]
fn cluster_f_policy_has_one_constructor() {
    assert_eq!(
        occurrences_in("../src/transport/hyper_client.rs", "fn persistent("),
        0,
        "the persistent constructor must be gone"
    );
    assert_eq!(
        occurrences_in("../src/transport/hyper_client.rs", "fn cached_route("),
        0,
        "the cached-route constructor must be gone"
    );
    assert_eq!(
        occurrences_in(
            "../src/transport/hyper_client.rs",
            "pub(crate) fn new(\n        retry_canceled_requests: bool,"
        ),
        1,
        "the shared policy constructor must exist exactly once"
    );
}

/// Cluster G: the wire-target UTF-8 diagnostic lives once.
#[test]
fn cluster_g_target_utf8_diagnostic_has_one_home() {
    assert_eq!(
        occurrences("target extension is not valid UTF-8"),
        1,
        "the wire-target UTF-8 diagnostic must live once"
    );
    assert_eq!(
        occurrences("fn target_extension_str("),
        1,
        "the wire-target decode must be implemented once"
    );
}

/// Cluster A: one builder for the cached direct route clients.
#[test]
fn cluster_a_route_client_builder_has_one_home() {
    assert_eq!(
        occurrences_in("../src/client/routes.rs", "fn build_resolved_client("),
        0,
        "the resolved-route builder must be gone"
    );
    assert_eq!(
        occurrences_in("../src/client/routes.rs", "fn build_direct_client("),
        1,
        "the shared route-client builder must exist exactly once"
    );
    // One definition plus the two cached route families that use it.
    assert_eq!(
        occurrences_in("../src/client/routes.rs", "build_direct_client("),
        3,
        "the resolved-target and SNI routes must both use the shared builder"
    );
}

/// The public surface must not grow: every symbol the consolidation added
/// is crate-private.
#[test]
fn public_surface_is_unchanged() {
    for (path, needle) in [
        ("../src/transport_hints.rs", "pub fn target_extension_str"),
        (
            "../src/transport_hints.rs",
            "pub const TARGET_NOT_UTF8_MESSAGE",
        ),
        ("../src/transport/mod.rs", "pub fn negotiated_http2"),
        ("../src/transport/mod.rs", "pub const ALPN_HTTP2"),
        ("../src/http_origin.rs", "pub fn validate_effective_port"),
        (
            "../src/http_origin.rs",
            "pub fn validate_url_effective_port",
        ),
        ("../src/http_origin.rs", "pub enum EffectivePortSubject"),
        (
            "../src/pipeline/custom_dialing.rs",
            "pub struct CustomDialingState",
        ),
        (
            "../src/pipeline/custom_dialing.rs",
            "pub enum CustomDialingConflict",
        ),
        ("../src/client/routes.rs", "pub enum DirectRouteBinding"),
    ] {
        assert_eq!(
            occurrences_in(path, needle),
            0,
            "`{needle}` must not be part of the public surface"
        );
    }
    // The crate-private forms do exist.
    assert_eq!(
        occurrences_in(
            "../src/transport_hints.rs",
            "pub(crate) fn target_extension_str"
        ),
        1
    );
    assert_eq!(
        occurrences_in("../src/transport/mod.rs", "pub(crate) fn negotiated_http2"),
        1
    );
    assert_eq!(
        occurrences_in(
            "../src/http_origin.rs",
            "pub(crate) fn validate_effective_port"
        ),
        1
    );
    assert_eq!(
        occurrences_in(
            "../src/http_origin.rs",
            "pub(crate) enum EffectivePortSubject"
        ),
        1
    );
}

/// Cluster D: a non-HTTP(S) scheme never reaches the origin normalization.
///
/// The high-level client rejects such a URL at the client boundary, so the
/// stricter origin rule cannot change any observable behavior.
#[test]
fn cluster_d_non_http_scheme_is_rejected_before_the_origin_normalization() {
    let Err(error) = eggfetch_core::Client::new().get("ftp://example.com/path") else {
        panic!("a non-HTTP(S) scheme must be rejected");
    };
    assert_eq!(error.kind(), "unsupported");
    assert!(error.to_string().contains("not supported"), "got {error:?}");
}

/// Cluster G: a non-UTF-8 wire target extension keeps its error kind and
/// message through the public request path.
#[tokio::test]
async fn cluster_g_non_utf8_target_keeps_its_error_kind_and_message() {
    let hints = TransportHints {
        target: Some(bytes::Bytes::from_static(&[0xff, 0xfe])),
        ..Default::default()
    };
    let error = eggfetch_core::Client::new()
        .get("http://example.com/path")
        .expect("the request builds")
        .transport_hints(hints)
        .send()
        .await
        .expect_err("a non-UTF-8 target extension must be rejected");
    assert_eq!(error.kind(), "invalid_url");
    assert_eq!(
        error.to_string(),
        "invalid URL: target extension is not valid UTF-8",
        "the diagnostic must stay byte-identical"
    );
}
