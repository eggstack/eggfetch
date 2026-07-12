# Post-Milestone J Tightening Plan

## Objective

Perform a focused corrective and validation pass after Milestones F-J. The repository now has a substantial Rust core, sync and async Python clients, a response compatibility surface, common request body arguments, and redirect support. Before adding cookies, auth, proxies, multipart, HTTP/2, or CLI breadth, tighten the behavior that has already been exposed.

This pass should prioritize semantic correctness, lifecycle safety, packaging reliability, and honest compatibility claims. It should not expand the feature surface except where required to make an existing public API real rather than nominal.

## Current state assumption

The repository currently provides:

- async Rust HTTP engine over tokio, hyper/hyper-util, and rustls
- true streamed request-body support in the Rust core
- response streaming with pool leases held through body lifetime
- per-chunk read timeout enforcement in the Rust core
- sync Python API with top-level helpers and `Client`
- async Python API with `AsyncClient`
- Python response properties and buffered iterators
- Python request kwargs for `content`, `data`, and `json`
- redirect policy, redirect history, method rewriting, and sensitive-header stripping
- maturin/PyO3 package structure
- extensive Rust and Python tests

## Completion status

The following tracks have been completed in the corrective pass:

- **Track A**: Implemented true Python network streaming. `client.stream()` and `async_client.stream()` return `StreamingResponse` context managers that consume the live Rust body incrementally. Sync iterators release the GIL per chunk; async iterators use the PyO3 asyncio bridge with cancellation safety. Pool leases are held until exhaustion, close, or drop. 9 required streaming tests implemented (first-chunk-before-complete, large-response-not-buffered, early-break permit release, read-timeout exception, client-reusable-after-error, split UTF-8, cross-chunk line delimiters, double-consumption, use-after-close). Named exceptions `StreamConsumed`, `StreamClosed`, `ResponseNotRead` added.
- **Track B**: Fixed redirect body buffering. When `follow_redirects=False` (the default), request body is no longer eagerly buffered, preserving streaming body semantics for ordinary requests. Cookie audit tests added (raw header isolation, kwarg non-persistence, client cookie persistence).
- **Track C**: Audited and confirmed. Total timeout already uses `start_time.elapsed()` for a deadline across the full redirect chain. Auth audit: 17 Rust + 14 Python tests. Cross-origin redirect auth bug fixed (client auth no longer reapplied on cross-origin hops). `NOAUTH` sentinel added to Python API for per-request auth disable.
- **Track D**: Audited and confirmed. History entries use drained bodies with empty bytes, releasing pool permits. No code changes needed.
- **Track E**: Audited and confirmed. Charset-aware decoding via `encoding_rs` works correctly with case-insensitive parsing, quoted values, and UTF-8 fallback. No code changes needed.
- **Track F**: Clippy fixes applied. All feature-gated builds verified (default, no-default-features, tls-rustls only, all-features). CI configuration exists but Python matrix and wheel builds remain for future work.
- **Track G**: Documentation updated. Architecture overview now correctly documents cross-origin auth behavior, `NOAUTH` sentinel, Python streaming API, cookie semantics, and pipeline order.
- **Track H**: Updated `.gitignore` to cover `*.egg-info/`, `.maturin/`, `.pytest_cache/`, and `*.pyo`.
- **Track I**: Fixed async response construction. `AsyncClient` now uses `PyResponse::from_core_response_with_body()` (same as sync path), fixing a history bug where async responses discarded redirect history. Also made `AsyncClient::close()` idempotent to match `Client::close()`.
- **Track J**: Documentation truth pass. Removed false `aiter_*` claims, updated async adapter description, added tightening plan to repo layout and further reading.
- **Cookie/Auth Interaction (Track D continued)**: `TestAuthDisableDoesNotDisableCookies` added. 30 parity tests covering cookies, auth, and streaming.

Tracks remaining: CI Python matrix and wheel builds (Track F continuation).

## Main risks to close

The remaining risks are:

1. CI only runs on ubuntu-latest with a single Python version; no multi-platform or Python 3.10-3.13 matrix validation.
2. No wheel builds or wheel smoke tests.
3. A Python virtual environment was committed and later removed, leaving possible repository-history bloat or local-path leakage.

## Non-goals

Do not implement cookies, auth, proxies, multipart, retries, HTTP/2, HTTP/3, SOCKS, compression, ASGI/WSGI transports, or advanced CLI behavior in this pass.

Do not redesign the Rust core unless a real correctness defect requires it.

Do not promise drop-in HTTPX or requests compatibility.

Do not add broad dependencies merely for convenience.

# Track A: Real Python response streaming

## Problem

The Python API currently exposes `iter_bytes`, `iter_text`, `iter_lines`, `aiter_bytes`, `aiter_text`, and `aiter_lines`, but these operate over already buffered response content. This is useful for API shape, but it is not equivalent to HTTPX-style network streaming and can mislead users about memory behavior.

## Target behavior

Add explicit streaming request entry points:

```python
with client.stream("GET", url) as response:
    for chunk in response.iter_bytes():
        ...

async with client.stream("GET", url) as response:
    async for chunk in response.aiter_bytes():
        ...
```

The response body must remain in Rust and be consumed incrementally.

The pool lease must remain held until the streaming response is exhausted, closed, or dropped.

Buffered request methods such as `client.get()` should continue returning buffered responses for familiar semantics.

## API design

Add separate response classes only if needed:

- `Response` for buffered responses
- `StreamingResponse` for live bodies

Alternatively, use one `Response` class with explicit body state. A separate type may be safer initially because sync and async live-body ownership differ significantly.

Recommended initial API:

```python
client.stream(method, url, **kwargs) -> sync context manager
async_client.stream(method, url, **kwargs) -> async context manager
```

Do not allow a streaming response to outlive its client silently unless the Rust core safely owns all required state.

## Sync bridge

The synchronous iterator must block on the async Rust body one chunk at a time while releasing the GIL.

Do not create a fresh runtime per chunk. Use the runtime owned by `Client`.

Each `__next__` call should:

1. release the GIL
2. block on the next Rust body chunk
3. map Rust errors to Python exceptions
4. return Python bytes

The iterator must close the body on exhaustion or error.

## Async bridge

The async iterator should convert each Rust body-chunk future into a Python awaitable using the established PyO3 async bridge.

Cancellation must drop the pending Rust future and eventually release the body lease.

## Content access rules

For live streaming responses:

- `.content`, `.text`, and `.json()` should either consume-and-buffer the remaining body or raise a clear `ResponseNotRead`/`StreamConsumed` error.
- choose one policy and document it.

Recommended:

- `read()` / `aread()` consumes and caches the remaining body
- `.content`, `.text`, and `.json()` require the body to have been read
- after iteration has partially consumed the body, `read()` may consume the remainder but should not pretend the full original body is available unless prior chunks were cached

Keep behavior explicit rather than surprising.

## Line/text iteration

Implement line buffering across chunk boundaries.

Text iteration requires an incremental decoder. Do not decode each chunk independently because multibyte encodings can split across chunk boundaries.

Use a small internal incremental decoder or initially support UTF-8 incremental decoding only, with other encodings deferred. Do not misuse `encoding_rs` in a stateless per-chunk way.

## Tests

Required sync tests:

- server sends many chunks slowly
- first Python chunk arrives before server finishes sending body
- memory does not require buffering entire body
- pool permit remains held until response close/exhaustion
- breaking iteration and closing response releases permit
- read timeout surfaces during iteration
- double iteration or iteration after close errors clearly

Required async tests:

- `async with client.stream()` works
- first chunk arrives incrementally
- cancellation during `aiter_bytes()` releases resources
- after cancellation, another request succeeds
- `aiter_lines()` handles boundaries across chunks

## Acceptance criteria

- Python streaming APIs consume the live Rust body rather than buffered bytes.
- Existing buffered request methods remain stable.
- Documentation clearly distinguishes buffered and streamed responses.
- At least one test proves first-chunk delivery before full-body completion.

# Track B: Redirect and request-body replay semantics

## Problem

The redirect implementation reports body buffering in the redirect loop. This can undermine true streamed uploads and can blur the distinction between replayable and non-replayable bodies.

## Target behavior

Redirect handling must not eagerly buffer all request bodies merely because redirects are enabled.

Rules:

- `Empty` and `Bytes` bodies are replayable.
- form and JSON bodies generated as bytes are replayable.
- live stream bodies are non-replayable unless explicitly constructed with a replay factory.
- 301/302/303 rewrites that drop the body do not require replaying it.
- 307/308 require replayability if a resend is needed.
- non-replayable bodies on 307/308 should fail with a specific redirect/body replay error.

## Refactor direction

Represent replayability explicitly in the request body model.

Possible shape:

```rust
pub enum RequestBody {
    Empty,
    Bytes(Bytes),
    Stream {
        stream: BoxBytesStream,
        length: Option<u64>,
    },
}

impl RequestBody {
    pub fn try_clone_for_redirect(&self) -> Result<Self, Error>;
}
```

Because streams cannot generally be cloned, redirect execution should keep a replayable template only for replayable bodies.

Do not buffer a stream just to make redirects possible unless the caller explicitly opts into buffering.

## Redirect execution model

Before first send, derive a redirect request template containing:

- method
- URL
- headers
- replayable body template, if available
- timeout/deadline context

For each hop:

- construct the next body only if needed
- do not retain prior live body streams after send
- drain or close redirect response bodies before next hop

## Python behavior

When a user sends a non-replayable stream body and receives a 307/308 while `follow_redirects=True`, raise a clear exception such as `RedirectBodyUnavailable` or a redirect-specific `RequestError`.

Do not silently disable redirects or buffer the body.

## Tests

- streamed POST without redirect remains truly streaming when `follow_redirects=True`
- 303 after streamed POST rewrites to GET without attempting replay
- 307/308 after byte body replays exactly
- 307/308 after stream body raises specific error
- redirect response body is released before next hop
- no pool permit leak across multi-hop redirects

## Acceptance criteria

- Redirect support does not disable constant-memory upload streaming for ordinary requests.
- Replayability is explicit and tested.
- 307/308 never silently drop or buffer a non-replayable body.

# Track C: Timeout semantics across redirect chains

## Problem

A total timeout may be applied independently per hop rather than across the entire logical request. That can allow a redirect chain to exceed the requested wall-clock limit.

## Target behavior

`Timeout.total` should apply to the full logical request, including:

- initial pool acquisition
- initial request
- redirect response processing
- all redirect hops
- final response buffering for buffered APIs

For streaming APIs, define whether total timeout ends at response headers or continues through body consumption.

Recommended policy:

- buffered request methods: total timeout covers the entire redirect chain and full body buffering
- streamed request methods: total timeout covers redirect resolution and final response headers; per-chunk read timeout governs body consumption unless a separate streaming deadline is later added

## Implementation direction

Convert total timeout duration into a deadline at logical request start.

For each operation/hop, calculate remaining duration:

```rust
remaining = deadline.saturating_duration_since(Instant::now())
```

Return `TimeoutPhase::Total` when the deadline is exhausted.

Do not reset the total timer for each redirect hop.

## Tests

- two redirect hops individually under timeout but collectively over total timeout should fail
- short redirect chain under deadline succeeds
- pool wait on later hop counts against same total deadline
- final response buffering counts against total deadline
- streamed final response header acquisition counts against deadline

## Acceptance criteria

- Total timeout is a logical-request deadline rather than a per-hop duration.
- Docs state exact buffered versus streaming scope.

# Track D: Redirect history correctness

## Problem

`response.history` now exists, but history object semantics may be unclear. It is important to avoid retaining active body streams or large unnecessary bodies for every redirect response.

## Target behavior

Each history entry should be a closed, immutable response snapshot containing:

- status code
- reason phrase
- headers
- URL
- HTTP version
- body only if deliberately buffered under a documented limit or already consumed

History responses must not hold live pool leases.

## Tasks

Audit Rust `Response.history` ownership.

Ensure redirect response bodies are consumed or discarded safely before next hop.

Decide body policy:

- preferred initial policy: history bodies are empty or limited metadata-only snapshots
- alternative: buffer redirect bodies up to a small configurable/internal cap

Do not accidentally retain unbounded redirect bodies.

Python `history` should return stable `Response` objects whose `.close()` is harmless.

## Tests

- history order is oldest to newest
- history entries contain correct URLs/statuses
- history entries hold no active pool permit
- large redirect response body is not retained unboundedly
- final response references correct history count

## Acceptance criteria

- Redirect history is lifecycle-safe and memory-bounded.
- History behavior is documented.

# Track E: Python response decoding correctness

## Problem

`encoding_rs` was added for charset-aware decoding. This is useful but needs a precise policy, especially for unknown labels, malformed `Content-Type`, BOM handling, and streaming text decoding.

## Tasks

Document and test decoding precedence:

1. explicit user-set `response.encoding`
2. `charset=` from `Content-Type`
3. BOM if supported
4. UTF-8 fallback

Decide unknown charset behavior:

- fallback to UTF-8 with replacement, or
- raise `LookupError`

Prefer predictable compatibility over silent ambiguity.

Ensure charset parsing is case-insensitive and tolerates quoted values.

Do not treat all invalid bytes as fatal unless documented.

## Tests

- UTF-8
- ISO-8859-1 / Windows-1252
- quoted charset
- uppercase `CHARSET`
- unknown charset
- malformed charset
- explicit encoding override
- multibyte characters split across streamed chunks once true streaming text iteration exists

## Acceptance criteria

- Decoding policy is deterministic and documented.
- Buffered and streaming text paths agree.

# Track F: Package and wheel validation

## Objective

Prove the Python package builds and imports across supported Python versions and common operating systems.

## Supported versions

Choose an explicit initial Python support range. Recommended starting point:

- Python 3.10-3.13

Add 3.14 only when PyO3/maturin support is confirmed.

## CI matrix

Add a dedicated Python workflow or expand CI with:

- Linux x86_64
- macOS arm64/x86_64 as practical
- Windows x86_64
- Python 3.10, 3.11, 3.12, 3.13

At minimum, run source builds and tests. Add wheel builds with maturin.

Recommended tools:

- `PyO3/maturin-action`
- `actions/setup-python`

Keep workflow dependencies pinned to major or commit according to project policy.

## Wheel checks

For each built wheel:

- install into a clean virtual environment
- import `eggfetch`
- run a smoke request against local server
- run core Python tests where feasible
- verify no local absolute paths are embedded in package metadata

## Package metadata

Audit `pyproject.toml`:

- package name
- version source
- classifiers
- Python requirement
- license
- repository URL
- README
- extension module path
- package inclusion

Ensure the Rust crate and Python package versions are coordinated.

## Acceptance criteria

- Clean wheel builds succeed for declared Python versions/platforms.
- Installed wheels import and pass smoke tests.
- Supported Python version policy is documented.

# Track G: CI visibility and branch health

## Problem

No combined commit statuses were visible through the connector. The repository may still have Actions runs, but this should be verified.

## Tasks

Inspect `.github/workflows/ci.yml` and repository Actions settings.

Ensure push and pull request triggers work on `main`.

Add separate jobs if useful:

- Rust fmt/clippy/test/doc
- Python source build/test
- wheel build matrix

Ensure failing Python tests fail the workflow.

Add status badges only after workflow names stabilize.

Consider branch protection requiring the core checks once CI is proven stable.

## Acceptance criteria

- New commits visibly trigger CI.
- Rust and Python validation are represented by required checks or documented checks.

# Track H: Repository hygiene and virtual-environment history audit

## Problem

A `.venv311` directory was accidentally committed and later removed. Although the working tree is clean, Git history may contain significant generated files, absolute local paths, or unexpected secrets.

## Tasks

Audit the offending commit contents.

Search history for:

- tokens
- credentials
- `.env` files
- private keys
- pip configuration
- local absolute paths
- certificates

If no secrets are present, decide whether repository bloat justifies history rewrite.

Measure repository size and the size contribution of `.venv311` objects.

If history rewrite is warranted:

- use `git filter-repo` to remove `.venv311/`
- coordinate force-push carefully
- document that collaborators must reclone or reset

Do not rewrite history solely for cosmetic reasons without considering disruption.

Ensure `.gitignore` covers:

```text
.venv/
.venv*/
venv/
__pycache__/
*.py[cod]
*.so
*.dylib
*.pyd
.pytest_cache/
.maturin/
target/
```

## Acceptance criteria

- History is audited for secrets.
- Repository size impact is known.
- Either history is cleaned or the decision not to rewrite is documented.

# Track I: Compatibility and API audit

## Objective

Review the Python API now that F-J have landed quickly.

## Audit areas

Sync and async parity:

- same method names
- same kwargs
- same defaults
- same exception types
- same redirect behavior

Top-level helpers versus clients:

- same kwarg normalization
- same timeout behavior
- same redirect defaults

Response behavior:

- buffered versus streaming clearly separated
- `close()` and `aclose()` semantics
- `history`
- repeated `.content`, `.text`, `.json()` access
- `raise_for_status()` context

Headers:

- duplicate header handling
- `get_list()`
- case-insensitive behavior
- non-UTF-8 header value behavior

Exceptions:

- Python hierarchy matches Rust error taxonomy
- `TooManyRedirects`
- replay errors
- timeout phase mapping
- cancellation behavior

## Differential tests

Add selected behavioral comparisons against HTTPX and requests for common stable cases:

- params encoding
- form encoding
- JSON body
- redirect method rewriting
- cross-origin auth stripping
- status helpers
- header lookup

Do not require exact parity where eggfetch intentionally differs. Encode expected differences explicitly.

## Acceptance criteria

- Public API inconsistencies are fixed or documented.
- Compatibility claims are backed by tests.

# Track J: Documentation truth pass

Review and update:

- README.md
- AGENTS.md
- CONTRIBUTING.md
- docs/architecture/overview.md
- docs/architecture/dependency-policy.md
- docs/architecture/feature-flags.md
- plans/ROADMAP.md

Required corrections:

- distinguish buffered iterators from true network streaming until Track A lands
- state redirect replayability limits
- state timeout scope across redirects
- state supported Python versions
- state wheel/platform status
- state current CLI status
- avoid calling the project drop-in compatible

Add a concise compatibility matrix for requests/httpx-like features.

# Suggested implementation order

1. Audit redirect request-body buffering and replayability.
2. Fix redirect total-timeout deadline behavior.
3. Tighten redirect history lifecycle.
4. Implement true sync Python streaming.
5. Implement true async Python streaming and cancellation.
6. Align text decoding between buffered and streaming paths.
7. Audit Python API parity and exceptions.
8. Add Python/wheel CI matrix.
9. Audit repository history and `.venv311` impact.
10. Complete documentation truth pass.

# Final acceptance criteria

This tightening pass is complete when:

- Python streaming APIs operate on live Rust response bodies, or buffered iterator methods are renamed/limited so they cannot be mistaken for network streaming.
- Redirect handling preserves true request streaming and rejects non-replayable 307/308 resends safely.
- Total timeout applies across the full redirect chain for buffered requests.
- Redirect history entries are closed and memory-bounded.
- Response decoding policy is deterministic and tested.
- Sync and async Python APIs have matching supported kwargs and errors.
- Wheels build and import on the declared Python/platform matrix.
- CI checks are visible and reliable.
- The accidental virtual-environment commit has been audited for secrets and bloat.
- Documentation accurately describes current capabilities and limitations.

## Required validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd crates/eggfetch-python
maturin develop
python -m pytest
maturin build
```

Also run clean-wheel smoke tests in isolated environments rather than relying only on `maturin develop`.

## Handoff note

Do not begin broad feature expansion until this pass is complete. The codebase now exposes enough public Python behavior that lifecycle, timeout, redirect, and packaging defects will become increasingly expensive to correct later.
