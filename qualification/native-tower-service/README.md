# Native Tower service qualification

This external-style fixture verifies that `eggfetch_core::NativeHttpService`
can be passed directly to Tonic 0.14.6's generic `Grpc` transport constructor
using the `tonic::body::Body` request-body shape and generated-client response
body bounds. It uses only the public `tower_service::Service<http::Request<B>>`
boundary; no Tonic type is part of eggfetch-core's API or dependency graph.

The fixture enables Tonic's `codegen` feature only. Tonic `transport`/`Channel`,
server, and TLS features are intentionally disabled because this qualifies
eggfetch's native transport rather than Tonic's transport stack. Compatibility
with Tonic's interceptor convenience helper is not implied; that helper has
additional `Default` requirements outside this qualification.

Run it manually from the repository root:

```sh
cargo run --manifest-path qualification/native-tower-service/Cargo.toml
```

The fixture intentionally performs a compile-time transport-bound check and
does not contact a server. It is an external/manual qualification fixture, not a
runtime eggfetch dependency or routine compatibility suite. HTTP/2 selection
remains an ordinary eggfetch `HttpVersionPolicy` setting. The fixture's
generated `target/` directory is build output and must not be committed.
