# Native HTTP body and TLS qualification fixture

This external-style consumer uses only the public `eggfetch-core` API. It
proves that a caller can combine a frame-preserving request/response body,
custom dialing, an explicitly selected Rustls provider, and an additional
private trust anchor without importing downstream application code.

Run it locally with:

```sh
cargo run --manifest-path qualification/native-http-body-tls/Cargo.toml
```
