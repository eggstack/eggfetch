//! Private connector preparation for the client.

#[cfg(feature = "advanced-routing")]
use std::sync::Arc;

#[cfg(feature = "advanced-routing")]
use super::Dialer;
#[cfg(feature = "advanced-routing")]
use super::{Error, Result};
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
use crate::transport::Connector;

/// Build the standard cleartext connector.
#[cfg(all(
    not(feature = "tls-rustls"),
    any(feature = "transport-http1", feature = "transport-http2")
))]
pub(super) fn build_standard_connector(
    enabler: crate::http_version::HttpVersionPolicyEnabler,
) -> Connector {
    build_standard_connector_with_resolver(
        hyper_util::client::legacy::connect::dns::GaiResolver::new(),
        enabler,
    )
}

/// Build a standard cleartext connector with a caller-selected private
/// resolver. Production callers use the default system resolver; tests use
/// this generic seam to keep resolver provenance deterministic.
#[cfg(all(
    not(feature = "tls-rustls"),
    any(feature = "transport-http1", feature = "transport-http2")
))]
pub(super) fn build_standard_connector_with_resolver<R>(
    resolver: R,
    enabler: crate::http_version::HttpVersionPolicyEnabler,
) -> hyper_util::client::legacy::connect::HttpConnector<
    crate::transport::standard_resolver::ClassifyingResolver<R>,
>
where
    R: tower_service::Service<hyper_util::client::legacy::connect::dns::Name>
        + Clone
        + Send
        + Sync
        + 'static,
    R::Response: Iterator<Item = std::net::SocketAddr>,
    R::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    R::Future: Send + 'static,
{
    let _ = enabler;
    let mut connector = hyper_util::client::legacy::connect::HttpConnector::new_with_resolver(
        crate::transport::standard_resolver::ClassifyingResolver::new(resolver),
    );
    connector.enforce_http(true);
    connector
}

/// Build the standard TLS connector from the authoritative [`TlsConfig`].
#[cfg(all(
    feature = "tls-rustls",
    any(feature = "transport-http1", feature = "transport-http2")
))]
pub(super) fn build_standard_connector(
    config: rustls::ClientConfig,
    enabler: crate::http_version::HttpVersionPolicyEnabler,
) -> Connector {
    build_standard_connector_with_resolver(
        config,
        enabler,
        hyper_util::client::legacy::connect::dns::GaiResolver::new(),
    )
}

/// Build the standard Rustls connector around an explicitly constructed
/// Hyper resolver connector so DNS provenance survives the TLS wrapper.
#[cfg(all(
    feature = "tls-rustls",
    any(feature = "transport-http1", feature = "transport-http2")
))]
pub(super) fn build_standard_connector_with_resolver<R>(
    mut config: rustls::ClientConfig,
    enabler: crate::http_version::HttpVersionPolicyEnabler,
    resolver: R,
) -> hyper_rustls::HttpsConnector<
    hyper_util::client::legacy::connect::HttpConnector<
        crate::transport::standard_resolver::ClassifyingResolver<R>,
    >,
>
where
    R: tower_service::Service<hyper_util::client::legacy::connect::dns::Name>
        + Clone
        + Send
        + Sync
        + 'static,
    R::Response: Iterator<Item = std::net::SocketAddr>,
    R::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    R::Future: Send + 'static,
{
    // hyper-rustls fills ALPN from the selected protocol methods.
    config.alpn_protocols.clear();
    let mut http = hyper_util::client::legacy::connect::HttpConnector::new_with_resolver(
        crate::transport::standard_resolver::ClassifyingResolver::new(resolver),
    );
    http.enforce_http(false);
    let builder = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(config)
        .https_or_http();
    #[cfg(all(feature = "transport-http1", feature = "transport-http2"))]
    {
        match (enabler.enable_http1(), enabler.enable_http2()) {
            (true, true) => builder.enable_http1().enable_http2().wrap_connector(http),
            (true | false, false) => builder.enable_http1().wrap_connector(http),
            (false, true) => builder.enable_http2().wrap_connector(http),
        }
    }
    #[cfg(all(feature = "transport-http1", not(feature = "transport-http2")))]
    {
        let _ = enabler;
        builder.enable_http1().wrap_connector(http)
    }
    #[cfg(all(feature = "transport-http2", not(feature = "transport-http1")))]
    {
        let _ = enabler;
        builder.enable_http2().wrap_connector(http)
    }
}

#[cfg(all(feature = "tls-rustls", feature = "advanced-routing"))]
pub(super) fn build_custom_connector(
    config: rustls::ClientConfig,
    dialer: Arc<dyn Dialer>,
    enabler: crate::http_version::HttpVersionPolicyEnabler,
    sni_hostname: Option<&str>,
) -> Result<crate::transport::CustomConnector> {
    let config = configure_tls_alpn(config, enabler);
    let resolver: Arc<dyn hyper_rustls::ResolveServerName + Send + Sync> =
        if let Some(sni_hostname) = sni_hostname {
            let server_name = rustls::pki_types::ServerName::try_from(sni_hostname.to_owned())
                .map_err(|error| Error::Tls(format!("invalid SNI hostname: {error}")))?;
            Arc::new(hyper_rustls::FixedServerNameResolver::new(server_name))
        } else {
            Arc::new(hyper_rustls::DefaultServerNameResolver::default())
        };
    Ok(hyper_rustls::HttpsConnector::new(
        crate::transport::dialer::DialerConnector::new(dialer),
        Arc::new(config),
        false,
        resolver,
    ))
}

#[cfg(all(not(feature = "tls-rustls"), feature = "advanced-routing"))]
pub(super) fn build_custom_connector(
    _config: (),
    dialer: Arc<dyn Dialer>,
    _enabler: crate::http_version::HttpVersionPolicyEnabler,
    _sni_hostname: Option<&str>,
) -> Result<crate::transport::CustomConnector> {
    Ok(crate::transport::dialer::DialerConnector::new(dialer))
}

/// Apply the selected HTTP protocol policy to a rustls client configuration.
///
/// The standard hyper-rustls connector applies this policy while building its
/// connector. Custom connectors must set the same ALPN list themselves.
#[cfg(feature = "tls-rustls")]
pub(crate) fn configure_tls_alpn(
    mut config: rustls::ClientConfig,
    policy: crate::http_version::HttpVersionPolicyEnabler,
) -> rustls::ClientConfig {
    config.alpn_protocols.clear();
    if policy.enable_http2() {
        config.alpn_protocols.push(b"h2".to_vec());
    }
    if policy.enable_http1() {
        config.alpn_protocols.push(b"http/1.1".to_vec());
    }
    config
}
