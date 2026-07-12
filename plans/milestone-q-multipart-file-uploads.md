# Milestone Q Plan: Multipart and File Uploads

## Objective

Implement correct, streaming `multipart/form-data` request bodies in `eggfetch-core` and expose requests/httpx-like `files=` semantics through the Python sync and async APIs.

Multipart must preserve the project’s streaming guarantees: large files should not be loaded into memory, cancellation must release resources, known-size bodies should use `Content-Length` when practical, and unknown-size bodies should stream safely.

## Prerequisites

The post-Milestone N validation/polish pass must be complete.

Required stable foundations:

- live request-body streaming
- request-body replayability model
- Python sync/async cancellation behavior
- Content-Length validation
- redirect behavior for non-replayable bodies
- auth/cookie request pipeline

## Scope

Milestone Q includes:

- core multipart model and encoder
- text fields
- byte/file/stream parts
- boundary generation and validation
- per-part headers
- known-length calculation
- streamed encoding
- Python `files=` support
- Python mixed `data=` + `files=` support
- file-like/path/bytes inputs under a documented policy
- multipart tests

Milestone Q does not initially include:

- nested multipart
- MIME tree construction
- resumable uploads
- automatic retries of non-replayable files
- browser form emulation beyond standard multipart/form-data

## Core data model

Suggested types:

```rust
pub struct Multipart {
    boundary: Boundary,
    parts: Vec<Part>,
}

pub enum PartBody {
    Bytes(Bytes),
    Stream {
        stream: BoxBytesStream,
        length: Option<u64>,
    },
}

pub struct Part {
    name: String,
    filename: Option<String>,
    content_type: Option<HeaderValue>,
    headers: Headers,
    body: PartBody,
}
```

Provide builder APIs:

```rust
Multipart::new()
    .text("field", "value")
    .bytes("file", "a.txt", "text/plain", bytes)
    .stream("file", "large.bin", content_type, stream, length)
```

The public API should make replayability and known length inspectable.

## Boundary generation

Generate a sufficiently random boundary without adding an unnecessarily large RNG dependency.

Requirements:

- valid multipart boundary characters
- low collision probability
- no CR/LF
- deterministic injection option for tests

Allow custom boundary only if validated strictly.

## Encoding format

Each part must emit:

```text
--boundary\r\n
Content-Disposition: form-data; name="..."[; filename="..."]\r\n
[Content-Type: ...\r\n]
[additional headers]
\r\n
<body>
\r\n
```

The final terminator is:

```text
--boundary--\r\n
```

Escape or reject invalid names/filenames. Prevent CR/LF header injection.

Use a deliberate filename encoding policy. Initial support may use quoted UTF-8 values with documented limitations; RFC 5987 `filename*` can be added if needed.

## Streaming encoder

Implement multipart as a state-machine stream rather than concatenating all parts.

States should emit:

- boundary/header bytes
- part body chunks
- trailing CRLF
- final boundary

Preserve backpressure and avoid unbounded channels.

Part stream errors should map to a multipart/body error while retaining source information.

## Known-length calculation

Calculate total Content-Length only if every part length is known and all generated header lengths are known.

Use checked arithmetic to prevent overflow.

If any part has unknown length, return unknown total length and permit chunked HTTP/1.1 transfer.

Do not consume streams to determine their size.

## Replayability

Multipart replayability depends on every part:

- text/bytes/path-opened-as-reopenable may be replayable
- arbitrary live streams are non-replayable

For the initial implementation, a multipart body containing any stream should be non-replayable unless a stream factory abstraction is deliberately added.

Redirect behavior:

- 303/body-dropping redirects may proceed
- 307/308 requiring resend reject non-replayable multipart

## File handling in Rust

Provide a file part helper that streams from Tokio file I/O if adding `tokio::fs` is acceptable.

A path-backed part can be replayable if the path is reopened for each send, but file mutation between attempts creates ambiguity. Initial policy may classify it as replayable by reopen, with documentation.

Do not block Tokio threads with synchronous file reads.

## Python API

Target familiar forms:

```python
eggfetch.post(url, files={"file": b"data"})

eggfetch.post(
    url,
    data={"description": "sample"},
    files={"file": ("report.txt", b"contents", "text/plain")},
)
```

Support a documented subset of file specifications:

- bytes-like value
- `(filename, bytes_or_file)`
- `(filename, bytes_or_file, content_type)`
- `(filename, bytes_or_file, content_type, headers)` if practical

Support mappings and sequences of pairs so repeated field names are possible.

## Python file-like objects

This is a key design choice.

Options:

1. Read Python file-like objects into memory before releasing GIL.
2. Call Python `read()` incrementally from the Rust body stream.
3. Support only paths and bytes initially.

Calling Python incrementally from a Tokio/network future complicates GIL, Send, and async behavior. Recommended initial policy:

- support bytes-like values directly
- support filesystem paths through an explicit wrapper or path-like object, streamed by Rust
- optionally support seekable Python file objects by buffering only under a documented size policy

Do not claim constant-memory Python file-object streaming unless it is genuinely implemented safely.

Consider exposing:

```python
eggfetch.File("/path/to/file", filename=None, content_type=None)
```

This maps cleanly to Rust-owned file streaming.

## Body argument conflicts

Define precedence:

- `files=` may be combined with form-style `data=`
- `files=` conflicts with raw `content=` and `json=`
- `data=` plus files becomes multipart fields
- user-supplied Content-Type with files should either be rejected or must include a valid matching boundary

Recommended: reject a manually supplied multipart Content-Type unless a custom Multipart object is supplied; otherwise eggfetch controls the boundary.

## Timeouts and cancellation

Per-chunk write timeout should apply to multipart production/upload.

Cancellation must:

- drop open file handles
- drop active part streams
- release pool permits
- leave the client reusable

## Security and limits

Prevent:

- CR/LF injection in field names, filenames, and headers
- boundary injection/collision through user values
- arithmetic overflow in length calculation
- unbounded buffering of large paths/files

Do not infer MIME types through a large database dependency initially. Default to `application/octet-stream` when unknown.

## Tests

### Core encoder tests

- one text field
- multiple fields
- bytes file
- streamed file
- repeated field names
- custom per-part headers
- final boundary correctness
- known total length exactness
- unknown total length behavior
- checked overflow path
- invalid names/filenames/boundaries rejected

### Streaming tests

- first bytes emitted before full file read
- large path upload remains bounded-memory
- backpressure prevents eager full-file reading
- cancellation closes file/stream
- stream error maps correctly

### Integration tests

Use a local multipart parser/test endpoint to verify:

- fields and files received correctly
- filenames/content types
- repeated names
- Content-Length for known-size multipart
- chunked transfer for unknown-size multipart
- auth/cookies remain applied
- 307/308 non-replayable behavior

### Python tests

- files mapping with bytes
- tuple variants
- data plus files
- repeated files through sequence of pairs
- path-backed streaming wrapper
- content/json conflicts
- sync/async parity
- cancellation during upload
- clear unsupported Python file-object behavior

## Feature gating

Add a `multipart` feature in `eggfetch-core` if dependencies or file I/O warrant it.

Python wheels should enable multipart by default once stable.

## Documentation

Document:

- accepted `files=` forms
- constant-memory guarantees and exceptions
- path versus Python file-object behavior
- redirect replayability
- timeout/cancellation behavior
- multipart Content-Type ownership

## Acceptance criteria

Milestone Q is complete when:

- multipart bodies stream without full buffering
- known total lengths are computed exactly when possible
- unknown-length multipart uploads work
- Python supports common `files=` forms
- data+files behavior matches documented requests/httpx expectations
- cancellation and redirect behavior are safe
- sync/async tests pass

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps

cd crates/eggfetch-python
maturin develop
python -m pytest
```

## Handoff note

Milestone R adds response decompression. Keep multipart request-body encoding independent from response content-coding logic.
