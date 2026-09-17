# Shared HTTP CONNECT Wire Primitive and Release-Graph Prerequisite

## Status

**READY FOR IMPLEMENTATION — 2026-09-16**

## Baseline

Written against Eggfetch `main` at:

```text
0f720b4fd9efea80d180ce5942fce5e3453d8003
```

At this baseline:

- `eggfetch-core` is `0.1.5`;
- the workspace MSRV is Rust `1.89`;
- HTTPS proxy CONNECT is still assembled and parsed inside `eggfetch-core`;
- `establish_https_tunnel()` owns the ordered proxy-connect -> CONNECT write/read -> origin-TLS sequence;
- `read_proxy_response()` is generic over an async reader but returns Eggfetch-specific errors and uses hard-coded proxy-response limits;
- release/package validation assumes `eggfetch-core` has no internal publishable dependencies and therefore dry-runs/publishes it first;
- the version validator and internal-dependency validator both hard-code the current five publishable crates/topology.

This plan exists because a downstream consumer needs the HTTP/1 CONNECT wire mechanics without depending on the full Eggfetch client engine. The motivating consumer is Eggress, but this plan must produce a generally useful Eggfetch primitive with no Eggress-specific runtime types, feature flags, or policy branches.

Before implementation, fetch current `main` and re-check every path and invariant below. If an equivalent published stream-level CONNECT primitive already exists, stop and adapt the downstream consumer to it instead of creating another one.

---

# Objective

Create one small, publishable Eggfetch-owned crate that contains only reusable HTTP/1 CONNECT wire mechanics over an already-established asynchronous byte stream, then make `eggfetch-core` consume that crate while preserving Eggfetch's current routing, timeout, pooling, TLS, retry, error-classification, and public API behavior.

The desired ownership is:

```text
caller-owned transport stream
        |
        v
+-----------------------------+
| eggfetch-http-connect       |
|-----------------------------|
| target/authority formatting |
| CONNECT request bytes       |
| proxy auth/header encoding  |
| bounded response-head parse |
| read-ahead preservation     |
+-----------------------------+
        |
        +---------------------> eggfetch-core policy adapter
        |                         - proxy dialing/TLS
        |                         - phase timeouts
        |                         - exact status policy
        |                         - rejection-body handling
        |                         - origin TLS
        |                         - pooling/route identity
        |
        +---------------------> other stream-oriented consumers
```

A suitable package name is:

```text
eggfetch-http-connect
```

If that crates.io name is unavailable, choose another narrow Eggfetch-owned name and record the final choice in the closure note. Do not create a name or API that implies the crate is specifically for Eggress.

The primary success metric is maintenance consolidation and correctness. Binary-size reduction is not a requirement for this upstream pass.

---

# Why the current implementation blocks reuse

## 1. CONNECT wire behavior is embedded in `establish_https_tunnel()`

Current `crates/eggfetch-core/src/transport/connect.rs` performs all of these operations in one Eggfetch-owned flow:

1. connect to the configured proxy;
2. determine the CONNECT authority;
3. serialize `CONNECT ... HTTP/1.1` and `Host`;
4. serialize `Proxy-Authorization` and proxy-only headers;
5. enforce Eggfetch's request-header-size ceiling;
6. apply Eggfetch write timeout/deadline policy;
7. parse the proxy response;
8. apply Eggfetch read timeout/deadline policy;
9. classify exact status behavior;
10. build Eggfetch's rejection error/body;
11. preserve read-ahead bytes via `ProxyTunnel`;
12. perform origin TLS and ALPN setup.

Only items 2-4 and 7 plus the read-ahead mechanics are generic CONNECT wire behavior. The remaining items are Eggfetch client policy and must stay in `eggfetch-core`.

A reusable crate must therefore extract the protocol operations, not move `establish_https_tunnel()` wholesale.

## 2. `read_proxy_response()` is close to reusable but not a public cross-crate boundary

Current `crates/eggfetch-core/src/transport/proxy.rs` already has a useful bounded byte-preserving parser:

```text
read_proxy_response<S: AsyncRead + Unpin>(
    &mut BufReader<S>
) -> Result<(u16, Vec<(String, Vec<u8>)>, Vec<u8>, Option<String>)>
```

It correctly avoids requiring response header values to be UTF-8 and preserves bytes already read past the header terminator. However it is unsuitable as-is for external reuse because:

- it returns `eggfetch_core::Error`;
- its limits are hard-coded rather than caller-provided;
- its diagnostics and rejection semantics are Eggfetch-specific;
- it also serves general proxy-response internals and should not be moved wholesale if that creates unnecessary coupling.

The new crate should own a small generic CONNECT response-head parser. Eggfetch may continue to retain a separate forward-proxy response/body parser where that behavior is not actually shared.

## 3. Eggfetch's release graph assumes `eggfetch-core` is the leaf package

Current `scripts/check.sh package` explicitly states:

```text
# eggfetch-core has no internal deps — full dry-run succeeds independently.
```

Current release docs publish in this order:

```text
eggfetch-core
  -> eggfetch-cli
  -> eggfetch-ffi
  -> eggfetch-python
  -> eggfetch-node
```

Adding a published low-level crate changes the graph to:

```text
eggfetch-http-connect
        |
        v
eggfetch-core
  +-----+---------+
  |     |         |
 cli   ffi      python
        |
        v
       node
```

If code extraction is implemented without updating package validation and release policy, the repository will become internally correct but operationally unpublishable or misleading. The release-graph correction is therefore part of this prerequisite, not optional follow-up cleanup.

---

# Non-goals

Do not use this plan to:

- redesign `Client`, `ClientBuilder`, `ProxyConfig`, `Dialer`, `NativeHttpService`, or response APIs;
- move proxy TCP connection ownership out of `eggfetch-core`;
- move proxy-endpoint TLS out of `eggfetch-core`;
- move origin TLS or ALPN policy out of `eggfetch-core`;
- change route-key identity or reusable Hyper-client ownership;
- change exact CONNECT success/rejection policy unless a separate behavior fix is explicitly approved;
- add HTTP/2 CONNECT support to the new crate;
- add HTTP/3/QUIC CONNECT support;
- extract SOCKS in the same pass;
- generalize this into a proxy-chain framework;
- change retry semantics for pinned target attempts;
- add Eggress types, modules, feature names, error variants, or adapters;
- add a new mandatory CI matrix solely for the new crate;
- make a binary-size claim without measurement;
- lower or raise `eggfetch-core`'s MSRV as a side effect;
- broaden the existing forward-proxy parser refactor unless it clearly reduces duplication without changing behavior.

If implementation starts requiring any of those expansions, stop and create a new narrow plan instead of silently increasing scope.

---

# Workstream 0 — Reconfirm current ownership and package assumptions

Before editing, inspect the current versions of:

```text
Cargo.toml
crates/eggfetch-core/Cargo.toml
crates/eggfetch-core/src/transport/connect.rs
crates/eggfetch-core/src/transport/proxy.rs
crates/eggfetch-core/src/proxy.rs
crates/eggfetch-core/src/error.rs
crates/eggfetch-core/src/headers.rs
scripts/check.sh
scripts/validate_release_versions.py
scripts/test_validate_release_versions.py
scripts/validate_publishable_internal_dependencies.py
docs/releases/process.md
docs/verification-policy.md
plans/README.md
```

Search the repository for every hard-coded publishable-crate inventory and every assumption that `eggfetch-core` has no internal dependencies.

At minimum search for:

```text
PUBLISHABLE_CRATES
EXPECTED_DEPENDENCIES
eggfetch-core has no internal deps
cargo publish -p eggfetch-core
cargo publish --dry-run -p eggfetch-core
eggfetch-node
```

Record any additional scripts/docs that must change before implementation starts.

### Decision gate

Proceed only if a new low-level crate can remain free from Hyper client routing, TLS, socket creation, DNS, retry, and Eggfetch public-client types.

If the only viable implementation requires depending on `eggfetch-core`, the abstraction is wrong and this plan should stop.

---

# Workstream 1 — Add the small publishable crate

Add a workspace member, preferably:

```text
crates/eggfetch-http-connect/
```

The crate must be independently publishable and must not depend on `eggfetch-core`.

## Dependency budget

Prefer only dependencies directly required by wire behavior. Likely candidates:

- `tokio` with the minimum I/O features needed for `AsyncRead`, `AsyncWrite`, and `BufReader`;
- `base64` if Basic proxy-auth construction is centralized here;
- `http` if using `HeaderName`/`HeaderValue` materially improves structural validation;
- `thiserror` if a small typed protocol error keeps the API clearer.

Do not add:

- `hyper`;
- `hyper-util`;
- `tower-service`;
- `rustls`, `tokio-rustls`, or `hyper-rustls`;
- `url` solely to format CONNECT authorities;
- DNS/resolver dependencies;
- socket-creation dependencies;
- retry/backoff dependencies;
- serde;
- Eggress dependencies.

The package must inherit or explicitly declare the repository's normal license, edition, lints, and MSRV policy.

## Versioning

Eggfetch's release contract currently coordinates all publishable crates on one version. Preserve that policy unless a separate versioning-policy change is approved.

Therefore the new crate should join the next selected coordinated Eggfetch release version rather than invent an independent version line. Do not hard-code a specific next patch number in implementation until the maintainer confirms the next unpublished version.

---

# Workstream 2 — Define the generic CONNECT wire API

The exact Rust surface may vary, but the following semantic boundary is required.

## 2.1 Target / authority ownership

The shared crate must own CONNECT authority formatting so every consumer does not reimplement IPv6 rules.

A conceptual API is:

```rust
pub struct ConnectTarget {
    host: String,
    port: u16,
}

impl ConnectTarget {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, ConnectError>;
    pub fn authority(&self) -> String;
}
```

Required behavior:

- domain -> `example.com:443`;
- IPv4 -> `127.0.0.1:443`;
- IPv6 -> `[::1]:443`;
- empty host rejected;
- CR/LF rejected;
- other request-line control bytes rejected;
- pre-bracketed and unbracketed IPv6 handling is deterministic and tested;
- no path/query/body semantics exist in this type.

Do not require callers to hand-format authority strings.

## 2.2 Request serialization

Expose a byte-oriented serializer that can be tested independently from network I/O.

Conceptually:

```rust
pub struct ConnectRequest<'a> {
    pub target: &'a ConnectTarget,
    pub proxy_authorization: Option<&'a ...>,
    pub extra_headers: ...,
    pub max_head_bytes: usize,
}

pub fn encode_connect_request(request: &ConnectRequest<'_>)
    -> Result<Vec<u8>, ConnectError>;
```

Required wire shape:

```text
CONNECT host:port HTTP/1.1\r\n
Host: host:port\r\n
[Proxy-Authorization: ...\r\n]
[proxy-only headers...]
\r\n
```

Required invariants:

- request-target and `Host` authority are identical;
- IPv6 authority is bracketed;
- request head is bounded before write;
- proxy-only headers preserve byte values where HTTP permits;
- `Proxy-Authorization` ownership is deterministic;
- a caller cannot accidentally emit two auth headers;
- reserved headers cannot override the generated CONNECT authority;
- CR/LF/header injection is structurally rejected;
- serialization has no timeout, socket, TLS, or retry behavior.

## 2.3 Basic auth helper

If the shared crate owns Basic auth serialization, expose a small helper that:

- validates username/password control characters;
- produces the correct `Basic <base64(user:pass)>` value;
- marks secret-bearing header values as sensitive where the chosen header representation supports it;
- does not derive or implement `Debug`/`Display` in a way that prints credentials;
- never echoes credentials in errors.

If Eggfetch's existing `ProxyAuth` continues to build the header value, the shared API must still accept it without depending on `ProxyAuth` itself.

## 2.4 Caller-provided response limits

Replace hard-coded parser constants at the shared boundary with an explicit limits type, for example:

```rust
pub struct ConnectResponseLimits {
    pub max_status_line: usize,
    pub max_header_line: usize,
    pub max_headers_bytes: usize,
    pub max_header_count: usize,
}
```

Required semantics:

- `max_header_count` counts actual header fields only;
- status line and terminal blank line are not counted as headers;
- exact limit is accepted;
- one-past limit is rejected;
- aggregate byte accounting is documented and tested;
- no unbounded allocation precedes limit checks.

The shared crate may provide Eggfetch-oriented defaults, but callers must be able to supply different limits.

## 2.5 Response-head parser

Expose a parser over an already-buffered caller-owned stream, conceptually:

```rust
pub async fn read_connect_response_head<S>(
    stream: &mut tokio::io::BufReader<S>,
    limits: &ConnectResponseLimits,
) -> Result<ConnectResponseHead, ConnectError>
where
    S: tokio::io::AsyncRead + Unpin;
```

A conceptual result is:

```rust
pub struct ConnectResponseHead {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub reason_phrase: Option<Vec<u8>>,
}
```

The exact public representation may use `http` types if that improves correctness, but it must preserve the following behavior:

- no whole-head UTF-8 requirement;
- header names validated safely;
- header values retained as bytes / equivalent lossless representation;
- bounded status line;
- bounded individual header lines;
- bounded aggregate header bytes;
- bounded header count;
- premature EOF fails closed;
- malformed status fails closed;
- status is returned to the caller without imposing universal success policy;
- response body is not consumed merely to establish a tunnel.

Do not silently tighten status/version acceptance beyond current Eggfetch behavior during extraction. Any intentional protocol-hardening change must be called out and tested separately.

## 2.6 Read-ahead preservation

The parser must not discard bytes already read after `\r\n\r\n`.

The preferred design for Eggfetch is to operate directly on the existing `BufReader<ProxyIo>` so its buffer remains available after parsing. Eggfetch may then continue its existing `ProxyTunnel` conversion or another equivalent internal transfer without moving that Eggfetch-specific type into the shared crate.

Tests must send response head and initial tunneled payload in one write and prove those payload bytes remain readable after the parser returns.

## 2.7 Timeout ownership

The shared crate must not own Eggfetch phase timeout policy.

Eggfetch currently wraps CONNECT write and CONNECT read separately using `effective_timeout()` and `TimeoutPhase::{Write,Read}`. Preserve that structure by exposing operations that Eggfetch can wrap externally:

```text
encode/write CONNECT request
read CONNECT response head
```

Do not collapse them into a convenience function if doing so prevents Eggfetch from retaining its current write/read timeout classification.

A convenience high-level helper is optional only if the lower-level operations remain available and are the path used by `eggfetch-core`.

---

# Workstream 3 — Define a neutral error contract

Create a small protocol error type that contains no Eggfetch client-policy variants.

Expected categories include:

- invalid target;
- invalid credential/header input;
- request head too large;
- I/O failure;
- malformed status line;
- malformed header line;
- status line too large;
- header line too large;
- total response head too large;
- too many headers;
- unexpected EOF.

Do not encode:

- Eggfetch `TimeoutPhase`;
- `ProxyConnectRejected`;
- retryability;
- proxy route identity;
- TLS failure;
- destination/origin policy;
- Eggress error variants.

`eggfetch-core` maps shared errors into its existing public/internal `Error` variants at the adapter boundary.

Diagnostics must be bounded and must not include raw credentials or unbounded attacker-controlled response lines.

---

# Workstream 4 — Refactor `eggfetch-core` to consume the primitive

Modify only the CONNECT wire portion of `establish_https_tunnel()`.

The intended structure is approximately:

```text
connect_to_proxy(...)                         [unchanged eggfetch-core]
    -> build ConnectTarget                    [shared primitive]
    -> encode_connect_request(...)            [shared primitive]
    -> effective_timeout(write) + write_all   [unchanged eggfetch-core]
    -> read_connect_response_head(...)        [shared primitive]
       wrapped by effective_timeout(read)     [unchanged eggfetch-core]
    -> exact status/rejection mapping         [unchanged eggfetch-core]
    -> preserve buffered post-head bytes      [shared parser + existing adapter]
    -> origin TLS / ALPN                      [unchanged eggfetch-core]
```

## Behavior that must remain Eggfetch-owned

Preserve all of the following:

- `connect_to_proxy()` and proxy TCP/TLS endpoint handling;
- `proxy_connect_timeout`;
- `proxy_tls_timeout`;
- request total deadline handling;
- CONNECT write timeout classification;
- CONNECT read timeout classification;
- current exact status acceptance/rejection semantics;
- `proxy_rejection_body()` behavior;
- retry behavior for pinned target candidates;
- 502/504 retry classification;
- `ProxyTunnel` or equivalent read-ahead ownership;
- destination/origin TLS config;
- SNI override behavior;
- HTTP version/ALPN policy;
- transport metrics;
- route/cache identity;
- Hyper connector pooling;
- public `Client`/`ClientBuilder`/`Proxy`/`Dialer` APIs.

Do not expose the new crate's types through existing Eggfetch public client APIs merely because the crate is public.

## Forward proxy parser scope

Do not automatically replace all uses of `read_proxy_response()`.

If ordinary forward-proxy body parsing still needs its current Eggfetch-specific parser, leave it in `eggfetch-core`. Shared code should be introduced only where behavior is genuinely the same.

It is acceptable for the repository to retain a small forward-response parser after CONNECT extraction if that avoids coupling body framing or Eggfetch-specific response semantics into the new crate.

---

# Workstream 5 — Correct the workspace and release graph

This workstream is mandatory. The new crate cannot be considered complete if the repository's package/release tooling still assumes `eggfetch-core` is dependency-free.

## 5.1 Workspace membership

Add the new crate to root workspace members and normal workspace lint/license/repository conventions.

Update `Cargo.lock` normally.

## 5.2 `eggfetch-core` manifest

Add the new crate as an internal path + concrete version dependency using the repository's existing publishable-internal dependency convention.

The packaged manifest must resolve from crates.io after publication; it must not rely on a path-only dependency.

Do not feature-gate the dependency in a way that reintroduces a second private CONNECT implementation unless a real minimal-feature requirement proves it necessary.

## 5.3 Coordinated version validator

Update:

```text
scripts/validate_release_versions.py
scripts/test_validate_release_versions.py
```

Add the new crate to the authoritative publishable-crate inventory and prove that version drift is rejected.

The validation must continue to fail closed if any coordinated publishable Cargo crate or Python package version differs.

## 5.4 Internal dependency topology validator

Update:

```text
scripts/validate_publishable_internal_dependencies.py
```

The expected topology must include at least:

```text
eggfetch-core   -> eggfetch-http-connect
eggfetch-cli    -> eggfetch-core
eggfetch-ffi    -> eggfetch-core
eggfetch-python -> eggfetch-core
eggfetch-node   -> eggfetch-ffi
```

Keep the existing requirements that every publishable internal dependency has both:

- a concrete registry version requirement;
- a local workspace path for development.

Add/update tests if the validator has a test fixture or script-level test surface.

## 5.5 Tier 3 package validation

Current `scripts/check.sh package` dry-runs `eggfetch-core` because it has no internal dependency. That assumption becomes false.

Update package validation so the new leaf crate receives the pre-publication full dry-run:

```text
cargo publish --dry-run -p eggfetch-http-connect
```

Before the new crate version exists on crates.io, `eggfetch-core` may need the same local package-structure validation currently used for other dependent crates:

```text
cargo package --list -p eggfetch-core
```

plus the internal-dependency topology/version validator.

Do not fake registry availability or use `--no-verify` as a release-policy workaround.

At actual publication time, after the shared crate is visible in the registry, run a real:

```text
cargo publish --dry-run -p eggfetch-core
```

before publishing `eggfetch-core`.

## 5.6 Release documentation

Update `docs/releases/process.md` to reflect the new dependency-first order.

The conceptual crates.io order becomes:

```text
1. eggfetch-http-connect
2. eggfetch-core
3. eggfetch-cli
4. eggfetch-ffi
5. eggfetch-python
6. eggfetch-node   (after eggfetch-ffi is visible)
```

The exact ordering of the independent core consumers may remain operator-driven as long as every crate is published only after its internal dependencies are visible.

Remove/replace statements that say `eggfetch-core` has no internal dependencies.

Do not automate crates.io publication as part of this plan; the current manual publication policy remains unchanged.

## 5.7 Other hard-coded crate inventories

Search all scripts/docs for the five-crate assumption and update every normative list that should now include the shared package.

Do not rewrite historical completed plans solely to insert the new crate. Historical plans may remain historical unless they are currently normative.

---

# Workstream 6 — Security and correctness tests

Add focused tests to the shared crate and retain consumer-level tests in `eggfetch-core`.

## 6.1 Target and request tests

Required cases:

- domain authority;
- IPv4 authority;
- IPv6 authority;
- explicit non-default port;
- empty host rejection;
- CR injection rejection;
- LF injection rejection;
- other control-character rejection where relevant;
- Basic auth exact encoding;
- credential control-character rejection;
- proxy-only headers serialized once;
- generated `Host` cannot be overridden inconsistently;
- duplicate `Proxy-Authorization` is prevented or deterministically resolved;
- exact request-head byte limit accepted;
- one-past request-head byte limit rejected.

## 6.2 Response parser tests

Required cases:

- ordinary `HTTP/1.1 200 Connection Established`;
- other valid 2xx status returned intact without universal policy classification;
- 403, 407, 502, 504 returned intact;
- arbitrary syntactically valid status returned intact;
- malformed status rejected;
- truncated status line rejected;
- truncated headers rejected;
- exact maximum header count accepted;
- maximum + 1 header rejected;
- exact aggregate limit accepted when the response is complete;
- aggregate + 1 rejected;
- status-line limit enforced;
- per-header-line limit enforced;
- obs-text / non-UTF-8 header values preserved;
- malformed header name rejected;
- duplicate response headers preserved according to the chosen representation;
- read-ahead tunnel bytes remain available after the parser returns.

## 6.3 Secret handling tests

Required cases:

- auth configuration `Debug`/`Display` does not reveal credentials;
- shared error messages never include Base64 credentials;
- invalid header/target errors do not echo unbounded attacker input;
- no trace/log statement in the new crate renders secret header values.

## 6.4 Eggfetch integration regressions

Retain or add tests proving that refactoring does not change:

- proxy TLS behavior;
- origin TLS behavior;
- SNI overrides;
- exact current CONNECT success status policy;
- 403/407/502/504 error classification;
- rejection-body sanitization;
- write vs read timeout phase classification;
- total deadline behavior;
- pinned target retry behavior;
- route-cache identity;
- persistent CONNECT client reuse;
- transport metrics;
- custom proxy headers/auth behavior.

Do not replace these consumer tests with shared-crate unit tests; both layers need coverage.

---

# Workstream 7 — Documentation and public API discipline

Add concise rustdoc to the shared crate that makes its boundary explicit:

```text
- caller supplies an already-connected async stream;
- crate serializes/parses HTTP/1 CONNECT wire data;
- crate does not dial sockets;
- crate does not perform TLS;
- crate does not retry;
- crate does not decide whether a status is acceptable;
- crate does not own request deadlines.
```

Include one small generic example using an in-memory/Tokio stream or another self-contained fixture.

Update only durable Eggfetch architecture docs necessary to show:

```text
eggfetch-core transport/connect
    -> eggfetch-http-connect wire primitive
```

Do not add Eggress-specific instructions to the crate's public rustdoc or package description.

No existing `eggfetch-core` public API should break. Run the repository's normal API/compatibility validation because production core internals are changing even though public signatures are intended to remain stable.

---

# Workstream 8 — Validation

Use current repository commands if names have moved. At minimum run the equivalent of:

```sh
cargo fmt --all -- --check
cargo clippy -p eggfetch-http-connect --all-targets -- -D warnings
cargo test -p eggfetch-http-connect
cargo doc -p eggfetch-http-connect --no-deps

cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo test -p eggfetch-core --all-features

python scripts/test_validate_release_versions.py
python scripts/validate_publishable_internal_dependencies.py

./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Run the Rust `1.89.0` MSRV gate from the existing extended validation.

If the new shared crate deliberately declares an earlier MSRV for downstream compatibility, add a focused build/test at that declared MSRV in addition to—not instead of—the workspace's existing Rust 1.89 gate. Do not change the whole Eggfetch workspace MSRV solely for this consumer.

Run the repository's current exact-SHA API/compatibility qualification process required for executable `eggfetch-core` changes. Do not invent a new qualification framework.

---

# Workstream 9 — Publication and downstream handoff

The downstream consumer cannot close its migration until a registry-resolvable shared package exists.

## Publication sequence

1. select the next coordinated Eggfetch version according to normal maintainer policy;
2. update all coordinated publishable versions including the new crate;
3. update changelog/release metadata as normally required;
4. run routine, extended, security, and package validation;
5. run `cargo publish --dry-run -p eggfetch-http-connect`;
6. manually publish `eggfetch-http-connect`;
7. wait until crates.io resolution sees that exact version;
8. run `cargo publish --dry-run -p eggfetch-core`;
9. manually publish `eggfetch-core`;
10. publish remaining dependent crates in dependency order using the existing manual process;
11. verify a clean temporary Cargo project can resolve the shared crate and `eggfetch-core` from crates.io without workspace paths.

Do not leave the downstream consumer on a permanent git dependency or path dependency.

## Handoff record

At closure, append a concise record containing:

```text
shared crate name:
shared crate version:
eggfetch implementation commit:
registry availability verified:
eggfetch-core version consuming it:
Tier 1 result:
extended/MSRV result:
package result:
security preflight result:
compatibility/API qualification result:
```

The downstream implementation may then depend on the published shared crate directly without pulling in the full `eggfetch-core` dependency graph.

---

# Acceptance criteria

This plan is complete only when all of the following are true:

- [ ] A small published Eggfetch-owned HTTP/1 CONNECT wire crate exists.
- [ ] The crate does not depend on `eggfetch-core`.
- [ ] The crate has no Hyper client/pool, TLS, DNS, socket-creation, retry, or routing dependency.
- [ ] CONNECT target formatting is centralized and IPv6 authority is correct.
- [ ] Request serialization is byte-bounded and injection-safe.
- [ ] Basic proxy auth is supported without secret leakage, directly or through a neutral header input contract.
- [ ] Response limits are caller-configurable.
- [ ] Header count means actual header fields.
- [ ] Response parsing does not require whole-head/header-value UTF-8.
- [ ] Truncated responses fail closed.
- [ ] Read-ahead tunnel bytes are preserved.
- [ ] The parser returns status without imposing Eggfetch-specific success policy.
- [ ] `eggfetch-core` uses the shared crate for its CONNECT request/response wire mechanics.
- [ ] The old duplicate CONNECT serializer/parser logic is removed or reduced to thin Eggfetch policy adapters.
- [ ] Eggfetch's write/read timeout phase ownership remains unchanged.
- [ ] Eggfetch's exact CONNECT status/rejection behavior remains unchanged unless separately approved.
- [ ] Proxy endpoint TLS, origin TLS, SNI, pooling, route identity, retries, metrics, and rejection-body behavior remain Eggfetch-owned and regression-tested.
- [ ] Existing public `eggfetch-core` APIs remain source-compatible.
- [ ] The new crate is added to coordinated release-version validation.
- [ ] The internal dependency validator reflects `eggfetch-core -> shared crate`.
- [ ] `scripts/check.sh package` no longer assumes `eggfetch-core` has no internal dependencies.
- [ ] Release docs publish the shared crate before `eggfetch-core`.
- [ ] No normative script/document retains a false five-crate/dependency-root assumption.
- [ ] Routine, extended/MSRV, package, security, API, and compatibility gates pass according to current repository policy.
- [ ] The shared crate and the consuming `eggfetch-core` version are available from crates.io.
- [ ] Clean registry-only downstream resolution succeeds.
- [ ] No Eggress-specific runtime type, feature, branch, or dependency exists in the new crate or `eggfetch-core` implementation.

---

# Stop conditions

Stop and revise the design instead of forcing the extraction if any of these occur:

- the shared crate needs Hyper client/pool machinery;
- the shared crate needs TLS or socket creation;
- Eggfetch cannot preserve separate CONNECT write/read timeout classification without duplicating the protocol implementation;
- read-ahead preservation requires exposing Eggfetch-private transport types publicly;
- the only way to consume the code externally is to depend on full `eggfetch-core`;
- the release graph cannot be made publishable without breaking the repository's existing manual release policy;
- the extraction requires redesigning route caching, Hyper pooling, H2/H3, SOCKS, or forward-proxy body handling.

In those cases, retaining a small duplicated CONNECT implementation is preferable to creating a structurally incorrect shared abstraction.
