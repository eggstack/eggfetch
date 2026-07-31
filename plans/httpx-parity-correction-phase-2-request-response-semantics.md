# HTTPX Parity Correction Phase 2 — Request and Response Semantics

Status: implemented

Depends on:

- `plans/httpx-parity-correction-roadmap.md`
- `plans/httpx-parity-correction-phase-1-entrypoints-client-lifecycle.md`

## Objective

Correct the public `Request` and `Response` object behavior exposed by `eggfetch.compat.httpx` so that supported construction, body encoding, metadata, error, streaming-state, and redirect-state semantics match HTTPX 0.28.1.

This phase must treat object behavior as an executable contract. Matching attribute names while returning different state or exception types is not sufficient.

## Audited files

Review at minimum:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_stream.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- native response and streaming Python bindings
- request-body conversion and multipart bindings
- existing compatibility tests for models, streaming, encoding, redirects, and exception hierarchy

## Scope constraints

This phase may:

- refactor compatibility Request and Response internals;
- add small content encoders or reuse existing compatibility encoders;
- expose missing native timing or metadata fields through PyO3;
- correct stream wrapper behavior and state flags;
- add focused differential tests.

This phase must not:

- implement the full redirect/auth/cookie loop from Phase 4;
- add new compression algorithms;
- add a second network implementation;
- add a new validation workflow or broad test framework;
- preserve an incorrect object behavior merely because current tests encode it.

# Track 1 — Correct Request URL and body construction

## 1.1 Apply `params` to the Request URL

Direct construction must match HTTPX:

```python
request = Request("GET", "https://example.test/path?existing=1", params=[("a", "1"), ("a", "2")])
```

Required behavior:

- request parameters appear in `request.url`;
- repeated values are preserved;
- ordering follows the pinned reference;
- existing query behavior matches HTTPX’s replacement/merge rule for direct construction;
- `request.params`, if retained as an eggfetch extension, cannot disagree with `request.url.params`.

Prefer a single canonical URL/query representation rather than maintaining divergent URL and params fields.

## 1.2 Replace body-source mutual exclusion with HTTPX encoding rules

Match HTTPX’s supported body combinations:

- `content` is exclusive with `data`, `files`, and `json`;
- `json` is exclusive with `content`, `data`, and `files`;
- `data` plus `files` is valid and produces multipart form data;
- `data` alone produces URL-encoded form data when mapping-like;
- raw non-mapping `data` follows the pinned HTTPX deprecation behavior or a documented bounded difference;
- `stream=` remains a low-level alternative to encoded body arguments.

Do not retain a blanket “only one of data or files” error.

## 1.3 Use HTTPX-compatible JSON serialization

Match the pinned reference’s compact JSON serialization and generated headers, including:

- separators/whitespace;
- UTF-8 encoding;
- `Content-Type`;
- `Content-Length`;
- caller-supplied header precedence.

Use a deterministic test containing Unicode, nested structures, booleans, and null values.

## 1.4 Preserve multipart form and file metadata

Support the public HTTPX file tuple forms needed by the current compatibility target:

- file object or bytes;
- `(filename, fileobj)`;
- `(filename, fileobj, content_type)`;
- `(filename, fileobj, content_type, headers)` where supported by the pinned reference.

Requirements:

- combine `data` fields and `files` in one multipart body;
- preserve repeated form and file fields;
- quote field names and filenames safely;
- respect a caller-provided multipart boundary in `Content-Type` where HTTPX does;
- avoid reading a large file merely to construct a Request when the input is streaming.

Do not duplicate the native multipart encoder if it can be adapted without losing HTTPX semantics.

### Track 1 acceptance criteria

- [x] Direct `Request(params=...)` updates the URL correctly.
- [x] Repeated query values survive construction.
- [x] `data` plus `files` is accepted and encoded as multipart.
- [x] Invalid body combinations raise the same error class and equivalent message category as HTTPX.
- [x] JSON bytes and generated headers match the pinned reference for representative payloads.
- [x] Multipart metadata and repeated fields are preserved.
- [x] Large streaming file inputs are not eagerly buffered solely by Request construction.

# Track 2 — Match request auto-header and stream semantics

## 2.1 Distinguish `content=` from explicit `stream=`

HTTPX intentionally treats these differently:

- encoded `content=` may add Host, Content-Length, Content-Type, or Transfer-Encoding as appropriate;
- explicit low-level `stream=` must not automatically add content headers merely because a stream object is present.

Remove automatic `Transfer-Encoding: chunked` injection from the explicit `stream=` path unless it is part of the encoded-content path selected by HTTPX.

## 2.2 Match empty-body method behavior

For empty POST, PUT, and PATCH requests, add `Content-Length: 0` where the reference does.

Do not add that header indiscriminately to GET, HEAD, DELETE, or OPTIONS.

Explicit caller headers retain precedence.

## 2.3 Make Host construction lossless

Host header generation must preserve:

- IPv4 and DNS names;
- IPv6 bracket syntax;
- non-default ports;
- omission of default ports;
- IDNA behavior inherited from the URL object.

## 2.4 Enforce request stream type and consumption state

Required behavior:

- unread streaming `.content` raises `RequestNotRead`;
- `read()` consumes a sync stream once and caches content;
- `aread()` consumes an async stream once and caches content;
- sync Client rejects an async request stream;
- AsyncClient rejects an incompatible sync-only stream where HTTPX does;
- repeated reads return cached content where allowed;
- invalid second iteration raises `StreamConsumed` rather than returning empty data silently.

Expose `is_stream_consumed` consistently.

### Track 2 acceptance criteria

- [x] Explicit `stream=` does not receive encoded-content auto-headers.
- [x] Empty POST/PUT/PATCH header behavior matches HTTPX.
- [x] Host formatting passes IPv4, IPv6, port, and IDNA cases.
- [x] Unread streaming content raises `RequestNotRead`.
- [x] Sync/async stream mismatches fail before dispatch.
- [x] Request consumption state is externally consistent with HTTPX.

# Track 3 — Surface response protocol metadata correctly

## 3.1 Treat response extensions as the source of protocol metadata

`Response.http_version` and `Response.reason_phrase` must read the standard extension values when present.

Handle the pinned HTTPX representation, including byte values such as:

- `b"HTTP/1.1"`;
- `b"HTTP/2"`;
- native reason-phrase bytes.

If native bindings currently provide strings, normalize them once when constructing extensions. Do not store valid extension values and then ignore them behind hardcoded properties.

## 3.2 Preserve request and URL attachment

`Response.request` must:

- return the attached compatibility Request;
- raise the HTTPX-compatible runtime error when no request is attached;
- be assignable where the reference exposes a setter.

`Response.url` must derive from the attached request rather than a separate stale field.

## 3.3 Measure elapsed time

For network and in-process transports, record the duration from immediately before the transport call until response close/read completion according to HTTPX semantics.

Requirements:

- `.elapsed` raises before the response has been read or closed where HTTPX does;
- buffered responses expose measured elapsed time on return;
- streaming responses expose elapsed time after close or full consumption;
- elapsed values are non-negative and not hardcoded to zero;
- custom transport timing is included.

A native timing field may be used, but the facade must define one consistent start/finish boundary.

### Track 3 acceptance criteria

- [x] HTTP/2 native responses report `HTTP/2` through the compatibility facade.
- [x] Reason phrase uses response extensions when supplied.
- [x] Missing request attachment produces the correct runtime error.
- [x] Response URL cannot diverge from the attached request URL.
- [x] Buffered and streaming elapsed timing follow HTTPX state rules.
- [x] No response initializes elapsed to a misleading constant zero.

# Track 4 — Correct response status and redirect state

## 4.1 Match status predicates

Verify the pinned behavior for:

- `is_informational`;
- `is_success`;
- `is_redirect`;
- `is_client_error`;
- `is_server_error`;
- `is_error`;
- `has_redirect_location`.

`has_redirect_location` must be limited to redirect statuses for which HTTPX builds a next request and must require a Location header.

## 4.2 Match `raise_for_status()`

Required behavior:

- raise when no request is attached;
- return `self` for successful responses;
- raise `HTTPStatusError` for informational, redirect, client-error, server-error, and invalid status classes according to HTTPX;
- attach the exact request and response objects;
- include equivalent error category, status, URL, redirect location where applicable, and status-information URL in the message.

Do not limit raising to 4xx and 5xx.

## 4.3 Add `next_request`

`Response.next_request` must exist and default to `None`.

Phase 4 will populate it when redirects are not followed. This phase establishes the public state, constructor behavior, property semantics, and tests with manually assigned values.

## 4.4 Preserve redirect history mutability semantics

`history` must be copied at construction and assignable or replaceable where HTTPX’s client state machine requires it. Avoid returning a shared caller-owned list.

### Track 4 acceptance criteria

- [x] All status predicates match the pinned reference across representative 1xx–5xx codes.
- [x] `raise_for_status()` raises for non-success status classes as HTTPX does.
- [x] Raised errors retain request and response identity.
- [x] Redirect error messages include Location when present.
- [x] `next_request` exists and defaults to `None`.
- [x] History construction does not alias caller-owned lists.

# Track 5 — Correct response content, encoding, and stream state

## 5.1 Raise HTTPX stream exceptions

Required unread-state behavior:

- `.content` raises `ResponseNotRead` before a streaming body is read;
- `.text` and `.json()` reach the same unread-state error through `.content`;
- iterating a closed stream raises `StreamClosed`;
- consuming a stream twice raises `StreamConsumed`;
- sync versus async iterator misuse raises the same runtime error category as HTTPX.

Do not use generic `RuntimeError` where the public exception hierarchy defines a specific class.

## 5.2 Expose stream state

Expose and update:

- `is_closed`;
- `is_stream_consumed`;
- `num_bytes_downloaded`.

State must change on:

- buffered construction;
- each raw network chunk;
- complete read;
- iterator exhaustion;
- explicit close;
- exceptional close.

## 5.3 Preserve raw versus decoded iteration

Ensure:

- `iter_raw()`/`aiter_raw()` yield undecoded transport bytes;
- `iter_bytes()`/`aiter_bytes()` yield decoded content;
- chunk-size behavior matches HTTPX, including `None` if supported;
- text decoders preserve multibyte characters across chunk boundaries;
- line iteration handles CRLF and final unterminated lines as the reference does.

Avoid decoding an entire buffered body and slicing characters by count where HTTPX’s byte-chunk/text-decoder semantics differ.

## 5.4 Match encoding behavior

Support `default_encoding` as:

- a string;
- a callable receiving content bytes after content is available.

Requirements:

- explicit `.encoding` overrides charset/default selection;
- invalid/unknown charset falls back according to HTTPX;
- setting `.encoding` after `.text` has been accessed raises `ValueError`;
- `.text` uses an incremental decoder and caches the result;
- JSON decoding behavior follows the pinned reference.

## 5.5 Preserve body content across wrappers

When rewrapping a response from a native or custom transport:

- a buffered body remains buffered;
- a streaming body remains streaming;
- content is never replaced with `None` merely because `_stream` is absent;
- response extensions, request, history, and encoding policy survive;
- no wrapper consumes a one-shot body solely to classify it.

### Track 5 acceptance criteria

- [x] Unread streaming properties raise `ResponseNotRead`.
- [x] Closed and consumed stream errors use the compatibility exception hierarchy.
- [x] State flags update on all completion and failure paths.
- [x] Raw and decoded iterators are distinct.
- [x] Multibyte text split across chunks decodes correctly.
- [x] Line iteration matches CRLF and final-line behavior.
- [x] Callable default encoding works after content becomes available.
- [x] Encoding cannot be reset after text access.
- [x] Buffered custom-transport content survives response wrapping.

# Track 6 — Public representations and missing exports

## 6.1 Match stable repr behavior

Correct representative repr output for:

- `Request`;
- `Response`;
- stream errors if relevant.

Response repr should include status and reason phrase where the reference does. Do not expose credentials or sensitive headers.

## 6.2 Add or explicitly classify missing public exports

Audit the HTTPX 0.28.1 top-level manifest for at least:

- `create_ssl_context`;
- `main`.

Implement low-cost public compatibility where it does not widen the networking architecture. Otherwise add an exact intentional-difference record and ensure the drop-in claim is bounded.

The CLI entry point is lower priority than request behavior and must not trigger a CLI redesign.

### Track 6 acceptance criteria

- [x] Request and Response repr match the stable public reference shape.
- [x] Sensitive data remains redacted.
- [x] Every missing public export has either an implementation or exact active difference record.
- [x] No private HTTPX module compatibility is added by implication.

# Testing plan

Suggested focused files:

- `test_request_construction_parity.py`
- `test_request_stream_state.py`
- `test_response_metadata_parity.py`
- `test_response_status_and_redirect_state.py`
- `test_response_stream_state_parity.py`
- `test_response_encoding_parity.py`

Required representative fixtures:

- duplicate query values;
- empty POST/PUT/PATCH;
- JSON with Unicode;
- multipart data plus files;
- explicit low-level stream;
- HTTP/1.1 and HTTP/2 metadata;
- 1xx, 2xx, 3xx, 4xx, and 5xx responses;
- streaming UTF-8 characters split across chunks;
- CRLF lines and final unterminated line;
- buffered and streaming custom transport responses.

Run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers

./scripts/check.sh
```

Do not add a new workflow or a separate generated evidence format.

# Phase completion criteria

Phase 2 is complete only when:

- every Track 1–6 acceptance item is satisfied;
- no valid HTTPX multipart `data` plus `files` request is rejected;
- direct Request query parameters cannot diverge from the URL;
- unread and consumed stream states use HTTPX exception classes;
- HTTP protocol metadata is surfaced from actual response extensions;
- elapsed time is meaningful and state-dependent;
- `raise_for_status()` covers all reference status classes;
- custom transport wrapping preserves buffered and streaming bodies;
- all required differential tests pass without skips or xfails;
- the public API oracle has no unexplained new differences;
- no new CI architecture was introduced.

## Stop conditions

Stop and record a blocker if:

- native bindings cannot expose protocol or timing information without a broad core rewrite;
- exact multipart streaming would require replacing the native multipart system rather than adapting its boundary;
- raw-versus-decoded iteration cannot be represented because the native stream exposes only already-decoded bytes;
- a public export requires private HTTPX internals or a separate CLI project.

A blocker must be reflected in the compatibility claim and active difference registry; it must not be hidden by a placeholder property or generic fallback.