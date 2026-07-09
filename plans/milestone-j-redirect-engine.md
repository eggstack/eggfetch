# Milestone J Plan: Redirect Engine

## Objective

Implement a correct, configurable redirect engine in `eggfetch-core` and expose it through the Python API with familiar requests/httpx-style controls. Redirect behavior is security-sensitive because it can alter methods, resend bodies, and leak credentials across origins if implemented carelessly.

The redirect engine must live in the Rust core. Python should only configure redirect policy and expose redirect history.

## Prerequisites

Milestones F-I should be complete enough that Python clients and request construction are stable.

Request body replayability must be represented correctly. Redirects cannot safely resend non-replayable streaming bodies unless explicitly buffered or rejected.

Origin-key handling and header safety should already be hardened.

## Scope

Milestone J includes:

- core redirect policy type
- redirect following in `eggfetch-core`
- max redirect enforcement
- redirect loop detection
- method rewrite/preservation rules
- body replayability handling
- cross-origin sensitive header stripping
- response history
- Python `follow_redirects` / `allow_redirects` mapping
- redirect tests

Milestone J does not include:

- cookies beyond preserving existing explicit headers
- auth flows beyond stripping sensitive headers safely
- HSTS
- browser-equivalent redirect policy
- proxy-specific redirect behavior
- full HTTPX parity for every redirect edge case

## Policy model

Add a core redirect configuration type.

Suggested shape:

```rust
pub struct RedirectPolicy {
    pub follow: bool,
    pub max_redirects: usize,
}
```

Default should be deliberate.

HTTPX default behavior is not to follow redirects unless configured. requests follows redirects for most methods by default. Since eggfetch aims to be familiar to both but closer to HTTPX in architecture, choose and document the default carefully.

Recommended default:

- Rust core: `follow = false`
- Python top-level/client: default `follow_redirects=False` initially, matching HTTPX
- Provide `allow_redirects` alias later only if requests compatibility is explicitly desired

## Core redirect loop

Redirect handling should occur inside `Client::send` or a dedicated request execution pipeline.

High-level algorithm:

1. Send request.
2. If response status is redirect and policy follow is enabled:
   - inspect `Location`
   - resolve relative location against current URL
   - validate scheme
   - decide method/body for next request
   - strip sensitive headers if origin changes
   - append prior response to history
   - enforce max redirects
   - detect loops
   - resend if safe
3. Return final response with history.

Use an internal function to keep `Client::send` readable.

## Redirect status codes

Handle common redirect codes:

- 301 Moved Permanently
- 302 Found
- 303 See Other
- 307 Temporary Redirect
- 308 Permanent Redirect

Possible 300 Multiple Choices can be left as non-followed unless intentionally supported.

## Method rewrite rules

Implement conservative browser/requests/httpx-compatible behavior:

- 303: switch method to GET and drop body except for HEAD, which remains HEAD if that is the chosen compatibility policy.
- 301/302: for POST, commonly switch to GET and drop body; for other methods, preserve or follow HTTPX behavior. Decide and document.
- 307/308: preserve method and body.

The exact behavior should be tested and documented.

Recommended initial policy:

```text
301/302: POST -> GET, body dropped; other methods preserved
303: all non-HEAD -> GET, body dropped; HEAD preserved
307/308: method and body preserved
```

## Body replayability

Redirects that need to resend a body require replayable bodies.

Rules:

- Empty and byte bodies are replayable.
- Buffered JSON/form/content bodies are replayable.
- True streaming request bodies are not replayable unless explicitly buffered or constructed as replayable.
- If a redirect requires resending a non-replayable body, return a clear error.
- If redirect rewrites method to GET and drops body, replayability is not required for the next request, but the original body may already have been sent.

Add an error variant such as:

```rust
Error::BodyNotReplayableForRedirect
```

or a more general redirect error.

Do not silently drop a body for 307/308.

## Location handling

Support:

- absolute URLs
- relative paths
- scheme-relative URLs if `url` crate resolves them safely
- percent-encoded locations

Reject:

- unsupported schemes
- malformed locations
- URLs without a host when required

Fragments should not be sent on the wire. Follow URL library behavior but test it.

## Header safety

On cross-origin redirects, strip sensitive headers.

At minimum strip:

- `Authorization`
- `Proxy-Authorization`
- `Cookie`

Consider also stripping or recomputing:

- `Host`
- `Content-Length`
- `Transfer-Encoding`

`Host` should not be blindly carried across redirects.

For same-origin redirects, preserve ordinary headers unless method/body rewrite requires body headers to change.

When body is dropped, remove body-specific headers:

- `Content-Length`
- `Content-Type` maybe if it was body-specific
- `Transfer-Encoding`

Be conservative. Avoid sending stale body headers on a GET after POST redirect.

## History model

Add response history to core `Response`.

History should contain prior redirect responses in order.

Each history response should include:

- status
- headers
- URL
- possibly no body or a drained/cached small body depending on implementation

For Python compatibility, `response.history` should be a list of `Response` objects. Decide whether history bodies are accessible. Simpler initial behavior: history responses have metadata and buffered body if already read, but no active stream.

Avoid retaining active response streams in history.

## Loop detection

Enforce `max_redirects`.

Additionally detect exact URL loops if easy. Max redirect enforcement is required; explicit loop detection is useful but not a substitute.

Error should include enough context:

- redirect count
- last URL
- max allowed

Python should map to `TooManyRedirects`.

## Timeout interaction

Redirect chains should respect timeout behavior.

Decide whether total timeout applies per individual request or across the entire redirect chain.

Recommended:

- `total` timeout should apply across the entire logical request including redirects.
- phase timeouts apply per hop where implemented.

If current core cannot apply total across redirect chain cleanly, document and implement the best feasible behavior. Do not silently reset total timeout per hop without documenting it.

## Pool interaction

Each redirect hop is a separate request. Ensure response bodies from redirect responses are consumed or dropped safely before next hop.

Do not keep pool permits for prior redirect bodies active into the next hop unless intentionally required.

## Python API

Expose:

```python
eggfetch.get(url, follow_redirects=True)
client = eggfetch.Client(follow_redirects=True, max_redirects=20)
```

Maybe support requests-style alias later:

```python
eggfetch.get(url, allow_redirects=True)
```

If both `follow_redirects` and `allow_redirects` are passed, raise a clear error unless they agree.

Default should be documented.

Expose `TooManyRedirects` exception.

Expose `response.history`.

## Tests

Core Rust tests:

- 301 GET follows to final URL
- 302 GET follows
- 303 POST rewrites to GET and drops body
- 307 POST preserves method and body
- 308 POST preserves method and body
- relative Location resolves correctly
- unsupported scheme errors
- malformed Location errors
- max redirects errors
- loop errors or max redirect catches loop
- cross-origin redirect strips Authorization/Cookie
- same-origin redirect preserves safe headers
- body-specific headers removed when body dropped
- non-replayable streaming body with 307/308 errors
- redirect history recorded in order

Python tests:

- `follow_redirects=False` returns redirect response
- `follow_redirects=True` returns final response
- `response.history` contains prior responses
- `max_redirects` respected
- `TooManyRedirects` raised
- POST 303 behavior visible from server
- sensitive headers stripped cross-origin
- unsupported redirect kwargs fail loudly

## Documentation

Document:

- default redirect behavior
- supported status codes
- method rewrite rules
- sensitive header stripping
- body replayability rules
- timeout behavior across redirect chains
- differences from requests and HTTPX

## Acceptance criteria

Milestone J is complete when:

- redirect policy lives in core
- Python only configures redirect behavior
- max redirect enforcement works
- method rewrite/preservation is tested
- sensitive cross-origin header stripping is tested
- non-replayable body redirects fail safely
- response history is exposed
- docs clearly state defaults and differences

Required validation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd crates/eggfetch-python && maturin develop && python -m pytest
```

## Handoff notes

Redirects are deceptively security-sensitive. Do not optimize for broad compatibility until the safety rules are explicit. Header stripping and body replayability are more important than matching every requests/httpx corner case in the first pass.
