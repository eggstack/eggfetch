//! TLS configuration for the eggfetch client.
//!
//! Provides [`TlsConfig`] and [`TlsConfigBuilder`] for controlling TLS
//! behavior: trust stores, custom CA bundles, client certificates,
//! verification policy, TLS version bounds, and SNI.
//!
//! # Defaults
//!
//! - Hostname verification is enabled.
//! - Certificate-chain verification is enabled.
//! - Native system roots are preferred; packaged `WebPKI` roots are used
//!   as a construction fallback when native roots are unavailable.
//! - TLS 1.2 and 1.3 are supported.
//! - SNI is enabled by default.
//!
//! # Custom CA bundles
//!
//! A custom CA bundle replaces the default root store (it does not
//! augment). The bundle is loaded at construction time; malformed or
//! empty bundles are rejected immediately.
//!
//! # Safety
//!
//! Disabling certificate verification (`verify = false`) weakens TLS
//! guarantees. It should only be used for testing or against known
//! trusted targets. The Python binding documents this in the `verify`
//! parameter docstring.

use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::error::{Error, Result};

/// Trust store source for certificate verification.
#[derive(Debug, Clone, Default)]
pub enum TrustStore {
    /// Use native system roots with packaged `WebPKI` roots as a
    /// construction fallback (default).
    #[default]
    NativeWithWebPkiFallback,
    /// Use only native system roots. Fails at construction if the
    /// platform root store is unavailable.
    NativeOnly,
    /// Use only packaged Mozilla/WebPKI roots. Does not consult the
    /// operating system trust store.
    WebPkiOnly,
    /// Use the provided CA certificates as the sole trust anchor.
    /// These replace all other roots.
    Custom(Vec<CertificateDer<'static>>),
}

/// A client certificate and private key for mTLS.
#[derive(Clone)]
pub enum ClientIdentity {
    /// A certificate chain and private key in DER format.
    Pem {
        /// The certificate chain, in DER bytes.
        cert_chain: Vec<CertificateDer<'static>>,
        /// The private key, as raw DER bytes.
        private_key_der: Vec<u8>,
        /// The label from the PEM block (e.g., "PRIVATE KEY", "RSA PRIVATE KEY").
        key_label: String,
    },
}

impl std::fmt::Debug for ClientIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pem {
                cert_chain,
                key_label,
                ..
            } => f
                .debug_struct("ClientIdentity::Pem")
                .field("cert_count", &cert_chain.len())
                .field("key_label", key_label)
                .finish_non_exhaustive(),
        }
    }
}

/// Minimum or maximum TLS protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TlsVersion {
    /// TLS 1.2.
    Tls12,
    /// TLS 1.3.
    Tls13,
}

impl TlsVersion {
    fn to_rustls(self) -> &'static rustls::SupportedProtocolVersion {
        match self {
            Self::Tls12 => &rustls::version::TLS12,
            Self::Tls13 => &rustls::version::TLS13,
        }
    }
}

/// Immutable shared TLS configuration for a client.
///
/// Construct via [`TlsConfig::builder()`] or [`TlsConfig::builder_with_defaults()`].
#[derive(Clone)]
pub struct TlsConfig {
    trust_store: TrustStore,
    custom_ca_roots: Vec<CertificateDer<'static>>,
    client_identity: Option<ClientIdentity>,
    verify_hostname: bool,
    verify_certificate: bool,
    min_version: Option<TlsVersion>,
    max_version: Option<TlsVersion>,
    sni_enabled: bool,
    root_store: Arc<OnceLock<Result<rustls::RootCertStore>>>,
}

impl std::fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConfig")
            .field("trust_store", &self.trust_store)
            .field("has_custom_ca", &!self.custom_ca_roots.is_empty())
            .field("has_client_identity", &self.client_identity.is_some())
            .field("verify_hostname", &self.verify_hostname)
            .field("verify_certificate", &self.verify_certificate)
            .field("min_version", &self.min_version)
            .field("max_version", &self.max_version)
            .field("sni_enabled", &self.sni_enabled)
            .finish_non_exhaustive()
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TlsConfig {
    /// Create a builder with secure defaults (verification enabled, native
    /// roots preferred, SNI enabled).
    #[must_use]
    pub fn builder() -> TlsConfigBuilder {
        TlsConfigBuilder::new()
    }

    /// Create a builder from the default configuration.
    ///
    /// This is identical to [`TlsConfig::builder()`] and exists for
    /// discoverability.
    #[must_use]
    pub fn builder_with_defaults() -> TlsConfigBuilder {
        Self::builder()
    }

    /// Enable or disable certificate and hostname verification while
    /// retaining the rest of this configuration.
    #[must_use]
    pub fn danger_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.verify_certificate = !accept;
        self.verify_hostname = !accept;
        self
    }

    /// Build a `tokio_rustls::TlsConnector` from this TLS configuration.
    ///
    /// This is the convenience entry point for [`UpgradedStream::start_tls`]
    /// and other contexts where a generic TLS upgrade must reflect the
    /// full configured policy (custom CA, verification, SNI, version
    /// bounds, client identity).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying rustls configuration cannot be
    /// built (see [`Self::build_rustls_config`]).
    pub fn tls_connector(&self) -> Result<tokio_rustls::TlsConnector> {
        let config = self.build_rustls_config()?;
        Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
    }

    /// Build a `rustls::ClientConfig` from this TLS configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store, client certificates, or
    /// protocol versions cannot be configured.
    pub fn build_rustls_config(&self) -> Result<rustls::ClientConfig> {
        let root_store = self.build_root_store()?;

        let mut protocol_versions = Vec::new();
        match (self.min_version, self.max_version) {
            (Some(min), Some(max)) => {
                if min > max {
                    return Err(Error::TlsConfig(format!(
                        "invalid TLS version range: min ({min:?}) > max ({max:?})"
                    )));
                }
                for v in Self::supported_versions() {
                    if v >= min && v <= max {
                        protocol_versions.push(v.to_rustls());
                    }
                }
            }
            (Some(min), None) => {
                for v in Self::supported_versions() {
                    if v >= min {
                        protocol_versions.push(v.to_rustls());
                    }
                }
            }
            (None, Some(max)) => {
                for v in Self::supported_versions() {
                    if v <= max {
                        protocol_versions.push(v.to_rustls());
                    }
                }
            }
            (None, None) => {
                for v in Self::supported_versions() {
                    protocol_versions.push(v.to_rustls());
                }
            }
        }

        if protocol_versions.is_empty() {
            return Err(Error::TlsConfig(
                "no supported TLS versions in configured range".into(),
            ));
        }

        let provider = process_crypto_provider()?;
        let supported_schemes = provider
            .signature_verification_algorithms
            .supported_schemes();
        let mut config = if self.verify_certificate && self.verify_hostname {
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_protocol_versions(&protocol_versions)
                .map_err(|e| Error::TlsConfig(format!("TLS version config: {e}")))?
                .with_root_certificates(root_store)
                .with_no_client_auth()
        } else if self.verify_certificate {
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_protocol_versions(&protocol_versions)
                .map_err(|e| Error::TlsConfig(format!("TLS version config: {e}")))?
                .dangerous()
                .with_custom_certificate_verifier(no_hostname_verifier(&root_store, provider)?)
                .with_no_client_auth()
        } else {
            rustls::ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&protocol_versions)
                .map_err(|e| Error::TlsConfig(format!("TLS version config: {e}")))?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier { supported_schemes }))
                .with_no_client_auth()
        };

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        config.enable_sni = self.sni_enabled;

        if let Some(identity) = &self.client_identity {
            config.client_auth_cert_resolver = Arc::new(SingleCertResolver::new(identity.clone()));
        }

        Ok(config)
    }

    /// Build the root certificate store from the configured trust policy.
    fn build_root_store(&self) -> Result<rustls::RootCertStore> {
        self.root_store
            .get_or_init(|| self.build_root_store_uncached())
            .clone()
    }

    fn build_root_store_uncached(&self) -> Result<rustls::RootCertStore> {
        match &self.trust_store {
            TrustStore::NativeWithWebPkiFallback => match Self::try_native_roots() {
                Ok(store) if !store.is_empty() => Ok(store),
                _ => Ok(Self::webpki_roots()),
            },
            TrustStore::NativeOnly => Self::try_native_roots(),
            TrustStore::WebPkiOnly => Ok(Self::webpki_roots()),
            TrustStore::Custom(certs) => {
                if certs.is_empty() {
                    return Err(Error::CaBundle("custom CA bundle is empty".into()));
                }
                let mut store = rustls::RootCertStore::empty();
                for cert in certs {
                    store
                        .add(cert.clone())
                        .map_err(|e| Error::CaBundle(format!("invalid CA certificate: {e}")))?;
                }
                Ok(store)
            }
        }
    }

    fn try_native_roots() -> Result<rustls::RootCertStore> {
        let mut roots = rustls::RootCertStore::empty();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            #[cfg(target_os = "linux")]
            let pem_paths: &[&str] = &[
                "/etc/ssl/certs/ca-certificates.crt",
                "/etc/pki/tls/certs/ca-bundle.crt",
                "/etc/ssl/ca-bundle.pem",
            ];

            #[cfg(target_os = "macos")]
            let pem_paths: &[&str] = &["/etc/ssl/cert.pem", "/usr/local/etc/openssl/cert.pem"];

            for path in pem_paths {
                if let Ok(certs) = load_pem_certs_from_path(Path::new(path)) {
                    for cert in certs {
                        let _ = roots.add(cert);
                    }
                }
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            for cert in rustls_native_certs::load_native_certs().certs {
                let _ = roots.add(cert);
            }
        }

        if roots.is_empty() {
            return Err(Error::TlsConfig(
                "native root certificates not available".into(),
            ));
        }
        Ok(roots)
    }

    fn webpki_roots() -> rustls::RootCertStore {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        roots
    }

    fn supported_versions() -> impl Iterator<Item = TlsVersion> {
        [TlsVersion::Tls12, TlsVersion::Tls13].into_iter()
    }

    /// Returns `true` if hostname verification is enabled.
    #[must_use]
    pub fn verify_hostname(&self) -> bool {
        self.verify_hostname
    }

    /// Returns `true` if certificate verification is enabled.
    #[must_use]
    pub fn verify_certificate(&self) -> bool {
        self.verify_certificate
    }

    /// Returns `true` if SNI is enabled.
    #[must_use]
    pub fn sni_enabled(&self) -> bool {
        self.sni_enabled
    }

    /// Build a `rustls::ClientConfig` for QUIC (TLS 1.3 only, ALPN `h3`).
    ///
    /// This differs from [`build_rustls_config`](Self::build_rustls_config)
    /// by restricting to TLS 1.3 and using the `h3` ALPN.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store, client certificates, or
    /// protocol versions cannot be configured.
    #[allow(dead_code)]
    pub(crate) fn build_quic_rustls_config(&self) -> Result<rustls::ClientConfig> {
        let root_store = self.build_root_store()?;

        let provider = process_crypto_provider()?;
        let supported_schemes = provider
            .signature_verification_algorithms
            .supported_schemes();
        let mut config = if self.verify_certificate && self.verify_hostname {
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
                .with_root_certificates(root_store)
                .with_no_client_auth()
        } else if self.verify_certificate {
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
                .dangerous()
                .with_custom_certificate_verifier(no_hostname_verifier(&root_store, provider)?)
                .with_no_client_auth()
        } else {
            rustls::ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier { supported_schemes }))
                .with_no_client_auth()
        };

        config.alpn_protocols = vec![b"h3".to_vec()];
        config.enable_sni = self.sni_enabled;

        if let Some(identity) = &self.client_identity {
            config.client_auth_cert_resolver = Arc::new(SingleCertResolver::new(identity.clone()));
        }

        Ok(config)
    }
}

/// Builder for constructing a [`TlsConfig`].
pub struct TlsConfigBuilder {
    trust_store: TrustStore,
    custom_ca_roots: Vec<CertificateDer<'static>>,
    client_identity: Option<ClientIdentity>,
    verify_hostname: bool,
    verify_certificate: bool,
    min_version: Option<TlsVersion>,
    max_version: Option<TlsVersion>,
    sni_enabled: bool,
}

impl Default for TlsConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TlsConfigBuilder {
    /// Create a new builder with secure defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trust_store: TrustStore::NativeWithWebPkiFallback,
            custom_ca_roots: Vec::new(),
            client_identity: None,
            verify_hostname: true,
            verify_certificate: true,
            min_version: None,
            max_version: None,
            sni_enabled: true,
        }
    }

    /// Set the trust store source.
    #[must_use]
    pub fn trust_store(mut self, store: TrustStore) -> Self {
        self.trust_store = store;
        self
    }

    /// Load CA certificates from a PEM file, replacing any existing trust
    /// store configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the PEM data is
    /// malformed.
    pub fn ca_certificate_path(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let certs = load_pem_certs_from_path(path.as_ref())?;
        if certs.is_empty() {
            if path.as_ref().is_dir() {
                return Ok(self);
            }
            return Err(Error::CaBundle(format!(
                "no certificates found in {}",
                path.as_ref().display()
            )));
        }
        self.custom_ca_roots = certs;
        self.trust_store = TrustStore::Custom(self.custom_ca_roots.clone());
        Ok(self)
    }

    /// Add CA certificates from a PEM file or certificate directory to the
    /// configured custom trust store, replacing the default trust store if
    /// this is the first custom source.
    ///
    /// Duplicate certificates are ignored. This is useful when multiple
    /// environment-provided CA sources are combined.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be read or contains no PEM
    /// certificates. Empty directories are ignored.
    pub fn add_ca_certificate_path(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let certs = load_pem_certs_from_path(path.as_ref())?;
        if certs.is_empty() {
            if path.as_ref().is_dir() {
                return Ok(self);
            }
            return Err(Error::CaBundle(format!(
                "no certificates found in {}",
                path.as_ref().display()
            )));
        }
        if !matches!(&self.trust_store, TrustStore::Custom(_)) {
            self.custom_ca_roots.clear();
        }
        for cert in certs {
            if !self
                .custom_ca_roots
                .iter()
                .any(|existing| existing.as_ref() == cert.as_ref())
            {
                self.custom_ca_roots.push(cert);
            }
        }
        self.trust_store = TrustStore::Custom(self.custom_ca_roots.clone());
        Ok(self)
    }

    /// Load CA certificates from PEM bytes, replacing any existing trust
    /// store configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the PEM data is malformed or empty.
    pub fn ca_certificate_pem(mut self, pem_bytes: &[u8]) -> Result<Self> {
        let certs = parse_pem_certificates(pem_bytes)?;
        if certs.is_empty() {
            return Err(Error::CaBundle("no certificates found in PEM data".into()));
        }
        self.custom_ca_roots = certs;
        self.trust_store = TrustStore::Custom(self.custom_ca_roots.clone());
        Ok(self)
    }

    /// Load CA certificates from DER bytes, replacing any existing trust
    /// store configuration.
    ///
    /// Each certificate is expected as raw DER-encoded X.509 data
    /// (without PEM framing).
    ///
    /// # Errors
    ///
    /// Returns an error if no certificates are provided.
    pub fn ca_certificate_der(mut self, der_certs: Vec<Vec<u8>>) -> Result<Self> {
        let certs: Vec<CertificateDer<'static>> =
            der_certs.into_iter().map(CertificateDer::from).collect();
        if certs.is_empty() {
            return Err(Error::CaBundle("no DER certificates provided".into()));
        }
        self.custom_ca_roots = certs;
        self.trust_store = TrustStore::Custom(self.custom_ca_roots.clone());
        Ok(self)
    }

    /// Set the client identity for mTLS.
    #[must_use]
    pub fn client_identity(mut self, identity: ClientIdentity) -> Self {
        self.client_identity = Some(identity);
        self
    }

    /// Load a client certificate and private key from PEM files.
    ///
    /// # Errors
    ///
    /// Returns an error if the files cannot be read or the PEM data is
    /// malformed.
    pub fn client_cert_path(
        self,
        cert_path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let certs = load_pem_certs_from_path(cert_path.as_ref())?;
        if certs.is_empty() {
            return Err(Error::ClientCert(format!(
                "no certificates found in {}",
                cert_path.as_ref().display()
            )));
        }

        let key_bytes = std::fs::read(key_path.as_ref()).map_err(|e| {
            Error::PrivateKey(format!(
                "failed to read private key {}: {e}",
                key_path.as_ref().display()
            ))
        })?;
        let (key_data, key_label) = parse_private_key(&key_bytes)?;

        Ok(self.client_identity(ClientIdentity::Pem {
            cert_chain: certs,
            private_key_der: key_data,
            key_label,
        }))
    }

    /// Enable or disable hostname verification.
    ///
    /// When disabled, the client does not verify that the server's
    /// certificate matches the requested hostname. This is insecure
    /// and should only be used for testing.
    #[must_use]
    pub fn verify_hostname(mut self, enabled: bool) -> Self {
        self.verify_hostname = enabled;
        self
    }

    /// Enable or disable certificate verification.
    ///
    /// When disabled (`danger_accept_invalid_certs(true)`), the client
    /// does not verify the server's certificate chain or hostname.
    /// This is **insecure** and should only be used for testing against
    /// known self-signed certificates.
    #[must_use]
    pub fn danger_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.verify_certificate = !accept;
        self.verify_hostname = !accept;
        self
    }

    /// Set the minimum TLS version.
    #[must_use]
    pub fn min_tls_version(mut self, version: TlsVersion) -> Self {
        self.min_version = Some(version);
        self
    }

    /// Set the maximum TLS version.
    #[must_use]
    pub fn max_tls_version(mut self, version: TlsVersion) -> Self {
        self.max_version = Some(version);
        self
    }

    /// Enable or disable SNI (Server Name Indication).
    ///
    /// Disabling SNI can break hostname verification and virtual-host
    /// routing. It should rarely be necessary.
    #[must_use]
    pub fn sni(mut self, enabled: bool) -> Self {
        self.sni_enabled = enabled;
        self
    }

    /// Build the [`TlsConfig`].
    #[must_use]
    pub fn build(self) -> TlsConfig {
        TlsConfig {
            trust_store: self.trust_store,
            custom_ca_roots: self.custom_ca_roots,
            client_identity: self.client_identity,
            verify_hostname: self.verify_hostname,
            verify_certificate: self.verify_certificate,
            min_version: self.min_version,
            max_version: self.max_version,
            sni_enabled: self.sni_enabled,
            root_store: Arc::new(OnceLock::new()),
        }
    }
}

/// A certificate verifier that accepts all certificates (unsafe).
#[derive(Debug)]
struct NoVerifier {
    supported_schemes: Vec<rustls::SignatureScheme>,
}

pub(crate) fn process_crypto_provider() -> Result<Arc<rustls::crypto::CryptoProvider>> {
    if let Some(provider) = rustls::crypto::CryptoProvider::get_default() {
        return Ok(provider.clone());
    }

    // Ask rustls to install the provider selected by its crate features, then
    // use that same process-level provider for all custom verifiers.
    let _ = rustls::ClientConfig::builder();
    rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .ok_or_else(|| Error::TlsConfig("no default TLS crypto provider".into()))
}

/// Verify a certificate chain while intentionally skipping hostname matching.
///
/// This preserves certificate and signature validation for callers that need
/// to use a trusted certificate whose name is not the request hostname.
#[derive(Debug)]
struct NoHostnameVerifier {
    roots: Arc<rustls::RootCertStore>,
    delegate: Arc<rustls::client::WebPkiServerVerifier>,
    supported_algs: rustls::crypto::WebPkiSupportedAlgorithms,
}

fn no_hostname_verifier(
    roots: &rustls::RootCertStore,
    provider: Arc<rustls::crypto::CryptoProvider>,
) -> Result<Arc<dyn rustls::client::danger::ServerCertVerifier>> {
    let roots = Arc::new(roots.clone());
    let supported_algs = provider.signature_verification_algorithms;
    let delegate =
        rustls::client::WebPkiServerVerifier::builder_with_provider(roots.clone(), provider)
            .build()
            .map_err(|e| Error::TlsConfig(format!("failed to build certificate verifier: {e}")))?;
    Ok(Arc::new(NoHostnameVerifier {
        roots,
        delegate,
        supported_algs,
    }))
}

impl rustls::client::danger::ServerCertVerifier for NoHostnameVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let cert = rustls::server::ParsedCertificate::try_from(end_entity)?;
        rustls::client::verify_server_cert_signed_by_trust_anchor(
            &cert,
            &self.roots,
            intermediates,
            now,
            self.supported_algs.all,
        )?;
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.delegate.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.delegate.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.delegate.supported_verify_schemes()
    }
}

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.supported_schemes.clone()
    }
}

/// A resolver that returns a single certificate and key for mTLS.
struct SingleCertResolver {
    cert_chain: Vec<CertificateDer<'static>>,
    private_key_der: Vec<u8>,
    key_label: String,
}

impl std::fmt::Debug for SingleCertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleCertResolver")
            .field("cert_count", &self.cert_chain.len())
            .field("key_label", &self.key_label)
            .finish_non_exhaustive()
    }
}

impl SingleCertResolver {
    fn new(identity: ClientIdentity) -> Self {
        match identity {
            ClientIdentity::Pem {
                cert_chain,
                private_key_der,
                key_label,
            } => Self {
                cert_chain,
                private_key_der,
                key_label,
            },
        }
    }
}

impl rustls::client::ResolvesClientCert for SingleCertResolver {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[rustls::SignatureScheme],
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let key_der = match self.key_label.as_str() {
            "RSA PRIVATE KEY" => PrivateKeyDer::Pkcs1(rustls::pki_types::PrivatePkcs1KeyDer::from(
                self.private_key_der.clone(),
            )),
            "EC PRIVATE KEY" => PrivateKeyDer::Sec1(rustls::pki_types::PrivateSec1KeyDer::from(
                self.private_key_der.clone(),
            )),
            _ => PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                self.private_key_der.clone(),
            )),
        };
        let signing_key = rustls::crypto::ring::sign::any_supported_type(&key_der).ok()?;
        Some(Arc::new(rustls::sign::CertifiedKey::new(
            self.cert_chain.clone(),
            signing_key,
        )))
    }

    fn has_certs(&self) -> bool {
        !self.cert_chain.is_empty()
    }
}

// PEM parsing utilities.

/// Byte sequence opening a PEM section (`-----BEGIN `).
const PEM_BEGIN: &[u8] = b"-----BEGIN ";
/// Byte sequence closing a PEM section (`-----END `).
const PEM_END: &[u8] = b"-----END ";

/// Load PEM-encoded certificates from a file.
fn load_pem_certs_from_path(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .map_err(|e| Error::CaBundle(format!("failed to read {}: {e}", path.display())))?
            .filter_map(std::result::Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);

        let mut certs = Vec::new();
        for entry in entries {
            if !entry.metadata().is_ok_and(|metadata| metadata.is_file()) {
                continue;
            }
            if let Ok(data) = std::fs::read(entry.path()) {
                certs.extend(parse_pem_certificates(&data)?);
            }
        }
        return Ok(certs);
    }

    let data = std::fs::read(path)
        .map_err(|e| Error::CaBundle(format!("failed to read {}: {e}", path.display())))?;
    parse_pem_certificates(&data)
}

/// Parse PEM-encoded certificate data into DER certificates.
///
/// Iterates over every `-----BEGIN`/`-----END` section in the input.
/// Non-PEM content between sections is skipped. Malformed or unterminated
/// PEM sections return an error instead of silently producing a partial trust
/// store. Sections other than `CERTIFICATE` (keys, etc.) are ignored.
fn parse_pem_certificates(pem_bytes: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let mut certs = Vec::new();
    let mut rest = pem_bytes;

    while let Some(pos) = find_pem_start(rest) {
        // Feed `decode_vec` exactly one section: it rejects input with
        // trailing content after the END marker, so handing it the whole
        // remaining bundle would fail on multi-certificate files.
        let section = &rest[pos..];
        let section_len = pem_section_len(section)
            .ok_or_else(|| Error::CaBundle("unterminated PEM block in certificate data".into()))?;
        let (label, data) = pem_rfc7468::decode_vec(&section[..section_len])
            .map_err(|e| Error::CaBundle(format!("invalid PEM block in certificate data: {e}")))?;
        if label == "CERTIFICATE" {
            certs.push(CertificateDer::from(data));
        }
        rest = &section[section_len..];
    }

    Ok(certs)
}

/// Find the offset of the next `-----BEGIN ` marker in the input.
fn find_pem_start(data: &[u8]) -> Option<usize> {
    if data.len() < PEM_BEGIN.len() {
        return None;
    }
    data.windows(PEM_BEGIN.len()).position(|w| w == PEM_BEGIN)
}

/// Length of the PEM section starting at the beginning of `data` (which
/// must start with a BEGIN marker): from that marker through the end of
/// the first following END-marker line.
///
/// Returns `None` when no END marker exists (truncated block). The base64
/// body cannot contain the `-----END ` byte sequence, so the first match
/// after BEGIN closes the section.
fn pem_section_len(data: &[u8]) -> Option<usize> {
    let after_begin = &data[PEM_BEGIN.len()..];
    let end_marker = after_begin
        .windows(PEM_END.len())
        .position(|w| w == PEM_END)?
        + PEM_BEGIN.len();
    let line_end = data[end_marker..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(data.len(), |p| end_marker + p + 1);
    Some(line_end)
}

/// Parse a PEM-encoded private key, returning raw DER bytes and the PEM label.
///
/// Like [`parse_pem_certificates`], skips non-PEM sections between valid
/// blocks but rejects malformed or unterminated PEM sections.
fn parse_private_key(key_bytes: &[u8]) -> Result<(Vec<u8>, String)> {
    let mut rest = key_bytes;

    while let Some(pos) = find_pem_start(rest) {
        let section = &rest[pos..];
        let section_len = pem_section_len(section).ok_or_else(|| {
            Error::PrivateKey("unterminated PEM block in private key data".into())
        })?;
        let (label, data) = pem_rfc7468::decode_vec(&section[..section_len]).map_err(|e| {
            Error::PrivateKey(format!("invalid PEM block in private key data: {e}"))
        })?;
        if matches!(label, "PRIVATE KEY" | "RSA PRIVATE KEY" | "EC PRIVATE KEY") {
            return Ok((data, label.to_string()));
        }
        rest = &section[section_len..];
    }

    Err(Error::PrivateKey("no private key found in PEM data".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn default_tls_config() {
        let config = TlsConfig::default();
        assert!(config.verify_hostname());
        assert!(config.verify_certificate());
        assert!(config.sni_enabled());
    }

    #[test]
    fn builder_defaults() {
        let config = TlsConfig::builder().build();
        assert!(config.verify_hostname());
        assert!(config.verify_certificate());
        assert!(config.sni_enabled());
    }

    #[test]
    fn builder_disable_verification() {
        let config = TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .build();
        assert!(!config.verify_hostname());
        assert!(!config.verify_certificate());
    }

    #[test]
    fn config_disable_verification_preserves_other_settings() {
        let config = TlsConfig::builder()
            .sni(false)
            .min_tls_version(TlsVersion::Tls13)
            .build()
            .danger_accept_invalid_certs(true);
        assert!(!config.verify_hostname());
        assert!(!config.verify_certificate());
        assert!(!config.sni_enabled());
        assert_eq!(config.min_version, Some(TlsVersion::Tls13));
    }

    #[test]
    fn builder_custom_tls_version() {
        let config = TlsConfig::builder()
            .min_tls_version(TlsVersion::Tls13)
            .build();
        assert_eq!(config.min_version, Some(TlsVersion::Tls13));
    }

    #[test]
    fn builder_invalid_version_range() {
        let config = TlsConfig::builder()
            .min_tls_version(TlsVersion::Tls13)
            .max_tls_version(TlsVersion::Tls12)
            .build();
        let err = config.build_rustls_config().unwrap_err();
        assert_eq!(err.kind(), "tls_config");
    }

    #[test]
    fn builder_sni_disabled() {
        let config = TlsConfig::builder().sni(false).build();
        assert!(!config.sni_enabled());
    }

    #[test]
    fn trust_store_default() {
        let config = TlsConfig::builder().build();
        assert!(matches!(
            config.trust_store,
            TrustStore::NativeWithWebPkiFallback
        ));
    }

    #[test]
    fn trust_store_custom_empty_rejected() {
        let result = TlsConfig::builder()
            .ca_certificate_pem(b"")
            .map(super::TlsConfigBuilder::build);
        assert!(result.is_err());
    }

    #[test]
    fn add_ca_certificate_path_combines_files_and_directories() {
        let make_ca_pem = || {
            let mut params = rcgen::CertificateParams::default();
            params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
            let key = rcgen::KeyPair::generate().unwrap();
            params.self_signed(&key).unwrap().pem()
        };
        let first = make_ca_pem();
        let second = make_ca_pem();
        let tempdir = tempfile::tempdir().unwrap();
        let first_path = tempdir.path().join("first.pem");
        let cert_dir = tempdir.path().join("certs");
        std::fs::create_dir(&cert_dir).unwrap();
        std::fs::write(&first_path, first).unwrap();
        std::fs::write(cert_dir.join("second.pem"), second).unwrap();

        let config = TlsConfig::builder()
            .ca_certificate_path(first_path)
            .unwrap()
            .add_ca_certificate_path(cert_dir)
            .unwrap()
            .build();
        assert_eq!(config.custom_ca_roots.len(), 2);

        let empty_dir = tempdir.path().join("empty");
        std::fs::create_dir(&empty_dir).unwrap();
        let config = TlsConfig::builder()
            .ca_certificate_path(tempdir.path().join("first.pem"))
            .unwrap()
            .add_ca_certificate_path(empty_dir)
            .unwrap()
            .build();
        assert_eq!(config.custom_ca_roots.len(), 1);
    }

    #[test]
    fn tls_version_ordering() {
        assert!(TlsVersion::Tls12 < TlsVersion::Tls13);
    }

    #[test]
    fn build_rustls_config_verification() {
        let config = TlsConfig::builder().build();
        let result = config.build_rustls_config();
        assert!(result.is_ok());
    }

    #[test]
    fn build_rustls_config_no_verify() {
        let config = TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .sni(false)
            .build();
        let rustls_config = config.build_rustls_config().unwrap();
        assert!(!rustls_config.enable_sni);
    }

    #[test]
    fn build_rustls_config_version_range() {
        let config = TlsConfig::builder()
            .min_tls_version(TlsVersion::Tls12)
            .max_tls_version(TlsVersion::Tls13)
            .build();
        let result = config.build_rustls_config();
        assert!(result.is_ok());
    }

    #[test]
    fn parse_empty_pem_certificates() {
        let result = parse_pem_certificates(b"");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_invalid_pem_certificates() {
        let result = parse_pem_certificates(b"not valid pem data");
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_pem_certificates_skip_garbage_between_blocks() {
        // A corrupted/comment section between two certificates must not
        // silently discard the rest of the bundle.
        let valid_block = |body: &[u8]| -> Vec<u8> {
            let mut block = b"-----BEGIN CERTIFICATE-----\n".to_vec();
            block.extend_from_slice(body);
            block.extend_from_slice(b"\n-----END CERTIFICATE-----\n");
            block
        };
        // Valid base64 bodies (pem_rfc7468 validates base64 content).
        let first = valid_block(b"AAAA");
        let second = valid_block(b"BBBB");
        let mut pem = first;
        pem.extend_from_slice(b"\nthis is not valid PEM \x01\x02 data\n");
        pem.extend_from_slice(&second);

        let certs = parse_pem_certificates(&pem).unwrap();
        assert_eq!(certs.len(), 2, "both certificates must be recovered");
    }

    #[test]
    fn parse_pem_certificates_multi_block_bundle() {
        // A plain concatenated bundle (the standard CA-bundle shape)
        // must yield every certificate.
        let mut pem = b"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n".to_vec();
        pem.extend_from_slice(b"-----BEGIN CERTIFICATE-----\nBBBB\n-----END CERTIFICATE-----\n");
        pem.extend_from_slice(b"-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n");

        let certs = parse_pem_certificates(&pem).unwrap();
        assert_eq!(certs.len(), 2, "both certificates, key section skipped");
    }

    #[test]
    fn parse_private_key_skips_leading_garbage() {
        let key_block = b"-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n";
        let mut pem = b"corrupt \x03 preamble\n".to_vec();
        pem.extend_from_slice(key_block);

        let (der, label) = parse_private_key(&pem).unwrap();
        assert_eq!(label, "PRIVATE KEY");
        // Base64 "AAAA" decodes to three NUL bytes.
        assert_eq!(der, vec![0u8, 0, 0]);
    }

    #[test]
    fn parse_pem_certificates_rejects_corrupted_blocks() {
        let pem = b"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n\
-----BEGIN CERTIFICATE-----\n%%%\n-----END CERTIFICATE-----\n";
        let err = parse_pem_certificates(pem).unwrap_err();
        assert_eq!(err.kind(), "ca_bundle");
    }

    #[test]
    fn error_variants() {
        let err = Error::TlsConfig("test".into());
        assert_eq!(err.kind(), "tls_config");

        let err = Error::CaBundle("test".into());
        assert_eq!(err.kind(), "ca_bundle");

        let err = Error::ClientCert("test".into());
        assert_eq!(err.kind(), "client_cert");

        let err = Error::PrivateKey("test".into());
        assert_eq!(err.kind(), "private_key");

        let err = Error::CertificateVerification("test".into());
        assert_eq!(err.kind(), "certificate_verification");

        let err = Error::HostnameVerification("test".into());
        assert_eq!(err.kind(), "hostname_verification");
    }

    #[test]
    fn debug_impl_no_secrets() {
        let config = TlsConfig::builder().build();
        let debug = format!("{config:?}");
        assert!(debug.contains("verify_hostname: true"));
        assert!(debug.contains("verify_certificate: true"));
    }

    #[test]
    fn client_identity_debug_no_private_key() {
        let identity = ClientIdentity::Pem {
            cert_chain: vec![],
            private_key_der: b"super-secret-private-key-data".to_vec(),
            key_label: "PRIVATE KEY".to_string(),
        };
        let debug = format!("{identity:?}");
        assert!(
            !debug.contains("super-secret"),
            "ClientIdentity Debug must not leak private key bytes: {debug}"
        );
        assert!(debug.contains("cert_count"));
        assert!(debug.contains("key_label"));
    }

    proptest::proptest! {
        #[test]
        fn parse_pem_certificates_empty_input(_ in 0u64..100) {
            let result = parse_pem_certificates(b"");
            prop_assert!(result.unwrap().is_empty());
        }

        #[test]
        fn parse_pem_certificates_garbage(input in ".{0,200}") {
            // Must not panic
            let _ = parse_pem_certificates(input.as_bytes());
        }

        #[test]
        fn tls_config_builder_round_trip(sni in prop::bool::ANY) {
            let config = TlsConfig::builder()
                .sni(sni)
                .build();
            prop_assert_eq!(config.sni_enabled(), sni);
            // Defaults are always true
            prop_assert!(config.verify_hostname());
            prop_assert!(config.verify_certificate());
        }

        #[test]
        fn build_rustls_config_valid_for_defaults(_ in 0u64..100) {
            let config = TlsConfig::default();
            let result = config.build_rustls_config();
            prop_assert!(result.is_ok());
        }
    }
}
