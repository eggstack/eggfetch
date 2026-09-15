# Native Tower service qualification

This external-style fixture verifies that `eggfetch_core::NativeHttpService`
can be passed directly to Tonic's generic `Grpc` transport constructor. It
uses only the public `tower_service::Service<http::Request<B>>` boundary; no
Tonic type is part of eggfetch-core's API or dependency graph.

Run it manually from the repository root:

```sh
cargo run --manifest-path qualification/native-tower-service/Cargo.toml
```

The fixture intentionally performs a compile-time transport-bound check and
does not contact a server. HTTP/2 selection remains an ordinary eggfetch
`HttpVersionPolicy` setting. The fixture's generated `target/` directory is
build output and must not be committed.
