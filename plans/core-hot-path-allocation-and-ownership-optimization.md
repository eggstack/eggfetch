# Core Hot-Path Allocation and Ownership Optimization

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Parent program: plans/performance-optimization-no-api-regression-program.md
Prerequisite: plans/performance-benchmark-baseline-and-guardrails.md

## Objective

Remove objectively redundant allocation/copy/bookkeeping work from the Rust core and thin native adapters while preserving the complete public API and all request/response semantics.

Prioritize changes that reduce work on every ordinary request. Keep each optimization independently testable and benchmarkable so a regression can be reverted without undoing unrelated improvements.

## Part A — logical pool fast paths

### A1. Do not construct an origin key unless admission needs one

Today prepare_single_request constructs OriginKey before Pool::acquire even when PoolConfig has no per-origin limit. The native frame path does the same.

Add a crate-private Pool query/helper that answers whether per-origin admission is configured. Construct OriginKey only when that policy needs it.

Global-only admission must not require an origin key.

Preserve:

- max_connections/max_in_flight_requests alias precedence;
- per-origin alias precedence;
- proxy-origin/tunnel key semantics when per-origin admission is enabled;
- PoolMetrics meanings;
- timeout/cancellation classification;
- test-util origin diagnostics where they are actually meaningful.

Do not remove origin information from a guard that holds a real per-origin permit if existing diagnostics/tests rely on it.

### A2. Avoid cloning/storing origin in inert/global-only guards unless needed

Pool::acquire currently clones origin into every returned PoolGuard.

Make guard origin retention conditional on actual use/diagnostic need. A default unlimited guard should not own copied scheme/host/proxy Strings solely because a request passed through acquisition.

Any public or test-util host/origin observation must retain established behavior in configured cases. If changing test-only diagnostic behavior would complicate the fast path, keep a narrow internal representation rather than exposing a public change.

### A3. Skip Arc allocation for an inert response lease

finalize::apply_timeouts_and_lease currently wraps every PoolGuard in Arc and attaches it to streaming bodies.

Add a private predicate equivalent to PoolGuard::has_response_state that is true when the guard carries at least one of:

- global permit;
- per-origin permit;
- response read timeout;
- response total deadline;
- any other lifecycle state that must remain alive through response consumption.

If false after timeout metadata is applied, drop the inert guard and leave the body lease-free.

The public ResponseBody variant shapes must remain byte-for-byte source compatible; no new public fields/variants and no non_exhaustive change.

Tests must cover default/no-timeout, global-only, per-origin, read timeout, total timeout, cancellation, body drop, EOF, error, raw stream, decoded stream, and native frame paths.

## Part B — header and URI ownership

### B1. Move Hyper response headers instead of cloning

finish_hyper_response currently clones hyper_response.headers().

Rework the private conversion so the HeaderMap is moved out of the Hyper response while preserving the exact 101 upgrade sequence. Capturing hyper::upgrade::on must still occur at the correct time and non-upgrade bodies must continue to own Incoming correctly.

Acceptable approaches include taking the header map from the mutable response before body consumption or splitting response parts after upgrade state is captured.

Required tests:

- duplicate/multi-value headers;
- Set-Cookie;
- obs-text behavior already supported by the transport;
- H1/H2 ordinary response;
- 101 upgrade;
- trailers;
- wire Content-Encoding/Content-Length snapshots.

### B2. Move request header maps where ownership permits

Audit build_http_request/build_hyper_request and the native execute_http_body path.

Where a prepared request already owns a HeaderMap and no later stage requires the old container, move the map into http::Request rather than re-inserting every pair into a new map. Do not contort shared route helpers into unsafe ownership or duplicate route-specific implementations merely to avoid a small copy.

Preserve exact duplicate-header and replacement semantics.

### B3. Make trace-only URI formatting lazy

Change the private trace send helper so request.uri() is formatted only when a trace observer is present. Ordinary no-trace requests should allocate no target String for tracing.

Trace event payloads and callback abort behavior must remain identical.

### B4. Remove temporary URI String in request-size validation

Request-size validation currently calls request_uri.to_string().as_bytes().

Introduce a private exact serialized-length/counting path that does not allocate a String. The result must match http::Uri Display serialization for all supported target forms, including absolute URI, origin form, target override, and OPTIONS *.

Do not hand-roll a subtly different URI serializer. Prefer a fmt::Write counting sink or another exact private representation.

Add differential tests comparing the new count/path against the existing to_string representation over representative and property-generated URIs before deleting the old allocation.

## Part C — buffer ownership and capacity

### C1. Share resolved address snapshots

DirectConnector::with_resolved_target currently materializes target.addresses().to_vec().into() even though ResolvedTarget already exposes crate-private addresses_shared().

Use the shared Arc slice when the attempt order and immutability semantics are identical. Audit SOCKS/proxy route wrappers for the same pattern, but only change locations where ownership lifetime is safe and route-key ordering remains exact.

Do not sort/deduplicate caller-supplied addresses.

### C2. Avoid multipart header copy

MultipartEncoder owns its generated part header as Bytes but copies the remaining slice before yielding it.

Use Bytes ownership/slicing so yielding a header is O(1) on the Rust side. Preserve exact emitted bytes and chunk ordering. This is a small optimization; keep the patch correspondingly small.

### C3. Capacity-hint response collection only when trustworthy

ResponseBody::bytes begins streaming collection with an empty BytesMut.

If an internal, trustworthy decoded-length hint can be supplied without changing public ResponseBody layout, reserve a bounded initial capacity for unencoded bodies with valid wire Content-Length. Do not use encoded wire length as decoded-size truth after decompression. Do not reserve unbounded attacker-supplied sizes.

If plumbing the hint requires expanding the frozen public ResponseBody variants or duplicating timeout/decode state, do not implement this subpart. Record it as rejected.

A reservation cap is required to avoid a malicious Content-Length forcing a huge eager allocation.

## Part D — FFI copy removal

ResponseHandle is private to eggfetch-ffi and currently stores Vec<u8>; eggfetch_client_send converts core Bytes with to_vec before the C ABI later allocates/copies caller-owned bytes.

Change the private handle to retain Bytes directly if this does not alter the ABI. eggfetch_response_body must continue returning a separately allocated caller-owned buffer freed by eggfetch_body_free exactly as documented.

C ABI function names, signatures, error codes, ownership rules, and body bytes remain unchanged.

Node's prototype may continue to incur its own C-ABI copy; do not bypass the stable FFI layer in this plan.

## Part E — measurement-gated connector/TLS residual work

After Parts A-D, rerun the route cache hit/miss construction benchmarks.

Potential residual work:

- reuse immutable DirectConnector/TlsConnector state when creating SNI/resolved variants rather than rebuilding Rustls configuration;
- cache already-built proxy TLS connector/configuration inside reusable proxy connector state if physical connection churn makes build cost material;
- shorten cache lock scope around route-client construction only if benchmark contention is observable.

Do not implement speculative cache sharding, a new TLS cache, or a new dependency without measured evidence.

Recent route-cache work already amortizes client construction; it is acceptable and preferred to close Part E as “measured, no material benefit” rather than add complexity.

Any TLS caching change must preserve complete connection-policy identity. False reuse across distinct verification, trust root, client identity, ALPN/version, SNI, proxy, or route policy is a correctness/security regression.

## Performance acceptance

Compare against the baseline plan using identical conditions.

Expected evidence should show directionally:

- lower allocations for default request/pool path;
- no inert lease heap allocation on default streaming response;
- lower response-header copy cost as header count grows;
- no trace-target String allocation on trace=None;
- lower FFI transient memory for large buffered responses;
- no regression in configured pool/timeout cases.

Do not require a universal percentage threshold. A micro-optimization may land when it removes objectively redundant work and does not regress representative throughput, but measurement must still be recorded.

## Validation and compatibility

Run focused tests for each subpart, then Tier 1.

Before handing off to final closure, also run relevant feature slices:

- no-default-features;
- http1;
- http1,tls-rustls;
- http1,tls-rustls,proxy;
- all-features;
- FFI tests.

No public API manifest, feature/default definition, error type, timeout classification, or ResponseBody public shape may change.

## Exit criteria

- [ ] OriginKey is not built for default/global-only admission unless genuinely required.
- [ ] Default no-timeout/no-permit response bodies do not allocate an inert lease Arc.
- [ ] Configured pool permits/timeouts still live exactly through response EOF/error/drop.
- [ ] Hyper response headers are moved rather than cloned where safe.
- [ ] No-trace dispatch does not stringify URI for tracing.
- [ ] Request-size validation avoids a temporary String with differential proof.
- [ ] Resolved-address and multipart ownership copies are removed where safe.
- [ ] FFI buffered responses avoid the intermediate core Bytes-to-Vec copy.
- [ ] Connector/TLS residual work is either evidence-backed or explicitly rejected.
- [ ] Tier 1 and focused feature/FFI tests are green.
- [ ] No public API or semantic regression.

## Implementation record

Implemented on the campaign branch:

- per-origin keys are constructed only when per-origin logical admission is configured;
- default/global-only guards retain no copied origin and default response finalization skips
  the inert lease allocation;
- Hyper response headers are moved out of response parts, trace URI formatting is lazy,
  and request-size validation counts Uri display output without allocating a String;
- resolved address snapshots, multipart headers, native owned header maps, and FFI buffered
  bodies use the existing Arc/Bytes ownership where safe;
- native response bodies also omit an inert lease while preserving timeout/permit lifetimes.

Focused pool/header/cookie/FFI tests passed after these changes. Connector/TLS residual
optimization was measured as out of scope for this implementation slice; no new route
cache or TLS state was added.
