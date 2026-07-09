# Milestone E Plan: Streaming Foundation

## Objective

Build a protocol-neutral streaming foundation for request and response bodies. This milestone should make eggfetch capable of handling large bodies, incremental reads, streamed uploads, cancellation, and future Python sync/async iterator APIs without redesigning the core response model.

Streaming must be implemented in `eggfetch-core`. Python sync and async streaming wrappers later should only adapt this Rust body stream into Python iterator forms.

## Scope

Milestone E includes:

- request body abstraction beyond fixed bytes
- response body stream abstraction
- buffered response collection built on top of streaming
- body ownership and single-consumption rules
- cancellation and drop behavior
- read timeout integration for body chunks
- tests for chunked transfer and large bodies
- groundwork for Python `iter_bytes`, `iter_lines`, `aiter_bytes`, and `aiter_lines`

Milestone E does not include:

- Python bindings
- CLI streaming UI
- decompression unless already trivial
- multipart upload
- WebSockets
- HTTP/3
- full backpressure tuning beyond safe defaults

## Design principles

Streaming is not an optional add-on. It affects connection reuse, timeouts, memory use, cancellation, and Python API compatibility.

The core rule should be: a response body is consumed either by streaming or by buffering, but not by both unless the buffered result is explicitly cached.

A streamed response must keep its connection ownership clear until the body is consumed or dropped. If the body is dropped before being consumed, the underlying connection should be closed or otherwise made safe. Do not reuse a connection with unread bytes unless the implementation explicitly drains them safely.

## Request body model

Expand `Body` to support:

- empty body
- bytes body
- known-size static body
- async stream body

Suggested public shape:

```rust
pub struct Body { /* internal */ }

impl Body {
    pub fn empty() -> Self;
    pub fn from_bytes(bytes: Bytes) -> Self;
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<Bytes>> + Send + 'static;
}
```

If avoiding `futures-core` as a public dependency is preferred, create an internal trait abstraction. However, using the standard ecosystem `Stream` trait may be the pragmatic route.

Track whether the body has a known length.

Required body metadata:

- known length if available
- replayable versus non-replayable
- consumed state if relevant

Replayability will matter later for redirects and retries. A streamed body is generally not replayable.

## Response body model

Separate response metadata from body stream.

A response should expose:

- status
- version
- headers
- final URL if tracked later
- body stream handle

Buffered convenience methods should consume or cache the body:

```rust
let bytes = response.bytes().await?;
let text = response.text().await?;
```

Streaming methods should expose incremental reads:

```rust
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
}
```

The exact names can be adjusted, but the ownership semantics must be explicit.

## Single-consumption semantics

Define behavior when users attempt to consume a body twice.

Options:

1. `bytes().await` consumes body and stores cached bytes for repeated access.
2. `bytes().await` consumes body and subsequent body reads return an error.
3. `Response` has separate `read()` and `into_body()` APIs to make ownership explicit.

For Python compatibility, cached buffered content is useful because requests/httpx users expect repeated access to `.content`, `.text`, and `.json()` after a buffered response. However, for Rust, explicit ownership is cleaner.

Recommended compromise:

- Rust core exposes explicit consumption APIs.
- Python layer later can cache buffered content in the Python wrapper.
- Core may provide a `BufferedResponse` helper if useful.

## Read timeout integration

Milestone D read timeout behavior must apply to body chunks.

Required behavior:

- waiting too long for the next response body chunk triggers `TimeoutPhase::Read`
- chunks arriving within the read timeout keep the stream alive
- total timeout behavior should be documented if active across streaming
- dropping a timed-out body should not leak permits or corrupt the pool

## Backpressure

Avoid eagerly buffering streamed bodies. The stream should pull chunks as the caller requests them.

Do not create unbounded channels between hyper and eggfetch unless there is a clear reason. Preserve natural backpressure from the underlying body where possible.

If a channel is necessary for Python adaptation later, keep it in the Python crate rather than the core unless core semantics require it.

## Chunked transfer

Support HTTP/1.1 chunked responses naturally through hyper body handling. Tests should verify that chunked server responses are surfaced as byte chunks and collect correctly.

Do not promise exact preservation of wire chunk boundaries unless the implementation can guarantee it. HTTP client APIs generally expose body chunks, not necessarily original wire chunks.

## Text and line streaming foundation

The Rust core should focus on bytes. Text decoding and line iteration can be helper layers.

Future Python APIs will need:

```python
for chunk in response.iter_bytes(): ...
for line in response.iter_lines(): ...
async for chunk in response.aiter_bytes(): ...
async for line in response.aiter_lines(): ...
```

Prepare by keeping byte streaming clean and by avoiding assumptions that all bodies are UTF-8.

Line iteration can be added later using an incremental decoder. Do not block Milestone E on perfect text streaming.

## Connection reuse interaction

Document and test response body lifecycle effects:

- fully consumed body may allow reuse
- dropped unread body should close/discard connection
- body read error should close/discard connection
- timeout while reading should close/discard connection

This is essential for correctness and for avoiding cross-response contamination.

## Request streaming interaction

Streamed uploads should support unknown length. If length is unknown, the engine may use chunked transfer for HTTP/1.1 where appropriate.

Known-length streamed bodies can set `Content-Length` if the abstraction supports it.

For MVP, implement whichever streamed request body path is reliable. If streamed uploads are too much for this milestone, implement the type boundary and response streaming first, then document request streaming as partial.

## Tests

Required tests:

- large response can be streamed without full buffering
- large response can be buffered successfully
- chunked response collects correctly
- chunked response streams incrementally
- delayed chunk triggers read timeout
- chunks within read timeout do not fail
- dropping unread body prevents unsafe connection reuse
- body cannot be consumed twice unless explicitly cached
- response body error maps to stable eggfetch error

Preferred tests:

- streamed request body reaches server correctly
- unknown-length request body uses safe transfer behavior
- cancellation during response stream releases pool state
- cancellation during request stream releases pool state

## Documentation

Document:

- body ownership rules
- buffering versus streaming
- memory implications
- connection reuse implications
- timeout semantics during streaming
- replayability of request bodies
- future Python mapping

## Acceptance criteria

Milestone E is complete when:

- response body streaming is available in `eggfetch-core`
- buffered reads are implemented on top of streaming or share the same body path
- large response tests avoid full eager buffering before caller consumption
- read timeout applies to body chunks
- dropped or partially consumed bodies are handled safely
- connection reuse behavior after body consumption/drop is tested
- request body abstraction is ready for streamed uploads, even if full streamed upload support is limited
- public docs clearly explain consumption rules

## Risks

The main risk is designing an API that is convenient for one language but poor for the other. Keep the Rust core explicit and let Python wrappers provide cached convenience semantics.

Another risk is accidentally losing backpressure by routing all bodies through internal buffers. Avoid this unless buffering is explicitly requested.

## Handoff notes

After Milestone E, the project has enough core semantics to begin Python bindings safely. The Python sync and async layers should adapt the same body model into blocking iterators and async iterators rather than introducing separate buffering behavior in the engine.
