# CONNECT Wire Overlap Conformance

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Prerequisite: plans/core-private-module-decomposition.md
Relevant crates: eggfetch-core, eggfetch-http-connect

## Objective

Prevent the remaining CONNECT-related overlap from drifting while preserving every existing public API and validation behavior.

Production CONNECT wire ownership is already correctly split:

- eggfetch-http-connect owns ConnectTarget, request serialization, Basic-auth wire helper, and bounded response-head parsing over a caller-owned stream;
- eggfetch-core owns proxy selection, dialing, TLS, deadlines, accepted status policy, rejection handling, route identity, metrics, pooling, and origin TLS.

The residual concern is not a second production CONNECT implementation. It is duplicated or adjacent testing/fuzzing/auth logic that could diverge from the production wire primitive.

This plan should prefer conformance proof over forced deduplication when cross-crate visibility or intentionally different acceptance domains make code sharing unsafe.

## Part A — inventory overlapping contracts precisely

Document the exact overlap among:

- eggfetch_http_connect::ConnectTarget authority formatting;
- eggfetch_http_connect::encode_connect_request;
- eggfetch_http_connect::read_connect_response_head;
- eggfetch_http_connect::basic_auth_value;
- eggfetch_core::proxy::ProxyAuth::basic and its private header_value;
- eggfetch_core::proxy::parse_proxy_response_bytes, which is doc-hidden and intended for testing/fuzzing;
- production eggfetch_core::transport::connect CONNECT handling.

For every pair that appears duplicative, record:

- accepted input domain;
- rejected input domain;
- error taxonomy;
- size/count bounds;
- whether user-controlled bytes can enter errors;
- sync versus async constraints;
- whether the symbol is public/stable, doc-hidden, or private.

Do not assume Basic-auth validation domains are identical. The planning audit observed that eggfetch-http-connect rejects a broader set of control characters than ProxyAuth::basic's explicit constructor checks. Determine effective end-to-end behavior before changing either.

## Part B — make response-parser bounds share the same values where possible

The production parser's limits are represented by ConnectResponseLimits. The hidden core byte parser currently carries local constants for status-line length, header count, line length, and total header bytes.

If ConnectResponseLimits exposes these values without an API change, make the hidden parser derive its bounds from ConnectResponseLimits::default rather than maintaining independent numeric constants.

If doing so is not possible without adding/changing a public eggfetch-http-connect API, retain the constants and add an explicit conformance assertion that their values match production defaults.

Do not add a new public getter/helper merely for this cleanup.

## Part C — add differential parser fixtures

Create a shared set of deterministic CONNECT response fixtures that both the production async parser and the hidden byte parser consume.

Cover at least:

Valid:
- HTTP/1.1 200 with no headers;
- reason phrase present/absent;
- mixed-case header names;
- OWS around values;
- multiple ordinary headers;
- read-ahead bytes immediately following the head;
- representative non-200 status heads accepted syntactically.

Invalid/bounded:
- truncated status line;
- malformed status;
- invalid UTF-8/header name where applicable;
- header without colon;
- status line at and over limit;
- header line at and over limit;
- header count at and over limit;
- total header bytes at and over limit;
- unexpected EOF;
- unbounded/hostile bytes do not get echoed in errors.

The test should compare semantic parse results and error classes at the overlap boundary, not necessarily require identical display strings when the two APIs intentionally map errors differently.

Where the hidden parser is only a fuzz adapter, prefer making the fuzz corpus target production grammar invariants rather than maintaining a second independently evolving grammar expectation.

## Part D — authority-format conformance

Retain tests proving the shared ConnectTarget primitive owns:

- DNS host:port;
- IPv4 host:port;
- IPv6 bracket formatting;
- rejection of invalid/unsafe host input;
- no authority ambiguity.

Core production CONNECT must continue to construct ConnectTarget rather than formatting authority independently.

Search for any remaining manual CONNECT authority formatting in eggfetch-core and remove it only when the shared primitive can replace it without behavior change.

## Part E — auth behavior: consolidate only the proven common subset

First prove the effective ProxyAuth contract with focused tests for:

- colon in username;
- CR;
- LF;
- NUL;
- TAB and other ASCII control characters;
- DEL;
- non-ASCII username/password;
- empty username/password;
- redacted Debug/Display/errors;
- exact Basic base64 output for inputs accepted by both layers.

Then choose one of two valid outcomes:

### Outcome 1 — safe consolidation

If the effective accepted input domain is already identical, delegate the shared encoding/validation work to eggfetch-http-connect::basic_auth_value through existing public API and preserve eggfetch-core error mapping exactly.

### Outcome 2 — intentional retention

If acceptance differs, keep ProxyAuth validation/encoding separate, document the distinction in an internal comment, and add a conformance test for the common accepted subset.

Do not make ProxyAuth more restrictive or permissive under this maintenance campaign.

## Part F — production ownership assertion

Add/retain tests or source-level validation proving:

- transport/connect.rs uses encode_connect_request and read_connect_response_head;
- no second production response parser exists in transport code;
- eggfetch-http-connect does not dial sockets or own TLS/deadlines/retries;
- core maps ConnectError into its existing public Error taxonomy without exposing proxy-controlled bytes;
- non-200 CONNECT status policy remains in core, not in eggfetch-http-connect.

This may be a small repository checker if source ownership cannot be expressed through ordinary tests, but avoid brittle whole-file grep when a compile/test boundary is sufficient.

## Part G — fuzz/test ownership cleanup

Inspect fuzz targets and hidden parser callers.

If parse_proxy_response_bytes exists solely because the fuzz harness cannot conveniently drive the async production parser, retain it as a compatibility/testing adapter but make the differential tests mandatory.

If the fuzz harness can be switched to eggfetch-http-connect directly without removing/changing the hidden core symbol, do so and keep parse_proxy_response_bytes as a tested compatibility shim.

Do not remove the doc-hidden symbol during this API-frozen campaign.

## Focused validation

Required:

- cargo test -p eggfetch-http-connect --all-features -- --test-threads=1;
- cargo test -p eggfetch-core --all-features connect -- --test-threads=1;
- cargo test -p eggfetch-core --all-features proxy -- --test-threads=1;
- fuzz/unit seed checks where repository tooling permits;
- the Rust public-surface oracle;
- ./scripts/check.sh.

Run extended validation if any production core or http-connect implementation changes.

## Acceptance criteria

- [ ] Production CONNECT request/response wire grammar has one implementation owner: eggfetch-http-connect.
- [ ] Core CONNECT policy remains in eggfetch-core.
- [ ] Hidden byte-parser limits cannot silently diverge from production defaults.
- [ ] Differential fixtures cover valid, malformed, truncated, bounded, and read-ahead cases.
- [ ] Authority formatting remains exclusively delegated to ConnectTarget in production.
- [ ] ProxyAuth behavior is unchanged for every tested boundary input.
- [ ] Auth duplication is removed only if the accepted domain is proven identical; otherwise it is explicitly retained and covered.
- [ ] No new public cross-crate helper/API is introduced.
- [ ] Error redaction/bounding remains intact.
- [ ] Rust exact public surface remains unchanged.

## Stop conditions

Do not pursue deduplication if it requires:

- a new public eggfetch-http-connect helper solely for core internals;
- changing ProxyAuth accepted/rejected credentials;
- changing public error variants/messages relied upon by tests;
- moving proxy policy into the wire crate;
- adding synchronous runtime blocking solely to call the async production parser.

Conformance coverage is a successful outcome when safe code sharing is not possible under the API freeze.
