# Native HTTP body and TLS qualification fixture

This external-style consumer uses only the public `eggfetch-core` API. It
proves that a caller can combine a frame-preserving request/response body,
custom dialing, an explicitly selected AWS-LC Rustls provider, mTLS client-key
loading, and an additional private trust anchor without importing downstream
application code. It also checks that using the explicit provider does not
mutate Rustls's process-global provider.

Run it locally with:

```sh
cargo run --manifest-path qualification/native-http-body-tls/Cargo.toml
```
