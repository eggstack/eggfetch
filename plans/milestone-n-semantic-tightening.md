# Milestone N Plan: Semantic Tightening and Public-API Stabilization

## Objective

Stabilize all public behavior introduced through Milestones A-J before adding broad feature expansion. This milestone converts nominal or partially implemented behavior into tested, lifecycle-safe semantics and establishes a reliable release baseline for the next feature phase.

The primary standard is simple:

> Every exposed API must behave exactly as documented under normal use, cancellation, timeout, redirect, partial-consumption, and error conditions.

## Scope

Milestone N includes:

- true Python network streaming
- redirect/body replay correctness
- total timeout across redirect chains
- redirect history lifecycle and memory bounds
- response text-decoding policy
- sync/async Python API parity
- package and wheel validation
- CI visibility and branch health
- repository history hygiene audit
- compatibility documentation audit

Milestone N does not include:

- cookies
- authentication
- multipart files
- proxies
- compression
- retries
- HTTP/2
- CLI expansion

## Track 1: True Python response streaming

### Required behavior

Buffered methods remain buffered:

```python
response = client.get(url)
response.content
```

Streaming methods expose the live Rust response body:

```python
with client.stream("GET", url) as response:
    for chunk in response.iter_bytes():
        ...

async with async_client.stream("GET", url) as response:
    async for chunk in response.aiter_bytes():
        ...
```

### Sync implementation

The sync client-owned runtime must drive body chunks one at a time.

Each iterator advance must:

- release the GIL
- block on one Rust body read
- preserve per-chunk read timeout
- map Rust errors to Python exceptions
- release the pool/body lease on completion, error, explicit close, or object drop

Do not create a runtime per chunk.

### Async implementation

Use the existing PyO3 asyncio bridge.

Cancellation must drop the pending Rust future and release resources. After cancellation, the client must remain usable.

### Body-state policy

Define explicit states:

- unread
- actively streaming
- fully buffered/cached
- consumed
- closed

Expose clear errors for invalid transitions. Recommended Python exceptions:

- `ResponseNotRead`
- `StreamConsumed`
- `StreamClosed`

Provide `read()` and `aread()` to consume the remaining live body into the cache.

### Text and line iteration

`iter_lines()`/`aiter_lines()` must handle line delimiters split across chunks.

Text iteration must use an incremental decoder so multibyte characters split across chunks decode correctly.

### Tests

Required:

- first chunk is delivered before complete body arrival
- a large response does not buffer before first iterator result
- sync early-break plus close releases pool permit
- async cancellation releases pool permit
- read timeout occurs while waiting for a later chunk
- `read()`/`aread()` cache content deterministically
- invalid body-state transitions raise documented exceptions
- split multibyte text and split lines decode correctly

## Track 2: Redirect and request-body replay correctness

### Required model

Request replayability must be explicit.

Replayable:

- empty body
- bytes body
- JSON/form bodies represented as bytes

Non-replayable by default:

- live upload streams

### Redirect rules

- 301/302 POST rewrite to GET and drop body according to current policy
- 303 non-HEAD rewrite to GET and drop body
- 307/308 preserve method and require replayable body
- non-replayable stream on 307/308 raises a redirect replay error

Do not eagerly buffer a stream merely because redirect following is enabled.

### Suggested core API

Add or refine:

```rust
RequestBody::is_replayable()
RequestBody::try_clone_for_redirect()
```

Use a replayable request template only where possible.

### Tests

- streamed upload remains lazy with redirects enabled but no redirect received
- 303 after streamed POST does not attempt replay
- 307/308 replays bytes exactly
- 307/308 rejects a live stream specifically
- redirect response body releases its lease before the next hop
- multi-hop redirects do not leak permits

## Track 3: Total timeout across redirect chains

### Required behavior

Convert total timeout duration into one deadline at logical request start.

The same deadline must cover:

- pool acquisition
- initial request
- redirect processing
- all subsequent hops
- final body buffering for buffered request APIs

Do not reset total timeout per redirect hop.

For live streaming responses, document that total timeout covers redirect processing and final headers, while read timeout governs subsequent chunks unless a separate stream deadline is added.

### Tests

- multiple individually-fast hops exceeding the total deadline fail
- short chain succeeds
- pool wait on a later hop counts against original deadline
- final buffered body read counts against original deadline

## Track 4: Redirect history lifecycle

History entries must be closed immutable snapshots.

Each entry may contain:

- status
- URL
- headers
- HTTP version
- reason phrase

Avoid retaining active body streams or unbounded bodies.

Recommended initial policy: metadata-only history bodies, or a documented small bounded body cap.

Tests must prove history entries hold no active pool permit.

## Track 5: Response decoding policy

Define and test precedence:

1. explicit response encoding override
2. `charset=` parameter from Content-Type
3. BOM if deliberately supported
4. UTF-8 fallback

Charset matching should be case-insensitive and support quoted labels.

Unknown charset behavior must be deterministic. Recommended: UTF-8 replacement fallback with documented behavior, unless closer HTTPX compatibility is chosen.

Buffered and streaming text paths must agree.

## Track 6: Sync/async API parity

Create a parity matrix covering:

- method names
- request kwargs
- defaults
- timeout behavior
- redirects
- response properties
- exceptions
- close semantics

Fix unintended differences.

Add shared parameterized Python tests where possible so sync and async paths exercise the same cases.

## Track 7: Wheel and packaging validation

Declare a Python support range. Recommended initial target:

- Python 3.10-3.13

Build/test wheels for:

- Linux x86_64
- macOS arm64 and x86_64 where available
- Windows x86_64

Each wheel must be installed in a clean environment and pass an import plus local-server request smoke test.

Audit `pyproject.toml` metadata and version coordination.

## Track 8: CI visibility

Ensure GitHub Actions visibly run on push and pull request.

Required checks:

- Rust fmt
- Rust clippy
- Rust tests
- Rust docs
- Python extension build
- Python tests
- wheel smoke builds

After stability, consider branch protection requiring these checks.

## Track 9: Repository hygiene

Audit the accidentally committed `.venv311` history for:

- credentials
- private keys
- environment files
- local paths
- package configuration

Measure repository bloat.

Rewrite history only if secrets or meaningful bloat justify the disruption. Document the decision.

## Track 10: Documentation truth pass

Update README, architecture docs, roadmap, contribution docs, and Python docs.

Clearly distinguish:

- buffered requests
- live streaming requests
- timeout scopes
- redirect replay limitations
- supported Python/platform matrix
- tested compatibility versus planned compatibility

## Acceptance criteria

Milestone N is complete when:

- Python sync/async live streaming is implemented and tested
- redirect following does not eagerly buffer live uploads
- non-replayable 307/308 redirects fail safely
- total timeout is one deadline across redirect chains
- redirect history is closed and memory-bounded
- response decoding is deterministic
- sync/async APIs match intentionally
- declared wheels build and import cleanly
- CI checks are visible
- repository history has been audited
- documentation matches behavior

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd crates/eggfetch-python
maturin develop
python -m pytest
maturin build
```

Run wheel smoke tests in clean isolated environments.

## Handoff note

Do not begin Milestone O until Milestone N closes the lifecycle and compatibility gaps. Cookies depend on correct redirect, header, client-state, and response-history behavior.
