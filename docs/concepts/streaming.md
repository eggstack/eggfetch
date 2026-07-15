# Streaming

eggfetch provides both buffered and streaming APIs for request and response bodies. The choice affects memory usage, backpressure, and pool permit lifecycle.

## Request Bodies

Request bodies come in three variants:

- **Empty** -- no body sent
- **Bytes** -- fixed-size buffer with known length, sent with `Content-Length`
- **Stream** -- async stream of chunks, sent incrementally with no eager buffering

Stream bodies are piped through to the transport: each chunk is sent as soon as it is produced. A slow producer naturally backpressures the transport. When the stream has a known length, `Content-Length` is set; otherwise, the transport selects a safe transfer mode (chunked for HTTP/1.1).

Only `Empty` and `Bytes` bodies are replayable. Stream bodies are consumed on use and cannot be resent for redirects or retries.

## Response Bodies

All responses from the client start as `ResponseBody::Streaming`. Callers choose their consumption model:

- **Buffered**: `response.bytes()` or `response.text()` collects the full body into memory
- **Streaming**: `response.bytes_stream()` returns a stream for chunk-by-chunk processing
- **Line streaming**: `response.text_lines()` splits the stream into text lines

### Single-Consumption Semantics

Body types are single-consume. Calling `bytes_stream()` on a streaming body replaces it with `Consumed`; a second call returns an error. Calling `bytes()` on a consumed body also returns an error. This prevents accidental double-reads and enforces ownership transfer.

### Pool Permit Lifecycle

Streaming response bodies carry an internal pool permit. The permit is released when the body is fully consumed, explicitly closed, or dropped. A streaming response that is held but not consumed continues to occupy its pool slot.

Buffered and already-consumed responses do not carry a lease.

## Rust API

```rust
// Buffered
let bytes = response.bytes().await?;
let text = response.text().await?;

// Streaming
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    // process chunk
}

// Line streaming
let mut lines = response.text_lines();
while let Some(line) = lines.next().await {
    let line = line?;
    // process line
}
```

## Python API

### Buffered Response

`response` returned by `client.get(...)` is fully buffered:

```python
response = client.get("https://example.com")

# Access buffered data
print(response.text)
print(response.content)

# Iterate over buffered chunks
for chunk in response.iter_bytes():
    process(chunk)

for line in response.iter_lines():
    process(line)
```

### Streaming Response

`client.stream(...)` returns a `StreamingResponse` context manager with true network streaming:

```python
with client.stream("GET", url) as resp:
    for chunk in resp.iter_bytes():
        process(chunk)  # each chunk arrives from the network
    # or: resp.iter_text(), resp.iter_lines(), resp.read(), resp.text
```

Async variant:

```python
async with client.stream("GET", url) as resp:
    async for chunk in resp.aiter_bytes():
        process(chunk)
    # or: resp.aiter_text(), resp.aiter_lines(), resp.aread(), resp.text
```

### StreamingResponse Lifecycle

- Starts in `streaming` state
- `read()` / `aread()` transitions to `buffered`
- Iterators transfer ownership to `consumed`
- Explicit or context-manager close transitions to `closed`
- These are terminal ownership transitions

GIL is released during sync iteration and blocking body reads. Pool permits are released when the body is fully consumed, explicitly closed, or dropped.

## Cancellation Safety

Cancelling an in-flight request drops the Rust future cleanly. Pool permits are released via RAII `PoolGuard`. Dropping an async iterator aborts its producer task; dropping a sync iterator closes its worker channel.

## Buffered vs Streaming Iteration

| API | Behavior | Use case |
|-----|----------|----------|
| `response.iter_bytes()` | Iterates over pre-buffered chunks | Small responses, simple iteration |
| `response.text` | Returns fully buffered text | Small responses, read-once access |
| `client.stream().iter_bytes()` | Streams chunks from network | Large responses, backpressure control |
| `client.stream().read()` | Reads remaining stream into memory | When you need the full body but started streaming |
