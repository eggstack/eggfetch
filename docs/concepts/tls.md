# TLS Configuration

eggfetch uses rustls for TLS, providing memory-safe HTTPS with configurable trust stores, client certificates, and verification policy.

## Crypto Providers

Rustls crypto selection is local to each `TlsConfig`. A native Rust application
can construct any compatible `Arc<rustls::crypto::CryptoProvider>`—including a
provider supplied by a separately selected Rustls feature—and pass it through
`TlsConfigBuilder::crypto_provider`:

```rust
use std::sync::Arc;
use eggfetch_core::TlsConfig;

let tls = TlsConfig::builder()
    .crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
    .build();
```

The provider is retained when `TlsConfig` is cloned and governs both server
certificate verification and mTLS private-key loading. Building or using the
configuration never installs it as the process-wide Rustls default. If no
provider is supplied, eggfetch preserves its existing process-default/ring
fallback behavior. A provider's algorithm, FIPS, or post-quantum properties
come from the exact provider build and are not claims made by eggfetch.

## Verification Defaults

Certificate verification and hostname verification are enabled by default. The client validates the server's certificate chain against trusted roots and confirms the hostname matches the certificate.

## Trust Stores

eggfetch resolves trusted roots in this order, then overlays any explicit
additional roots:

1. **Custom CA bundle** -- if provided via `TlsConfig`, replaces all default roots
2. **Native system roots** -- the operating system's trust store
3. **Packaged Mozilla/WebPKI roots** -- fallback for minimal containers where native roots are unavailable

The `TrustStore` enum controls this behavior:

- `NativeWithWebPkiFallback` (default) -- try native roots, fall back to WebPKI
- `NativeOnly` -- fail if native roots are unavailable
- `WebPkiOnly` -- use only packaged roots
- `Custom(certs)` -- use only the provided CA certificates

A custom CA bundle **replaces** all default roots. Native Rust callers that
need both the selected base and private roots should use
`additional_ca_certificate_path`, `additional_ca_certificate_pem`, or
`additional_ca_certificate_der`; these methods do not alter hostname/SNI
verification or discover certificates from the environment. The existing
`add_ca_certificate_path` method remains replacement-style for compatibility.

## Custom CA Bundles

```rust
use eggfetch_core::TlsConfig;

let config = TlsConfig::builder()
    .ca_certificate_path("/path/to/ca-bundle.pem")?
    .build();
```

To add a private root while retaining the selected base trust policy:

```rust
let config = TlsConfig::builder()
    .additional_ca_certificate_path("/path/to/private-ca.pem")?
    .build();
```

Malformed additional certificate material is rejected before network I/O.
The Python and HTTPX-compatible facades retain their existing replacement CA
semantics; this additive API is native Rust functionality.

```python
client = eggfetch.Client(verify="/path/to/ca-bundle.pem")
```

## Client Certificates (mTLS)

For mutual TLS authentication, provide a certificate chain and private key:

```rust
let config = TlsConfig::builder()
    .client_cert_path("/path/to/cert.pem", "/path/to/key.pem")?
    .build();
```

```python
client = eggfetch.Client(cert=("/path/to/cert.pem", "/path/to/key.pem"))
```

Both PEM certificate chains and private keys are supported. Encrypted private keys produce an error at construction time. Private key material is never included in debug output or error messages.

## TLS Version Policy

Control the minimum and maximum TLS versions:

```rust
use eggfetch_core::{TlsConfig, TlsVersion};

let config = TlsConfig::builder()
    .min_tls_version(TlsVersion::Tls12)
    .max_tls_version(TlsVersion::Tls13)
    .build();
```

Both TLS 1.2 and TLS 1.3 are supported by default. Setting `min` to `Tls13` restricts to TLS 1.3 only.

## SNI

Server Name Indication (SNI) is enabled by default. Disabling SNI can break hostname verification and virtual-host routing and should rarely be necessary.

## Disabling Verification

As a testing escape hatch, certificate verification can be disabled:

```rust
let config = TlsConfig::builder()
    .danger_accept_invalid_certs(true)
    .build();
```

```python
client = eggfetch.Client(verify=False)
```

This disables both certificate chain verification and hostname verification. It should only be used against known self-signed certificates in testing environments.

## Python API

| Kwarg | Type | Description |
|-------|------|-------------|
| `verify=True` | bool | Enable verification (default) |
| `verify=False` | bool | Disable verification (insecure) |
| `verify="/path/to/ca.pem"` | str | Custom CA bundle path |
| `cert="/path/to/cert.pem"` | str | Client certificate (single PEM file) |
| `cert=("/path/cert.pem", "/path/key.pem")` | tuple | Client certificate and private key |

## CLI

```bash
# Disable verification (insecure)
eggfetch --no-verify https://self-signed.example.com

# Custom CA bundle
eggfetch --cacert /path/to/ca-bundle.pem https://example.com

# Client certificate
eggfetch --cert /path/to/cert.pem --key /path/to/key.pem https://example.com
```
