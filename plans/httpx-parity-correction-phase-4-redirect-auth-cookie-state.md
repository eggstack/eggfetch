# HTTPX Parity Correction Phase 4 — Redirect, Authentication, Cookie, and History State

Status: ready for implementation handoff

Depends on:

- `plans/httpx-parity-correction-roadmap.md`
- `plans/httpx-parity-correction-phase-1-entrypoints-client-lifecycle.md`
- `plans/httpx-parity-correction-phase-2-request-response-semantics.md`
- `plans/httpx-parity-correction-phase-3-transport-mount-hook-dispatch.md`

## Objective

Implement the HTTPX 0.28.1-visible multi-hop request state machine over the one-hop dispatch boundary established in Phase 3.

This phase aligns the interactions among:

- custom and built-in auth flows;
- redirects;
- request and response hooks;
- scoped cookies;
- request-body replayability;
- response history;
- manual `next_request` behavior;
- intermediate response draining and cleanup.

All network hops continue to execute through Rust-backed or in-process transports. The Python compatibility layer owns only HTTPX-specific orchestration.

## Audited files

Review at minimum:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_cookies.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_urls.py`
- native cookie bindings and core cookie jar interfaces
- core redirect method/header/body logic, for reference and reuse where appropriate
- existing compatibility tests for auth, redirects, cookies, hooks, streaming, and lifecycle

## Scope constraints

This phase may:

- implement facade-owned redirect and auth loops;
- use Python’s standard-library cookie jar or a structured native cookie bridge;
- expose scoped cookie import/export operations through PyO3;
- add small redirect/request-copy helpers;
- add focused differential tests.

This phase must not:

- create Python socket or HTTP protocol code;
- add browser-specific cookie policy beyond HTTPX 0.28.1;
- add OAuth frameworks or new auth schemes unrelated to HTTPX parity;
- add new CI workflows, soak systems, or evidence schemas;
- expand to private HTTPX redirect/auth internals as public contract.

# Track 1 — Build the shared multi-hop client algorithm

## 1.1 Separate high-level send from one-hop dispatch

Implement sync and async high-level send paths with the same logical stages:

1. validate client state and Request type;
2. set effective timeout extension;
3. select and construct request auth;
4. enter the auth flow;
5. for each auth-produced Request, execute redirect handling;
6. return the final Response;
7. read the response automatically unless `stream=True`;
8. close the Response on failures after it has been created.

Use separate sync and async drivers. Do not simulate async auth by calling sync generators or vice versa.

## 1.2 Keep auth and redirect history ownership explicit

Maintain one ordered history list owned by the high-level send operation.

Intermediate auth and redirect responses should enter history only where HTTPX does. Do not merge native history with compatibility history.

Before yielding or returning a final Response:

- assign a copy of history;
- attach the final Request;
- leave `next_request` populated only for an unfollowed redirect.

## 1.3 Close generators and responses on every exit

On success, failure, cancellation, or caller hook exception:

- close the auth generator;
- close any intermediate response not returned to the caller;
- release native pool permits and response streams;
- preserve the original exception.

Async generator cleanup must be awaited.

### Track 1 acceptance criteria

- [ ] Sync and async high-level sends implement equivalent stage ordering.
- [ ] Exactly one history list is authoritative.
- [ ] Native history does not leak into or duplicate compatibility history.
- [ ] Auth generators close on all exits.
- [ ] Intermediate responses close deterministically.
- [ ] `stream=False` reads the final Response and `stream=True` leaves it open.

# Track 2 — Match redirect request construction

## 2.1 Implement reference method rewriting

Match HTTPX 0.28.1 behavior for at least:

- 303: change non-HEAD methods to GET;
- 302: browser-compatible conversion to GET except HEAD;
- 301: convert POST to GET;
- 307 and 308: retain method and body.

When the method changes to GET:

- drop the request body;
- remove content-specific headers according to HTTPX;
- preserve unrelated headers.

## 2.2 Resolve redirect URLs correctly

Handle:

- absolute Location values;
- relative paths;
- scheme-relative URLs;
- malformed or non-ASCII Location values according to the pinned reference;
- fragment inheritance behavior;
- URL credentials and redaction;
- unsupported schemes.

The resulting Request URL must be a compatibility `URL` object.

## 2.3 Strip sensitive headers across origins

Match HTTPX’s origin rules and strip or retain at least:

- `Authorization`;
- `Proxy-Authorization` where applicable;
- `Cookie` when the cookie jar will regenerate it;
- `Host` for destination recalculation;
- content headers when method/body changes.

Preserve auth on safe same-origin HTTP→HTTPS redirects where HTTPX does. Do not preserve credentials across arbitrary origin changes.

## 2.4 Classify request-body replayability

Redirects that retain the method/body require a replayable body.

Classify:

- buffered bytes as replayable;
- file-like bodies as replayable only when position can be restored according to policy;
- one-shot sync iterators as not replayable after consumption;
- one-shot async iterators as not replayable after consumption;
- native multipart bodies according to their underlying parts.

Raise the correct stream/replay error before sending an invalid follow-up request. Do not silently send an empty body.

## 2.5 Implement manual redirect behavior

When `follow_redirects=False` and a valid redirect is received:

- build the next Request;
- assign it to `response.next_request`;
- do not send it;
- do not add the current Response to its own history;
- leave body ownership consistent with HTTPX.

When `follow_redirects=True`:

- drain/read the intermediate Response before reusing the connection;
- append it to history;
- close it as required;
- dispatch the next Request.

## 2.6 Enforce `max_redirects`

Count redirects exactly as HTTPX does and raise `TooManyRedirects` with the relevant Request attached.

Add boundary tests for zero, one, and configured maximum redirects.

### Track 2 acceptance criteria

- [ ] 301/302/303/307/308 method and body behavior matches HTTPX.
- [ ] Relative and absolute Location resolution matches the reference.
- [ ] Sensitive headers are stripped across origins correctly.
- [ ] Same-origin auth retention matches HTTPX.
- [ ] One-shot bodies are never silently replayed or replaced with empty data.
- [ ] Manual redirect responses expose `next_request` without dispatching it.
- [ ] Followed redirect responses are drained and added to history.
- [ ] Redirect limit counting and exception attachment match the reference.

# Track 3 — Complete sync and async authentication flows

## 3.1 Drive auth generators around redirect handling

For each auth-produced Request:

- execute the complete redirect handler for that Request;
- send the resulting Response back into the auth generator;
- allow the auth generator to yield a follow-up Request;
- drain and close intermediate challenge responses before dispatching the follow-up.

Do not bypass auth for mounted, custom, Mock, WSGI, or ASGI transports.

## 3.2 Preserve request identity and body state

Auth implementations may modify headers or construct a new Request. Hooks and transports must receive the Request yielded by the auth flow.

Digest auth or custom auth that requires a body hash must:

- read replayable content safely;
- restore replayable stream position;
- reject unavailable one-shot content with the correct behavior;
- avoid hashing an unrelated serialized representation.

## 3.3 Match cross-origin auth behavior

On redirects, recompute auth according to:

- explicit per-request auth;
- client auth;
- URL credentials;
- cross-origin credential stripping;
- auth flow’s own state.

Do not blindly carry an Authorization header from the previous Request.

## 3.4 Preserve auth response history

Determine and differential-test whether intermediate auth challenge responses appear in final `response.history` for each built-in flow. Implement the pinned behavior exactly.

## 3.5 Validate built-in auth classes

Cover at minimum:

- `BasicAuth`;
- `DigestAuth` with supported algorithms and qop modes;
- `NetRCAuth`;
- callable/FunctionAuth;
- a custom sync Auth subclass;
- a custom async Auth subclass.

Do not expand Digest algorithm support beyond HTTPX solely for this phase.

### Track 3 acceptance criteria

- [ ] Auth applies through every transport route.
- [ ] Sync auth uses `sync_auth_flow` and async auth uses `async_auth_flow`.
- [ ] Intermediate challenge responses are drained and closed.
- [ ] Digest body hashing respects replayability.
- [ ] Cross-origin redirects do not leak credentials.
- [ ] Custom auth Requests reach hooks and transports unchanged except for later redirect processing.
- [ ] Final history matches HTTPX for built-in auth flows.

# Track 4 — Replace flattened cookies with scoped cookie semantics

## 4.1 Select one authoritative compatibility cookie jar

Choose and document one of these bounded architectures:

### Preferred A — Standard-library `CookieJar` owned by the compatibility client

- compatibility `Cookies` wraps `http.cookiejar.CookieJar` like HTTPX;
- the facade generates Cookie headers before each one-hop dispatch;
- the facade extracts Set-Cookie headers after each response;
- native automatic cookies are disabled for compatibility dispatch to prevent two jars.

### Acceptable B — Structured native jar exposed losslessly

- expose cookie objects including name, value, domain, path, secure, expiry, and flags;
- compatibility `Cookies` is a faithful adapter over that jar;
- import/export and request selection are lossless;
- duplicate-name conflict behavior is implemented in the facade.

Do not retain both a dict facade and an independent native jar.

The implementation status must state which architecture was chosen and why.

## 4.2 Implement HTTPX Cookies public behavior

Support:

- construction from dict, list, `Cookies`, and `CookieJar` where HTTPX does;
- `.jar` access;
- `set(name, value, domain, path)`;
- `get(name, default, domain, path)`;
- `delete(name, domain, path)`;
- `clear(domain, path)`;
- `update()`;
- mutable mapping operations;
- duplicate-name `CookieConflict` behavior;
- repr without leaking inappropriate values beyond HTTPX behavior.

Do not accept scope arguments and discard them.

## 4.3 Parse all Set-Cookie headers

Use duplicate-preserving header access. A response may include multiple Set-Cookie fields.

Preserve and enforce:

- domain;
- path;
- secure;
- expiry/max-age;
- HttpOnly metadata;
- SameSite metadata to the extent represented by HTTPX’s CookieJar;
- host-only versus domain cookies;
- deletion cookies.

## 4.4 Select cookies for every hop

Before each request hook runs:

- merge client jar state with per-request cookies according to HTTPX;
- select cookies by URL domain, path, scheme, and expiry;
- generate a Cookie header;
- avoid carrying a stale Cookie header across redirects.

After each response hook stage at the reference-defined point, extract response cookies before constructing the next redirect/auth Request according to HTTPX ordering.

## 4.5 Keep public and native cookie state synchronized

`client.cookies` must immediately reflect cookies set by responses.

Mutating `client.cookies` between requests must affect the next request without reconstructing an unrelated client jar.

Response `.cookies` must represent cookies set by that Response, not the entire client jar.

### Track 4 acceptance criteria

- [ ] One cookie jar is authoritative for compatibility clients.
- [ ] Domain/path/secure/expiry state is retained.
- [ ] Same-name cookies on different scopes can coexist.
- [ ] Ambiguous `.get(name)` raises `CookieConflict`.
- [ ] Multiple Set-Cookie headers are all processed.
- [ ] Redirect and auth follow-up Requests receive newly set cookies where HTTPX does.
- [ ] Cross-domain and path-mismatched cookies are not sent.
- [ ] `client.cookies` mutation affects subsequent requests immediately.
- [ ] `response.cookies` contains response-set cookies rather than a flattened client snapshot.

# Track 5 — Integrate hooks, cookies, redirects, and auth in reference order

## 5.1 Define the per-hop order with tests

For each hop, differential-test the exact order among:

- auth yielding the concrete Request;
- cookie header generation;
- request hook;
- transport dispatch;
- response attachment;
- response hook;
- cookie extraction;
- redirect or auth continuation.

Use event recording rather than relying on implementation comments.

## 5.2 Ensure hook mutations follow HTTPX limits

Determine what public mutations HTTPX allows or observes at each hook stage. Preserve behavior for:

- adding headers in request hooks;
- reading response content in response hooks;
- response-hook exceptions;
- cookies set during response processing;
- auth follow-up requests.

Do not create stronger mutation guarantees than the pinned reference.

## 5.3 Prevent duplicate hook invocation

A redirect or auth challenge should produce one request-hook and one response-hook call per actual transport hop, no more and no less.

The outer client request method must not also invoke hooks.

### Track 5 acceptance criteria

- [ ] Event order matches HTTPX for direct, redirect, auth, and redirect-plus-auth sequences.
- [ ] Each actual hop produces exactly one request-hook and one response-hook invocation.
- [ ] Cookies and hook-visible headers match the reference at each stage.
- [ ] Response-hook content reads do not break later cleanup or history.
- [ ] Hook exceptions halt all further state-machine activity and close current resources.

# Track 6 — Preserve cancellation and resource ownership

## 6.1 Handle cancellation at every await point

Async cancellation may occur during:

- request hook;
- transport dispatch;
- response hook;
- response drain;
- auth generator send;
- redirect continuation;
- cookie processing if user code is involved.

Ensure current response and auth generator cleanup occurs without swallowing cancellation.

## 6.2 Avoid pool permit retention in history

Intermediate history responses must not retain open native stream/pool leases after they have been drained.

History bodies remain readable according to HTTPX, but resource handles must be released.

## 6.3 Bound redirect/auth loops

Redirects are bounded by `max_redirects`. Auth flows must not allow an accidental infinite generator/dispatch loop without any policy. Reuse HTTPX behavior where defined and add a conservative internal guard only if it does not alter legitimate custom auth.

### Track 6 acceptance criteria

- [ ] Cancellation closes current response and auth generator.
- [ ] Cancellation remains observable to the caller.
- [ ] History responses do not retain pool permits or open streams.
- [ ] Redirect loops are bounded by the public setting.
- [ ] Legitimate multi-step custom auth remains supported.

# Testing plan

Suggested focused files:

- `test_redirect_state_machine_parity.py`
- `test_auth_state_machine_parity.py`
- `test_cookie_scope_parity.py`
- `test_hook_cookie_auth_ordering.py`
- `test_multihop_cleanup.py`

Required local scenarios:

- 301/302/303/307/308 for GET, POST, and replayable/non-replayable bodies;
- same-origin and cross-origin redirects;
- HTTP→HTTPS same-host redirect where feasible in local fixtures;
- Basic and Digest challenge;
- custom two-step sync and async auth;
- response setting a cookie before redirect;
- same-name cookies on two paths and two domains;
- secure cookie over HTTP versus HTTPS;
- expired/deletion cookie;
- multiple Set-Cookie headers;
- request/response hook event log across three hops;
- cancellation while draining an intermediate response.

Run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers

./scripts/check.sh
```

Do not add a new CI job, downstream portfolio, soak framework, or evidence schema.

# Phase completion criteria

Phase 4 is complete only when:

- every Track 1–6 acceptance item is satisfied;
- all redirects are orchestrated through one-hop compatibility dispatch;
- manual redirects expose a usable `next_request`;
- followed redirects preserve correct method/body/header behavior;
- auth works identically through native, custom, mounted, Mock, WSGI, and ASGI transports;
- one authoritative scoped cookie jar is in use;
- cookies set on intermediate responses affect later hops correctly;
- hooks run once per actual hop in the reference order;
- history responses are readable but resource-closed;
- sync and async required differential tests pass with zero skips and zero xfails;
- no new CI architecture was introduced.

## Stop conditions

Stop and record a blocker if:

- native compatibility dispatch cannot disable its internal cookie jar while a facade jar is authoritative;
- no lossless scoped-cookie bridge can be exposed without a substantial core redesign;
- the native response stream cannot be drained and released while retaining buffered history content;
- one-hop redirects cannot preserve request-body replay state;
- exact HTTPX event order conflicts with unavoidable native callbacks.

Do not paper over a state split with dictionary snapshots. If cookie or per-hop state cannot be made authoritative, lower the compatibility claim for that surface until the architecture is corrected.