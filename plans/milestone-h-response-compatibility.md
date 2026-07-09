# Milestone H Plan: Response Compatibility Surface

## Objective

Expand the Python `Response` surface toward familiar requests/httpx semantics while preserving accurate Rust-core ownership and streaming behavior. This milestone should make response objects useful for common Python client workflows without pretending full HTTPX parity.

The key goal is to establish response behavior that will remain stable: cached content, text decoding, JSON parsing, streaming iteration, status helpers, history placeholders, and clear error behavior.

## Prerequisites

Milestones F and G should be complete or sufficiently stable. The core response body lifecycle and pool lease behavior should already be hardened.

If streaming response pool leases or body read timeouts remain incomplete, either finish them first or defer Python streaming methods in this milestone.

## Scope

Milestone H includes:

- richer Python `Response` properties
- cached `.content`
- `.text` with explicit decoding behavior
- `.json()`
- `.raise_for_status()` refinement
- `.is_success`, `.is_error`, `.is_redirect`, `.is_client_error`, `.is_server_error`
- response header mapping improvements
- sync byte/text/line iterators if streaming is stable
- async byte/text/line iterators if async streaming is stable
- response repr/debug behavior
- response tests

Milestone H does not include:

- redirect engine implementation unless Milestone J has landed first
- cookie jar behavior
- decompression unless core already supports it
- charset detection beyond a simple documented policy
- full HTTPX `Response` parity

## Response property target

Implement or refine:

```python
response.status_code
response.reason_phrase
response.headers
response.url
response.content
response.text
response.encoding
response.elapsed  # optional, only if measured honestly
response.http_version
response.request  # optional placeholder if Request wrapper exists
response.history  # empty until redirects land
```

Status helpers:

```python
response.is_informational
response.is_success
response.is_redirect
response.is_client_error
response.is_server_error
response.is_error
```

Methods:

```python
response.json()
response.raise_for_status()
response.iter_bytes()
response.iter_text()
response.iter_lines()
response.close()
```

Async methods if streaming supported:

```python
response.aiter_bytes()
response.aiter_text()
response.aiter_lines()
response.aclose()
```

## Content caching policy

For non-streaming requests, `.content` should be cached bytes.

Repeated access to `.content`, `.text`, and `.json()` should not re-read the Rust body.

For streaming responses:

- before streaming begins, `.content` may consume the stream and cache it, if that is the chosen policy
- after streaming has begun, `.content` should raise a clear stream-consumed error unless the full body was cached
- after `.content` has been read, iterators may iterate over cached content if implemented

Choose one policy and document it. Familiar Python behavior favors caching once fully read.

## Text decoding policy

Implement a conservative policy first:

1. If `response.encoding` is explicitly set by user code, use it.
2. Else parse `charset=` from `Content-Type` if present.
3. Else default to UTF-8.

Do not add charset-normalizer/chardet in this milestone. Those add dependency and behavior complexity.

Invalid bytes should decode using a documented error policy. Options:

- strict decode and raise `UnicodeDecodeError`
- replacement decode

HTTPX generally tries to be user-friendly. For early eggfetch, replacement decode may be acceptable for `.text`, while a stricter helper can come later. Document the decision.

## JSON parsing

Implement:

```python
response.json(**kwargs)
```

Use Python's standard `json` module from the Python layer or parse in Rust only if the project has deliberately enabled JSON support. For compatibility and dependency minimization, prefer Python `json.loads(response.text)` initially.

Should support common kwargs if easy:

- `encoding` is not needed if `.text` handles decoding
- `parse_float`, `parse_int`, `object_hook`, etc. can be passed through only if using Python json directly

If kwargs are unsupported, fail loudly.

## Header mapping improvements

`response.headers` should be mapping-like:

- case-insensitive lookup
- `.get(name, default=None)`
- `.items()`
- `.keys()`
- `.values()`
- `name in headers`
- `len(headers)`

Multi-value behavior must be explicit. If duplicate response headers are joined, document the joining policy. For `Set-Cookie`, do not collapse into an ambiguous comma-joined value if avoidable.

Consider adding:

```python
headers.get_list("set-cookie")
```

This is important for later cookies.

## Streaming iterators

Only implement Python response streaming if the Rust core correctly holds leases through body lifetime.

Sync API:

```python
with eggfetch.Client() as client:
    with client.stream("GET", url) as response:
        for chunk in response.iter_bytes():
            ...
```

Async API:

```python
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", url) as response:
        async for chunk in response.aiter_bytes():
            ...
```

If `client.stream()` is too large for this milestone, implement iterators only over cached content and defer true network streaming. Do not expose fake streaming.

Line iteration should handle boundaries across chunks. It should not assume each chunk is a line.

## raise_for_status

`raise_for_status()` should raise a Python `HTTPStatusError` for 4xx and 5xx.

The exception should include:

- status code
- reason phrase if available
- URL
- response object
- request object if available later

Do not raise for 1xx, 2xx, or 3xx unless following HTTPX semantics requires a narrower interpretation. Document the behavior.

## Repr/debug behavior

Implement useful `repr(response)`:

```python
<Response [200 OK]>
```

Keep this simple and deterministic.

## Tests

Required Python tests:

- `.status_code` works
- `.headers` case-insensitive lookup works
- `.headers.get_list()` if implemented
- `.content` is cached and stable
- `.text` decodes UTF-8
- `.text` respects charset header
- `.json()` parses JSON response
- `.json()` propagates invalid JSON errors
- `.raise_for_status()` raises on 404/500
- status helpers classify 1xx/2xx/3xx/4xx/5xx correctly
- duplicate response headers are handled as documented
- `repr(response)` is useful

Streaming tests if implemented:

- `iter_bytes()` yields chunks
- `iter_lines()` handles split lines across chunks
- consuming stream then accessing `.content` follows documented policy
- closing stream releases connection/pool permit
- async iterators work under `asyncio`

## Documentation

Update README and Python docs with:

- response property table
- content caching policy
- text decoding policy
- JSON parsing behavior
- streaming status
- differences from requests/httpx

## Acceptance criteria

Milestone H is complete when:

- Python `Response` supports common status/header/body operations
- content/text/json behavior is cached and deterministic
- status helper behavior is tested
- header duplicate behavior is documented and tested
- streaming methods are either real and tested or deliberately absent
- `raise_for_status()` provides useful exception context
- sync and async clients return compatible response objects

Required validation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd crates/eggfetch-python && maturin develop && python -m pytest
```

## Handoff notes

This milestone should avoid scope creep into request construction. Keep focus on response semantics. Request-side compatibility is Milestone I.
