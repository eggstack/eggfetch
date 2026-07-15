#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::{TlsConfig, TlsVersion, TrustStore};

fuzz_target!(|data: &[u8]| {
    // Test TlsConfigBuilder with various configurations.
    // The builder should never panic regardless of input.

    // Default builder.
    let config = TlsConfig::builder().build();
    let _ = config.verify_hostname();
    let _ = config.verify_certificate();
    let _ = config.sni_enabled();

    // Builder with various TLS version combinations.
    let _ = TlsConfig::builder()
        .min_tls_version(TlsVersion::Tls12)
        .max_tls_version(TlsVersion::Tls13)
        .build();

    let _ = TlsConfig::builder()
        .min_tls_version(TlsVersion::Tls13)
        .max_tls_version(TlsVersion::Tls12)
        .build();

    // Builder with SNI toggled.
    let _ = TlsConfig::builder().sni(false).build();
    let _ = TlsConfig::builder().sni(true).build();

    // Builder with verification toggled.
    let _ = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    let _ = TlsConfig::builder()
        .danger_accept_invalid_certs(false)
        .build();

    // Test PEM certificate parsing with arbitrary bytes.
    let _ = TlsConfig::builder().ca_certificate_pem(data);

    // Test with empty PEM.
    let _ = TlsConfig::builder().ca_certificate_pem(b"");

    // Test TrustStore variants.
    let _ = TrustStore::NativeWithWebPkiFallback;
    let _ = TrustStore::NativeOnly;
    let _ = TrustStore::WebPkiOnly;

    // Build rustls config should never panic.
    let config = TlsConfig::builder().build();
    let _ = config.build_rustls_config();

    let config = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    let _ = config.build_rustls_config();

    // Invalid version range should return error, not panic.
    let config = TlsConfig::builder()
        .min_tls_version(TlsVersion::Tls13)
        .max_tls_version(TlsVersion::Tls12)
        .build();
    let _ = config.build_rustls_config();
});
