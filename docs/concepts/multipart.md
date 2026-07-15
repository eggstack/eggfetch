# Multipart and File Uploads

eggfetch supports streaming multipart/form-data request bodies for file uploads and mixed form-plus-file payloads.

## Multipart Model

The multipart subsystem is built on four types:

- **`Multipart`** -- owns a `Boundary` and a list of `Part`s. Provides a builder API.
- **`Part`** -- a single part with name, optional filename, optional content type, optional extra headers, and a body.
- **`PartBody`** -- either `Bytes` (fixed buffer) or `Stream` (async chunked upload).
- **`Boundary`** -- a validated multipart boundary string.

## Building a Multipart Body

```rust
use eggfetch_core::multipart::Multipart;

let body = Multipart::new()
    .text("field", "value")?
    .bytes("file", "report.txt", "text/plain", data)?
    .into_body();
```

### Text Fields

```rust
multipart = multipart.text("description", "sample data")?;
```

### Byte Parts

```rust
multipart = multipart.bytes("file", "data.bin", "application/octet-stream", bytes_data)?;
```

### Streaming Parts

```rust
multipart = multipart.stream("file", "large.bin", "application/octet-stream", stream, Some(size))?;
```

Streaming parts do not buffer file contents. A slow stream naturally backpressures the encoder.

## Boundary Generation

Boundaries are generated randomly by default using a xorshift PRNG seeded from system randomness. Custom boundaries can be provided via `Boundary::try_new()`, which validates that the boundary contains only alphanumeric characters, underscores, and hyphens, and does not exceed 69 characters.

## Streaming Encoder

`MultipartEncoder` implements `Stream<Item = Result<Bytes>>` as a state machine:

1. **PartHeader** -- boundary line, `Content-Disposition`, optional content type, custom headers, blank separator
2. **PartBody** -- polls the part's body stream and forwards chunks
3. **TrailingCrlf** -- `\r\n` after the part body
4. **FinalBoundary** -- `--boundary--\r\n`
5. **Done** -- stream complete

The encoder preserves backpressure: it does not eagerly buffer file contents.

## Known-Length Calculation

`Multipart::content_length()` computes the total body size when all parts have known lengths. Returns `Some(u64)` for known-length bodies (enabling `Content-Length`) or `None` when any part is a stream (falling back to chunked transfer encoding).

## Replayability

A multipart body is replayable only when all parts are `PartBody::Bytes`. If any part is a `PartBody::Stream`, the multipart is non-replayable. Redirect behavior for 307/308 rejects non-replayable multipart bodies.

## Python API

The `files=` kwarg supports several forms:

```python
# Bytes directly
eggfetch.post(url, files={"file": b"data"})

# With filename and content type
eggfetch.post(url, files={"file": ("report.txt", b"contents", "text/plain")})

# Mixed data + files
eggfetch.post(url, data={"description": "sample"}, files={"file": open("data.bin", "rb")})

# Path-backed file via eggfetch.File
eggfetch.post(url, files={"file": eggfetch.File("/path/to/file.pdf")})
```

Supported `files=` forms:

- `bytes` value directly
- `(filename, bytes)` tuple
- `(filename, bytes, content_type)` triple
- `(filename, bytes, content_type, headers)` quad
- `eggfetch.File(path, filename=None, content_type=None)` wrapper

`files=` combined with `data=` merges form fields and file parts into a single multipart body. `files=` combined with `content=` or `json=` raises `TypeError`.

### eggfetch.File

`eggfetch.File` wraps a filesystem path. The file is read synchronously during request construction. For large files, consider streaming the upload from a generator or async iterator instead.

## CLI

```bash
# Upload a file
eggfetch --file report.pdf https://upload.example.com

# Upload with form fields
eggfetch --form "description=quarterly report" --file report.pdf https://upload.example.com
```
