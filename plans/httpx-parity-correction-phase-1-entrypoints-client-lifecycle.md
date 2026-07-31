# HTTPX Parity Correction Phase 1 — Entrypoints, Client Configuration, Auth Normalization, and Lifecycle

Status: implemented

Depends on: `plans/httpx-parity-correction-roadmap.md`

## Objective

Correct the public call paths through which users enter the HTTPX compatibility facade. This phase must eliminate silent argument loss, invalid top-level streaming behavior, inconsistent auth input handling, and client lifecycle behavior that diverges from HTTPX 0.28.1.

This is primarily a Python-facade phase. Native changes are permitted only when a supported client option cannot otherwise be represented faithfully. Do not begin the redirect/auth/cookie state-machine redesign from Phase 4 here.

## Audited files

Review at minimum:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_headers.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_timeout.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_limits.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py`
- native Python `Client` and `AsyncClient` constructor bindings
- existing compatibility tests for top-level API, auth, client lifecycle, defaults, and streaming

## Scope constraints

This phase may:

- change public helper signatures to match HTTPX 0.28.1;
- introduce internal argument-partitioning helpers;
- normalize auth inputs into compatibility Auth objects;
- correct client property setters and default headers;
- enforce client state transitions;
- validate protocol booleans and unsupported options;
- add focused sync and async compatibility tests.

This phase must not:

- add Trio or AnyIO support;
- add SOCKS, UDS, local-address, or socket-option implementations;
- add a new CI job or test harness;
- implement redirect loops, cookie processing, or per-hop hooks beyond interfaces needed by later phases;
- move network I/O into Python.

# Track 1 — Repair top-level convenience functions

## 1.1 Match HTTPX 0.28.1 public signatures

Replace variadic top-level wrappers with explicit signatures matching the pinned reference for:

- `request()`
- `stream()`
- `get()`
- `options()`
- `head()`
- `post()`
- `put()`
- `patch()`
- `delete()`

Preserve method-specific restrictions. In particular, body arguments should not appear on helpers where HTTPX excludes them.

The manifest oracle must observe matching parameter names, order, keyword-only status, and defaults for the claimed surface.

## 1.2 Partition temporary-client arguments from request arguments

Introduce one internal helper that separates:

Client construction arguments:

- `cookies`
- `proxy`
- `verify`
- `timeout`
- `trust_env`

Request arguments:

- method and URL;
- `params`;
- `content`;
- `data`;
- `files`;
- `json`;
- `headers`;
- `auth`;
- `follow_redirects`.

Do not pass `proxy`, `verify`, or `trust_env` into `Client.request()`. Do not accidentally omit request-level `auth` or `follow_redirects`.

Add negative tests that fail if a client-only option reaches the request method or if a request-only option is consumed during client construction.

## 1.3 Implement top-level `stream()` as a real context manager

The helper must use `@contextmanager` and `yield` the response while both the temporary client and streaming response remain open.

Required behavior:

```python
with compat_httpx.stream("GET", url) as response:
    assert not response.is_closed
    consume(response)
assert response.is_closed
```

Exceptions raised inside the caller’s `with` block must still close the response and temporary client exactly once.

The implementation must not return a response from inside nested `with` blocks.

### Track 1 acceptance criteria

- [ ] Public helper signatures match HTTPX 0.28.1 for the supported surface.
- [ ] `proxy`, `verify`, `timeout`, and `trust_env` configure the temporary client.
- [ ] Request parameters reach the request call unchanged.
- [ ] Top-level `stream()` yields an open response.
- [ ] Exiting top-level `stream()` closes response and client once.
- [ ] Error exits do not leak the response or client.
- [ ] Focused tests cover every helper, not only `request()`.

# Track 2 — Preserve per-call overrides through client streaming

## 2.1 Extract stream-call options before request construction

`Client.stream()` and `AsyncClient.stream()` currently pass all keyword arguments to `build_request()` and then call `send(..., stream=True)` without preserving some send-level options.

Explicitly separate:

- request construction arguments;
- `auth` override;
- `follow_redirects` override;
- `timeout` override.

Call `send()` with all three override values using the same sentinel semantics as ordinary `request()`.

## 2.2 Preserve omitted versus explicit values

The implementation must distinguish:

- omitted `timeout` → use client default;
- `timeout=None` → disable timeouts;
- omitted `auth` → use client default;
- `auth=None` → disable client auth, consistent with HTTPX;
- omitted `follow_redirects` → use client default;
- explicit `False` or `True` → override client default.

Do not use `None` as the sentinel for any option where HTTPX distinguishes omission from explicit disable.

## 2.3 Keep sync and async cleanup equivalent

Sync streaming must call `response.close()`.

Async streaming must await `response.aclose()` and must not fall back to synchronous close for a compatibility response that exposes async cleanup.

Both paths must close a partially created response if an exception occurs after dispatch but before the context manager yields.

### Track 2 acceptance criteria

- [ ] Sync stream calls preserve auth, redirect, and timeout overrides.
- [ ] Async stream calls preserve auth, redirect, and timeout overrides.
- [ ] Explicit `timeout=None` disables timeouts rather than selecting the client default.
- [ ] Explicit `auth=None` disables client auth.
- [ ] Explicit redirect values override the client default in both directions.
- [ ] Sync cleanup is synchronous and async cleanup is awaited.
- [ ] No stream override is silently ignored by `build_request()`.

# Track 3 — Normalize authentication inputs

## 3.1 Centralize auth construction

Add a single `_build_auth()` equivalent used by both `Client` and `AsyncClient` for constructor and request-level auth values.

Supported forms must include:

- `None`;
- `(username, password)` tuple;
- compatibility `Auth` instance;
- callable auth where HTTPX 0.28.1 supports it;
- the facade’s explicit no-auth sentinel, if retained internally.

Invalid auth inputs must raise `TypeError` before network dispatch.

## 3.2 Normalize constructor auth immediately

Store the normalized auth object, not the raw tuple or callable. Property setters must also normalize.

A tuple must never reach code that blindly invokes `.sync_auth_flow()` or `.async_auth_flow()` on it.

## 3.3 Resolve request-level auth and URL credentials

At send time:

1. use request-level auth if explicitly supplied;
2. otherwise use client auth;
3. otherwise derive Basic Auth from URL user-info when present;
4. otherwise use an empty Auth flow.

Ensure credentials are redacted from repr, logs, and exceptions according to existing project policy.

The full auth challenge loop belongs to Phase 4. This phase only establishes correct input normalization and selection.

## 3.4 Preserve callable auth semantics

Implement or reuse a small `FunctionAuth` adapter matching HTTPX’s public callable-auth behavior. It must operate through the same sync and async auth interfaces and must not receive an incompatible request object.

### Track 3 acceptance criteria

- [ ] Tuple auth becomes `BasicAuth` for sync and async clients.
- [ ] Callable auth becomes a compatibility auth adapter.
- [ ] Invalid auth input fails before dispatch.
- [ ] URL credentials are used when no explicit auth exists.
- [ ] Explicit per-request `auth=None` disables client and URL auth according to the pinned reference behavior.
- [ ] Auth property assignment reuses the same normalization path.
- [ ] Credential-bearing repr and errors remain redacted.

# Track 4 — Correct client state and mutable configuration

## 4.1 Introduce an explicit client state enum or equivalent

Model at least:

- unopened;
- opened;
- closed.

After close, the client must remain closed permanently. `_ensure_client()` must not recreate a native client when the compatibility client is closed.

`__enter__`, `__aenter__`, `send`, and `stream` must reject use after close with an HTTPX-compatible `RuntimeError` message category.

Closing an unopened or already closed client must be idempotent.

## 4.2 Implement HTTPX-exposed property setters

Where HTTPX 0.28.1 exposes mutable properties, implement compatible setters for:

- `auth`;
- `base_url`;
- `cookies`;
- `event_hooks`;
- `headers`;
- `params`;
- `timeout`.

Setter values must be copied or normalized as HTTPX does, rather than retaining caller-owned mutable containers unintentionally.

Changing configuration after the native client has been lazily created requires a clear policy:

- either rebuild before the next request without reviving a closed client;
- or store per-request compatibility state and pass it explicitly.

Do not silently leave the existing native client with stale configuration.

## 4.3 Enforce base URL trailing-slash semantics

Store the base URL in HTTPX-compatible canonical form and preserve relative URL joining semantics. Add cases for:

- base URL with and without trailing slash;
- request path with and without leading slash;
- absolute request URL bypassing the base URL;
- query and fragment handling.

## 4.4 Provide HTTPX default headers at the compatibility layer

Compatibility `Client.headers` and built requests must include HTTPX-equivalent defaults, subject to installed compression support:

- `Accept: */*`;
- `Accept-Encoding` matching supported decoders;
- `Connection: keep-alive` where applicable;
- HTTPX-compatible user-agent identity policy.

User headers override defaults using duplicate-preserving header semantics.

These headers must be visible to event hooks and custom transports, not injected only inside the native engine after the compatibility Request has been built.

### Track 4 acceptance criteria

- [ ] Closed clients cannot reopen through lazy initialization or re-entry.
- [ ] Repeated close is safe.
- [ ] Use after close fails consistently for request, send, stream, and context entry.
- [ ] Mutable properties match HTTPX’s public setter behavior.
- [ ] Property changes affect subsequent requests without stale native configuration.
- [ ] Base URL joining matches the pinned reference.
- [ ] Default headers are visible on built requests and to custom transports.

# Track 5 — Validate protocol switches and unsupported options

## 5.1 Honor or reject `http1` and `http2`

Define supported combinations explicitly:

- `http1=True, http2=False`;
- `http1=True, http2=True`;
- `http1=False, http2=True` if the native engine can enforce H2-only behavior;
- `http1=False, http2=False` must fail.

If H2-only policy is not implementable in the current native engine, reject `http1=False` with a precise unsupported error rather than storing and ignoring it.

## 5.2 Fail fast for unsupported transport constructor arguments

Until separately implemented, non-default values for these HTTPX transport options must raise `NotImplementedError` or a documented compatibility error:

- `uds`;
- `local_address`;
- `socket_options`.

Default `None` values remain accepted for signature compatibility.

Do not claim support in the feature matrix merely because constructors accept the parameter name.

## 5.3 Add a reusable unsupported-option assertion helper

Tests should verify:

- the error occurs at construction or before dispatch;
- the message names the unsupported option;
- no native request occurs;
- sync and async transport constructors behave the same way.

### Track 5 acceptance criteria

- [ ] Every accepted protocol combination has enforced behavior.
- [ ] Invalid protocol combinations fail immediately.
- [ ] H2-only is either real or explicitly unsupported.
- [ ] UDS, local address, and socket options do not silently no-op.
- [ ] Compatibility documentation distinguishes accepted signatures from implemented behavior.

# Testing plan

Add focused cases under the existing compatibility suite. Suggested organization:

- `test_top_level_helpers_parity.py`
- `test_client_stream_overrides.py`
- `test_auth_input_normalization.py`
- `test_client_mutability_and_state.py`
- `test_protocol_and_unsupported_options.py`

Use direct comparison with installed `httpx==0.28.1` where behavior is not already encoded in an existing differential helper.

Required command set:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest \
  crates/eggfetch-python/tests/compat/test_top_level_helpers_parity.py \
  crates/eggfetch-python/tests/compat/test_client_stream_overrides.py \
  crates/eggfetch-python/tests/compat/test_auth_input_normalization.py \
  crates/eggfetch-python/tests/compat/test_client_mutability_and_state.py \
  crates/eggfetch-python/tests/compat/test_protocol_and_unsupported_options.py \
  -q --strict-markers

./scripts/check.sh
```

Do not add a dedicated workflow for these files.

# Phase completion criteria

Phase 1 is complete only when:

- every Track 1–5 acceptance item is satisfied;
- the top-level API manifest has no unexplained differences introduced by the corrected signatures;
- all required focused tests pass with zero skips and zero xfails;
- ordinary compatibility tests remain green;
- no client-only option is silently passed to a request method;
- no request-level stream override is silently discarded;
- no raw tuple or callable reaches an auth-flow method;
- no closed client can recreate native resources;
- unsupported transport options fail before network activity;
- no new CI job, matrix, evidence schema, or release path was introduced.

## Stop conditions

Stop and record a blocker rather than widening scope if:

- correct top-level behavior requires changing HTTPX’s pinned public signatures;
- mutable property updates cannot be represented without redesigning native client configuration beyond this phase;
- H2-only enforcement would require a broad core transport rewrite;
- an unsupported option is needed by a retained downstream consumer and cannot be failed explicitly without changing the current compatibility stage.

A blocker does not authorize silent fallback. Keep the status at `Stage C candidate` and document the exact unsupported behavior.