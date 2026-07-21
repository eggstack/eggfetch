# HTTPX Drop-In Phase 2: Object Model and Core API Parity

Status: ready for implementation handoff

## Purpose

Implement the documented HTTPX 0.28.1 public data model and ordinary client API as a compatibility facade over the existing eggfetch Rust engine.

This phase focuses on construction, signatures, configuration merging, object relationships, metadata, return values, and exceptions. It must not duplicate network behavior in Python. A request built by compatibility objects must still execute through `eggfetch-core` unless a later explicit in-process transport is selected.

## Dependencies

Phase 0 must be complete so every public-surface gap is measured against a pinned manifest. Phase 1 should be complete for timeout, limits, and lifecycle semantics before this phase's client API is considered production-ready.

## Baseline

The current Python package exports native `Client`, `AsyncClient`, buffered and streaming response wrappers, headers, cookies, basic/bearer auth, timeout, retry, file helpers, top-level request methods, and an eggfetch-specific exception hierarchy.

The compatibility surface is missing or materially different in areas including:

- structured `URL` and `QueryParams` objects;
- public `Request` construction;
- HTTPX-compatible `Response` construction and metadata;
- `Limits`, `Proxy`, status-code helpers, and stream base classes;
- `build_request()` and `send()`;
- `base_url`, client-level params, mounts, hooks, transport, trust environment, and default encoding;
- complete exception inheritance and attached request/response context;
- chainable `raise_for_status()`;
- request attachment, next request, elapsed time, extensions, and response stream metadata.

Record exact manifest deltas before implementation begins.

## Non-goals

- Implementing custom transport execution, mounts, ASGI, WSGI, or mock behavior; only surface placeholders and validated rejection may land where necessary.
- Completing scalable Python request streaming; Phase 3 owns body-stream execution.
- Supporting private HTTPX modules.
- Matching arbitrary object identity or internal storage layout.
- Requiring Rust users to adopt HTTPX-shaped APIs.
- Creating the top-level `httpx` compatibility distribution before downstream validation.

## Deliverables

1. A dedicated compatibility module with documented import policy.
2. HTTPX-compatible public value objects.
3. HTTPX-compatible request and response objects.
4. HTTPX-compatible exception hierarchy and context attachment.
5. Client and async-client constructor signatures and state.
6. `build_request()`, `send()`, request helpers, and top-level helpers.
7. Deterministic configuration-merge semantics.
8. Manifest and behavioral tests with no unexplained Phase 2 deltas.
9. A phase implementation status file.

## Track A — Compatibility module architecture

### A1. Separate native and compatibility surfaces

Create a clear module boundary, for example:

- `eggfetch.compat.httpx`
- internal pure-Python modules under `eggfetch/compat/`
- native extension primitives imported behind the facade.

The ordinary `eggfetch` API may preserve eggfetch-native additions. The compatibility module must avoid exposing unrelated native names that are absent from HTTPX unless they are under an explicitly eggfetch-specific namespace.

### A2. Public export discipline

Drive `__all__` and public-module manifests from an explicit compatibility export table. Validate:

- top-level names;
- object modules and qualified names;
- aliases;
- constants;
- exception classes;
- status-code helpers;
- stream base classes;
- auth and transport names assigned to later phases.

Unimplemented required-later objects may initially exist only if construction or use raises a precise compatibility-stage error and the Phase 0 profile marks them open. Do not provide nonfunctional silent stubs.

### A3. Native bridge boundary

Define conversion protocols between compatibility objects and native extension types:

- URL to normalized core URL;
- headers preserving duplicate values and raw bytes;
- request body stream to core request body;
- timeout and limits to core configuration;
- proxy and TLS configuration to core builders;
- native response metadata to compatibility response;
- native errors to compatibility exceptions.

Keep conversion centralized and testable.

## Track B — URL and query model

### B1. Implement `URL`

Match the pinned reference for:

- constructors from string, bytes where supported, and existing URL;
- scheme, userinfo, host, raw host, port, path, raw path, query, raw query, fragment;
- authority, netloc, origin, and absolute/relative status;
- IDNA and Unicode handling;
- percent encoding and normalization;
- default ports;
- copying and mutation helpers such as `copy_with()`;
- joining and relative resolution where public;
- equality, hashing, string, bytes, repr, and ordering behavior if defined;
- credential redaction in display paths;
- invalid URL exceptions.

Do not use `url::Url` stringification as the sole compatibility definition; its normalization may differ from HTTPX.

### B2. Implement `QueryParams`

Support:

- mapping and sequence construction;
- repeated keys;
- blank values;
- scalar and sequence values according to reference conversion rules;
- `get()`, `get_list()`, `multi_items()`, keys, values, items;
- add/set/remove/merge operations exposed by the target;
- equality, hashing if defined, string, repr;
- stable encoding rules.

### B3. URL/query differential corpus

Add cases for:

- Unicode host and path;
- IPv4, IPv6, zone identifiers if supported;
- username/password;
- empty path and query;
- repeated and blank query keys;
- encoded slash and reserved characters;
- relative base URL resolution;
- invalid ports and malformed percent escapes;
- fragments excluded from wire requests.

## Track C — Headers and cookies

### C1. Complete `Headers`

Match:

- construction from mappings, sequences, bytes pairs, and existing headers;
- case-insensitive lookup with preserved raw representation;
- duplicate-field combination behavior;
- `get_list()`, `multi_items()`, raw iteration, keys, values, items;
- mutation where public;
- encoding selection and validation;
- equality and repr;
- CR/LF and invalid token rejection;
- `Set-Cookie` special handling.

The core bridge must not lose duplicate header fields by converting through a normal Python dictionary.

### C2. Complete `Cookies`

Match the public mapping and jar interactions required by HTTPX, including:

- construction from mappings and jars;
- set/get/delete/clear/update;
- domain and path disambiguation;
- conflict exceptions;
- request extraction and response update;
- iteration and repr;
- client-level mutable jar behavior.

Do not weaken the existing core cookie security behavior. Where HTTPX exposes a broader Python cookie-jar interface, adapt through a controlled facade.

## Track D — Timeout, limits, proxy, and status helpers

### D1. Complete `Timeout`

Match constructor forms, validation, repr, equality where defined, and attribute names for:

- scalar default;
- connect/read/write/pool values;
- `None` values;
- copying or dictionary conversion where public.

Map compatibility timeout fields to Phase 1 core deadlines without introducing a hidden total timeout unless the native API explicitly exposes one outside the HTTPX facade.

### D2. Complete `Limits`

Expose the pinned constructor, defaults, attributes, validation, repr, and core conversion from Phase 1.

### D3. Implement `Proxy`

Match public construction and attributes for URL, SSL context, auth, and headers. Preserve proxy credential redaction. Execution behavior belongs partly to the core and Phase 4 transport routing, but ordinary `proxy=` use must function in this phase where already supported.

### D4. Status codes

Implement the target's `codes` enumeration/helper behavior and any public constants or predicates included in the manifest.

## Track E — Request object

### E1. Construction

Implement HTTPX-compatible `Request` construction with:

- method;
- URL;
- params;
- headers;
- cookies;
- content/data/files/json;
- stream;
- extensions.

Body mutual-exclusion and auto-header behavior must match the reference, including content length, transfer encoding, host, accept, accept-encoding, connection, and user-agent defaults where applicable.

### E2. Request properties and methods

Support the target's public properties and methods, including:

- method mutation policy;
- URL and headers;
- content access state;
- stream access;
- extensions;
- `read()` and `aread()` behavior;
- `is_stream_consumed` where public;
- repr;
- copy/serialization behavior only where public.

### E3. Replayability metadata

Attach internal replayability classification without exposing incompatible public fields. Redirect and retry logic must use the same body object and state model as Phase 3.

## Track F — Response object

### F1. Construction and attachment

Implement public response construction for test, transport, and user-created responses, including:

- status code;
- headers;
- content/text/html/json-compatible input paths if public;
- stream;
- request;
- extensions;
- history;
- default encoding.

A response returned by the client must attach the exact compatibility request object used for execution.

### F2. Metadata and properties

Match:

- status helpers;
- reason phrase;
- URL derived from request;
- HTTP version from extensions or transport metadata;
- cookies;
- encoding getter and setter behavior;
- charset and apparent/default decoding policy;
- content/text/json;
- elapsed;
- history;
- next request;
- request property failure when not attached;
- extensions;
- stream/read state;
- repr.

### F3. `raise_for_status()`

Match the reference for:

- informational and redirect responses where applicable;
- client and server errors;
- message formatting and documentation link format where observable;
- attached request and response;
- return value of the response itself;
- error when no request is attached.

### F4. Redirect history bodies

Do not replace redirect responses with metadata-only objects if the reference exposes readable history bodies. Preserve bounded behavior using configurable limits or consumed buffers consistent with the reference.

## Track G — Exception hierarchy

### G1. Class structure

Implement the pinned public hierarchy, including the relationships among:

- `HTTPError`;
- `RequestError`;
- `TransportError`;
- timeout subclasses;
- network subclasses;
- protocol subclasses;
- proxy errors where public;
- `DecodingError`;
- `TooManyRedirects`;
- `HTTPStatusError`;
- invalid URL and cookie conflict classes;
- stream state exceptions;
- unsupported protocol.

The manifest test must validate MRO and subclass relationships.

### G2. Exception context

Every request-related exception must preserve a compatibility `request` attribute when the reference does. `HTTPStatusError` must preserve both request and response.

Do not construct public exceptions only from a message string and discard context.

### G3. Mapping policy

Create one centralized mapping from core errors to compatibility exceptions. It must:

- preserve the most specific class available;
- retain safe source details;
- redact credentials;
- avoid exposing Rust implementation reprs as the stable message contract;
- distinguish read/write/connect/close/protocol failures;
- map unsupported features precisely.

## Track H — Client and AsyncClient surface

### H1. Constructor signatures

Match the pinned public signatures and defaults for:

- auth;
- params;
- headers;
- cookies;
- verify;
- cert;
- trust environment;
- HTTP/1 and HTTP/2 flags;
- proxy;
- mounts;
- timeout;
- redirects;
- limits;
- max redirects;
- event hooks;
- base URL;
- transport;
- default encoding.

Phase 4 may complete execution for mounts, hooks, transports, and additional auth flows, but constructor acceptance and state must be designed now.

### H2. Configuration merge rules

Differentially test client-level and request-level merging for:

- URL/base URL;
- params;
- headers;
- cookies;
- auth;
- timeout;
- extensions;
- redirect policy;
- request body and auto headers.

Use explicit merge functions rather than scattered conditionals in every verb method.

### H3. `build_request()`

Build a compatibility `Request` without sending. It must include the same merged configuration and auto headers that `request()` would send.

### H4. `send()`

Accept a compatibility `Request`, preserve object attachment, and support the target's public send options such as stream, auth, and redirects.

`send()` must not reconstruct a semantically different request from only method and URL.

### H5. Request methods

Implement exact signatures and return types for:

- `request()`;
- `get()`;
- `options()`;
- `head()`;
- `post()`;
- `put()`;
- `patch()`;
- `delete()`;
- `stream()`;
- `close()`/`aclose()`;
- context managers.

### H6. Client properties

Expose and validate mutable/read-only behavior for public properties such as headers, cookies, params, auth, timeout, base URL, trust environment, and closed state.

## Track I — Top-level helpers

Match top-level request functions and top-level streaming where public:

- signatures and defaults;
- short-lived client behavior;
- timeout defaults;
- redirect defaults;
- environment trust;
- response and exception types;
- deterministic cleanup.

Top-level helpers must use the same compatibility client implementation, not a separate simplified path.

## Track J — Testing and evidence

### J1. Manifest closure

After implementation, Phase 2-required symbols must have no unexplained manifest differences. Required-later transport/backend items may remain only if assigned to Phase 4.

### J2. Construction differential tests

Compare reprs, signatures, properties, exceptions, and normalized state for every public object using safe deterministic fixtures.

### J3. Merge matrix

Generate a parameterized matrix for client/request configuration combinations. Include duplicate headers and params, cookie precedence, auth disable/override, timeout override, base URL edge cases, and extensions.

### J4. Built-wheel tests

Run the same API and construction corpus against a clean wheel install, not only an editable source tree.

## Expected files

Likely additions or changes include:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/`
- `crates/eggfetch-python/src/` native bridge modules;
- `crates/eggfetch-python/tests/compat/test_api_manifest.py`
- `crates/eggfetch-python/tests/compat/test_objects.py`
- `crates/eggfetch-python/tests/compat/test_client_merge.py`
- `crates/eggfetch-python/tests/compat/test_exceptions.py`
- compatibility profile and allowed-difference files;
- user and migration documentation;
- `plans/httpx-drop-in-phase-2-status.md`.

## Acceptance criteria

This phase is complete only when:

- [ ] The compatibility module has a stable documented import path.
- [ ] HTTPX 0.28.1 public Phase 2 symbols import from the compatibility module.
- [ ] `URL` passes the pinned construction, normalization, copy, equality, hashing, and repr corpus.
- [ ] `QueryParams` preserves repeated keys and passes encoding and mutation semantics.
- [ ] `Headers` preserves raw duplicates and passes lookup, iteration, mutation, and validation semantics.
- [ ] `Cookies` passes mapping, conflict, domain/path, request, and response behavior.
- [ ] `Timeout`, `Limits`, and `Proxy` signatures, defaults, reprs, and validation match the profile.
- [ ] Status-code helpers match the public target.
- [ ] `Request` construction and auto-header behavior match the reference.
- [ ] `Request.read()` and `aread()` state behavior is correct.
- [ ] `Response` construction and public metadata match the reference.
- [ ] Client responses attach the original compatibility request.
- [ ] Redirect history contains reference-compatible response objects and bodies.
- [ ] `raise_for_status()` returns the response and raises context-rich errors.
- [ ] The exception hierarchy and MRO match the pinned manifest.
- [ ] Request-related exceptions expose the expected request attribute.
- [ ] Status exceptions expose request and response.
- [ ] `Client` and `AsyncClient` constructor signatures and defaults match the target.
- [ ] Client properties expose the expected mutability and types.
- [ ] `build_request()` matches the request that would be sent.
- [ ] `send()` accepts a constructed request without losing body, extensions, or object identity.
- [ ] All request verb signatures and top-level helper signatures match the manifest.
- [ ] Client/request merge behavior passes the generated differential matrix.
- [ ] Required Phase 2 manifest deltas are zero or explicitly reviewed allowed differences.
- [ ] Tests pass from built wheels on the supported Python matrix.
- [ ] `plans/httpx-drop-in-phase-2-status.md` links exact CI and manifest evidence.

## Handoff notes

Prefer pure-Python compatibility value objects where that produces exact semantics cheaply, but keep all network execution in the Rust core. Do not expose raw native wrapper constraints as the compatibility contract. In particular, PyO3 method signatures that are convenient for the extension are not sufficient if they differ from the pinned Python API.
