# Custom dialer qualification fixture

This tiny external-style consumer depends on `eggfetch-core` through its
public path API with the minimal HTTP/1 + Rustls profile. It proves that a
caller can install a raw-stream `Dialer`, strict Hyper canceled-request
behavior, a physical connection cap, and established-transport I/O timers
without reaching into `eggfetch-core::transport` internals.

Run locally with:

```sh
cargo run --manifest-path qualification/embedded-custom-dialer/Cargo.toml
```
