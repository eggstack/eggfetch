# HTTPX Parity Phase 03 — Request/Response Extension Surface

Status: **Partially implemented** — target and wire reason_phrase implemented; sni_hostname, trace, stream_id deferred.
Depends on: Phase 02 should land first so protocol policy is stable.
Does not include: `network_stream` raw-I/O ownership; that is Phase 04.

## Objective

Implement the bounded HTTPX 0.28.1/httpcore 1.0.9 extension semantics that can be represented without exposing a raw connection handle:

- request `target`;
- request `sni_hostname`;
- request `trace`;
- existing request `timeout` preservation/regression coverage;
- response `http_version` and `reason_phrase` preservation;
- response `stream_id` when a reliable native source exists.

The compatibility facade may continue to expose an untyped Python `extensions` dictionary, but `eggfetch-core` must receive typed transport hints and typed response metadata, not arbitrary Python objects.

## Reference behavior

HTTPX forwards `Request.extensions` unchanged into httpcore transports.

httpcore 1.0.9 behavior relevant to this phase:

### `target`

At request construction, an extension named `target` replaces only the URL target used on the wire. It does not change the connection origin. This supports cases such as:

- `OPTIONS *`;
- absolute-form request targets;
- caller-controlled escaping that cannot be represented by normal URL parsing.

### `sni_hostname`

During TLS connection establishment, `sni_hostname` overrides the hostname passed to TLS while TCP still connects to the URL/origin host. This is used for “connect to an IP, verify/SNI as a DNS name” workflows.

### `trace`

The trace extension is a callback that receives event names and info dictionaries around httpcore operations. Event names are scoped by subsystem such as `connection.*`, `http11.*`, `http2.*`, and proxy-related operations. The event vocabulary is tied to the pinned httpcore version and should not be treated as a forever-stable API.

### Response metadata

- `http_version`: bytes in the reference transport extension;
- `reason_phrase`: bytes for HTTP/1.x where present;
- `stream_id`: integer for HTTP/2 responses;
- `network_stream`: deferred to Phase 04.

## Current EggFetch state

The compatibility facade already stores request extension dictionaries and explicitly interprets timeout mappings. It also synthesizes/overlays response HTTP version and reason phrase.

The native core request model has no general extension/transport-hint field. `RequestParts` currently carries method, URL, headers, body, version, timeout, redirect, auth, decompression, proxy, and retry state.

The direct Hyper send path converts `hyper::Response` into EggFetch `Response` while keeping only status/version/headers/body/URL. Connection-level metadata is not retained.

## Design rule: typed core metadata

Add a narrow internal structure instead of a generic map. Suggested shape:

```text
RequestTransportHints {
    target: Option<Bytes>,
    sni_hostname: Option<String>,
    trace: Option<Arc<dyn TraceObserver>>,
}
```

Names are illustrative; choose idiomatic repository naming.

Response transport metadata should likewise be typed, for example:

```text
ResponseTransportMetadata {
    http_version: ...,
    reason_phrase: Option<Bytes>,
    stream_id: Option<u32>,
    connection_metadata: ... // reserved for Phase 04, if useful
}
```

Do not expose these internal structs as a promise to native Rust consumers unless there is a separate API rationale.

## Required implementation tracks

### Track 1 — Carry transport hints through `Request`

Extend:

- `Request`;
- `RequestParts`;
- `RequestBuilder` if useful internally;
- redirect/retry reconstruction paths;
- Python-native request dispatch conversion.

Rules:

- timeout remains in the existing timeout model; do not duplicate it into a generic hint map;
- `target` and `sni_hostname` must survive one-hop dispatch;
- redirect construction should follow HTTPX differential behavior rather than blindly carrying all hints. Add explicit tests for whether a redirect request retains or clears each extension;
- retries of the same logical request should retain transport hints when HTTPX does.

### Track 2 — Implement `target` without changing logical URL state

The compatibility conversion should accept the same effective type/value forms as HTTPX/httpcore 1.0.9 and reject invalid forms at the same stage where practical.

Core behavior:

- connection routing, Host header defaults, cookies, auth-origin comparisons, redirects, and proxy selection continue to use the logical URL;
- only the wire request target is overridden;
- the raw target must not be round-tripped through `url::Url`, because that would defeat the point of the extension;
- construct the Hyper request URI in a way that preserves valid asterisk-form, origin-form, and absolute-form targets;
- reject CR/LF and any target form that the reference rejects;
- do not allow the target override to smuggle a second set of authority headers into proxy/origin routing logic.

Differential wire tests must capture the exact first request line for HTTP/1.1 and the exact `:path`/request target meaning for HTTP/2.

### Track 3 — Implement `sni_hostname` at the connector boundary

This is per-request TLS metadata, not an HTTP header.

Requirements:

- TCP destination remains the URL host/IP;
- Host header remains controlled independently by request headers;
- TLS ServerName/SNI and certificate-name verification use the override;
- invalid SNI values fail before or during connect with a deterministic mapped exception;
- no override is sent to plaintext HTTP.

Because Hyper connectors ordinarily receive only the URI, do not fake this by changing the request URL host. That would alter TCP routing, cookies, auth, and proxy behavior.

Preferred implementation approach:

- extend the existing custom direct connector so it can establish TLS with an explicit server name while connecting to the original endpoint;
- select/cache a connector/client by `(origin transport configuration, sni_hostname)` when an override is present;
- keep ordinary no-override requests on the current fast path;
- include the SNI override in any connection-reuse key required to prevent a connection authenticated under one name from being reused for an incompatible override.

Differentially check HTTPX’s same-origin/different-SNI reuse behavior, but prefer security-correct isolation if the reference’s pool key is less strict. Any intentional connection-count difference must be documented rather than altering request semantics.

### Track 4 — Introduce a core trace observer abstraction

Do not store `PyObject` inside `eggfetch-core`.

Define a small observer trait/callback type in core that receives:

- a stable internal event enum;
- an info value composed only of native scalar/byte/address metadata;
- start/complete/failure state;
- optional error classification or return metadata.

The Python binding may implement/capture this observer and acquire the GIL only at callback delivery points.

Critical rules:

- never hold the GIL across DNS/TCP/TLS/body waits;
- callback exceptions must propagate in the same operation stage HTTPX would abort, and resources must be closed/released;
- async callbacks must match HTTPX behavior for `AsyncClient`; determine from the pinned reference whether the supplied trace callable is awaited or must itself be sync at the httpcore layer and test accordingly;
- event info must redact proxy credentials and authorization data.

### Track 5 — Pin the HTTPX/httpcore trace vocabulary

Build a reference-derived table from httpcore 1.0.9 tests/source rather than inventing names.

Cover at least the paths EggFetch owns:

- TCP connect started/complete/failed;
- Unix connect where applicable;
- TLS start started/complete/failed;
- retry delay if the reference emits it for the selected transport retry configuration;
- HTTP/1.1 send headers/body, receive headers/body, response closed;
- HTTP/2 connection init, send headers/body, receive headers/body, remote settings, response closed;
- proxy connect/TLS events as applicable.

Map internal EggFetch events to the pinned names at the Python compatibility boundary. Native Rust tracing logs are a separate feature and must not be conflated with HTTPX’s callback extension.

### Track 6 — Preserve `http_version` and `reason_phrase` as actual transport metadata

Do not regress current behavior.

Where possible, preserve the original bytes/value returned by the protocol layer instead of reconstructing only from status/version enums. HTTP/2 has no wire reason phrase; keep reference fallback behavior at the facade.

Add tests for custom HTTP/1.1 reason phrases and HTTP/1.0 where local fixtures can produce them.

### Track 7 — Investigate and implement HTTP/2 `stream_id`

First inspect the current Hyper response extensions and public APIs for a reliable stream ID source.

Acceptable implementation:

- obtain a typed stream ID from a public Hyper/h2 integration seam and store it in response metadata.

Unacceptable implementation:

- parse debug strings;
- infer IDs from request ordering;
- maintain a fake client-side counter disconnected from actual H2 stream assignment.

If the current Hyper abstraction does not expose the ID, record this as a narrow residual difference and defer any custom H2 stack refactor. Do not replace Hyper merely to close `stream_id` without a separate decision.

## Differential test plan

Create dedicated tests such as:

- `test_extension_target_differential.py`;
- `test_extension_sni_differential.py`;
- `test_extension_trace_differential.py`;
- `test_extension_response_metadata_differential.py`.

### `target`

Cases:

- `OPTIONS *`;
- normal origin-form override;
- absolute URI target;
- percent-escape preservation;
- bytes vs ASCII string input where supported;
- malformed/non-ASCII string;
- redirect behavior;
- HTTP/2 path behavior.

### `sni_hostname`

Use a local TLS fixture whose certificate DNS name differs from the TCP address.

Cases:

- connect to loopback IP with Host + SNI DNS override succeeds;
- same connection without override fails hostname verification;
- wrong override fails;
- HTTP plaintext ignores/does not misuse SNI;
- sync and async;
- direct and relevant proxy tunnel path if reference supports it.

### `trace`

Capture event sequence for small deterministic requests:

- one new HTTP/1.1 TLS connection;
- one reused HTTP/1.1 connection;
- HTTP/2 request;
- connect failure;
- read failure;
- user callback raising;
- client close.

Compare event names, ordering constraints, and required info keys. Do not overfit reprs of internal stream objects.

### `stream_id`

If implementable, make two or more H2 requests and compare the actual extension type/presence and stream IDs to reference behavior. Exact numeric equality across separate reference/candidate connections is not required; protocol validity and per-stream uniqueness/odd client-initiated IDs are.

## Security requirements

- target override cannot alter destination routing implicitly;
- no CRLF/request-smuggling acceptance beyond what the underlying HTTP library safely permits;
- SNI override must participate in safe connection reuse decisions;
- trace info must redact secrets;
- trace callbacks must not run while internal locks are held in a way that permits deadlock through re-entrant requests;
- callback failure must release pool permits and close unusable streams;
- no fake `stream_id` data.

## Non-goals

- generic Python extension execution inside core;
- `network_stream` raw read/write;
- new tracing/logging backend;
- replacing Hyper/H2;
- HTTP/3-specific extension parity.

## Acceptance criteria

This phase is complete when:

1. `target` matches HTTPX 0.28.1 for the differential wire cases and does not change logical origin semantics.
2. `sni_hostname` controls TLS SNI/name verification while preserving the actual TCP destination.
3. `trace` callbacks receive the pinned event vocabulary for the implemented transport paths with correct ordering and failure propagation.
4. Python callback delivery does not keep the GIL held during network waits.
5. Current timeout extension semantics remain unchanged and regression tests pass.
6. `http_version` and `reason_phrase` behavior remains reference-compatible.
7. `stream_id` is either implemented from a reliable public source or retained as a precisely documented/tested residual difference; no guessed value is allowed.
8. Sync and async focused differential suites pass.
9. Active compatibility ledgers are updated for resolved extension differences.
10. `./scripts/check.sh` passes.
