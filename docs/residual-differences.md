# Residual Differences from HTTPX 0.28.1

Known intentional differences where EggFetch cannot match HTTPX/httpcore behavior due to underlying transport constraints. Each entry below describes what HTTPX does, what EggFetch does, why the gap exists, the affected surface, and the differential tests that pin the behavior.

These are the *only* places where EggFetch intentionally diverges from HTTPX. The compatibility profile in `compat/httpx/0.28.1/parity-cases.toml` classifies these rows as `bounded-difference`; every other case is `parity`.

## 1. `stream_id` — HTTP/2 response stream identifier (metadata-only)

**Status**: Residual difference (no implementation path without Hyper stack replacement).

**What HTTPX does**: `Response.extensions["stream_id"]` is an integer for HTTP/2 responses, derived from the h2 stream identifier assigned by the connection.

**What EggFetch does**: `stream_id` is not available on `Response.extensions`. The legacy hyper-util client erases the underlying h2 future.

**Why**: Hyper 1.10.1 consumes `h2::client::ResponseFuture` without extracting the stream ID, and `hyper::body::Incoming` wraps `h2::RecvStream` privately. `hyper_util::client::legacy::ResponseFuture` returns `Response<hyper::body::Incoming>` directly — there is no extension point carrying the h2 stream identifier. The `h2 0.4.15` crate exposes `StreamId` on `ResponseFuture::stream_id()` and `RecvStream::stream_id()`, but those are not reachable through hyper-util.

**Affected surface**: Python `Response.extensions` (the `stream_id` key is absent for H2 responses).

**Impact**: Narrow metadata-only difference. Does not affect request/response semantics, retry, redirect, or streaming behavior. Callers that need the stream ID for tracing or correlation can fall back to logging the connection metadata exposed via `network_stream`.

**Resolution path**: Would require either (a) upstream hyper/hyper-util to expose the stream ID via a response extension, or (b) replacing the Hyper client with direct h2 usage. Neither is warranted for a single metadata field.

**Differential tests**:
- `crates/eggfetch-python/tests/compat/test_h2_differential.py::TestStreamIdAbsence::test_stream_id_absent_in_response_extensions`
- `compat/httpx/0.28.1/parity-cases.toml` → `H2-008`

## 2. Proxy CONNECT origin request framing (protocol)

**Status**: Residual difference (CONNECT tunnel is HTTP/1.1 only).

**What HTTPX does**: When a request is routed through an HTTP proxy with a TLS origin, HTTPX can use HTTP/2 on the origin connection if the origin negotiates h2 via ALPN over the established tunnel.

**What EggFetch does**: The proxy CONNECT tunnel is always HTTP/1.1. After the tunnel is established and origin TLS is performed, the origin request is written as HTTP/1.1 (`crates/eggfetch-core/src/transport/connect.rs` and `crates/eggfetch-core/src/transport/proxy.rs` hardcode `HTTP/1.1` in the wire format). The Hyper `h2` builder is not used inside the CONNECT tunnel path.

**Why**: EggFetch's proxy path performs the CONNECT handshake and the origin request through a hand-rolled socket reader/writer rather than through the hyper-util legacy client. Adding h2 to this path would require folding the post-CONNECT socket into a hyper h2 connection and is intentionally deferred.

**Affected surface**: H2-only requests routed through an HTTP proxy (TLS origin) always use HTTP/1.1 to the origin.

**Impact**: Tunnels that have h2-capable origin servers report `http_version == "HTTP/1.1"` even when the client policy is H2-only. The user-facing `http2=True` flag is honored for direct connections and for origins reached without a proxy.

**Resolution path**: Route the post-CONNECT socket through a hyper h2 connection when the origin TLS selected h2. The plumbing exists (`tokio_rustls::client::TlsStream` exposes ALPN via `get_ref().1.alpn_protocol()`); the rewrite is scoped but non-trivial.

**Differential tests**:
- `compat/httpx/0.28.1/parity-cases.toml` → bounded-difference row tracking proxy + H2 origin behavior.

## 3. H2-on-direct-connector for `verify=False` (specialized transport)

**Status**: Residual difference scoped to one intersection of options.

**What HTTPX does**: HTTPX supports `http2=True, local_address=..., verify=False, socket_options=...` for an H2-capable direct connector.

**What EggFetch does**: When `verify=False` is combined with `local_address` or `socket_options` (forcing the direct connector path), the direct connector does not currently wire the verify-disabled TLS configuration into its hand-rolled TLS handshake. The h2 connection itself works (the direct connector supports h2 for the verify-enabled case via the H2 enforcement added in corrective 04); the gap is the verify-disabled intersection only.

**Why**: The direct connector currently calls `TlsConfig::build_rustls_config()` which always uses the configured trust store. Mapping `verify=False` into the same `NoVerifier` rustls config that the standard path uses is a focused addition that has not yet been made.

**Affected surface**: `HTTPTransport(http2=True, local_address=..., verify=False, socket_options=...)`.

**Impact**: TLS verification falls through to the default trust store rather than disabling verification. H2 capability itself is preserved.

**Resolution path**: Thread a `verify_certificate: false` flag into the direct connector's TLS builder so it produces a `NoVerifier` config when the user requested `verify=False`.

**Differential tests**:
- `compat/httpx/0.28.1/parity-cases.toml` → tracked under specialized-route bounded differences.

## 4. UDS + H2 + custom ALPN (specialized transport)

**Status**: Residual difference scoped to a narrow intersection.

**What HTTPX does**: HTTPX supports HTTP/2 over Unix domain sockets with TLS where the server negotiates h2 via ALPN.

**What EggFetch does**: The UDS connector supports h2 via the legacy `http2_only` setting and ALPN signaling (added in corrective 04), but only on the default TLS path. Custom ALPN protocols for UDS are not yet a public surface; if a caller restricts the server to a non-`h2` ALPN, the connection will fail. This is by design: UDS over TLS with non-h2 ALPN is not a documented HTTPX feature on the public compatibility surface either.

**Why**: Out of the public compatibility surface for HTTPX 0.28.1.

**Affected surface**: HTTPS-over-UDS where the caller manipulates ALPN protocols away from the defaults.

**Impact**: Negligible. Standard UDS + h2 paths work.

**Resolution path**: None planned. Not in the public compatibility surface.

**Differential tests**:
- `compat/httpx/0.28.1/parity-cases.toml` → UDS bounded-difference row.

## 5. Server-pushed HTTP/2 streams (server-side feature)

**Status**: Residual difference (server-only feature, out of client scope).

**What HTTPX does**: HTTPX honors HTTP/2 server push if the server uses it.

**What EggFetch does**: EggFetch does not implement server push handling. The hyper h2 builder is configured without a `server_push` option.

**Why**: HTTP/2 server push is a deprecated, widely-unimplemented feature. The HTTPX implementation accepts but discards pushed streams; EggFetch's omission is consistent with the broader ecosystem's deprecation.

**Affected surface**: Hypothetical servers using HTTP/2 server push. Not exercised by the differential tests.

**Impact**: None in practice.

**Resolution path**: None planned.

**Differential tests**: None.

---

# Previously bounded differences now closed

The following bounded differences from `parity-cases.toml` are closed as of corrective 04 (this phase) and are documented here for historical context only. The current behavior is full parity.

- **H2-002 (TLS H2-only enforcement)**: The candidate previously fell back to HTTP/1.1 when an H2-only client reached an H1-only TLS server. Closed by setting `http2_only(true)` on the hyper-util legacy client; the candidate now fails with a `RequestError` / `ConnectError` matching the reference behavior.
- **H2-003 (Cleartext H2 prior knowledge)**: The candidate previously sent HTTP/1.1 to cleartext H2 servers. Closed by the same `http2_only(true)` change; the candidate now sends the H2 client preface directly over plain TCP.
- **H2-007 (Direct connector H2)**: The candidate previously fell back to HTTP/1.1 when `local_address` or `socket_options` were set. Closed by `http2_only(true)` on the direct connector's hyper-util client and ALPN signaling in `DirectStream::connected`; the candidate now supports h2 over the direct connector and enforces H2-only against H1-only servers.
