# Quick start

eggfetch is a Rust-native HTTP client with Python bindings and a CLI
tool. The core engine is async-first, built on tokio and hyper. The
Python API exposes both sync and async interfaces; the sync API blocks
on the async engine while releasing the GIL.

## Install

```bash
pip install eggfetch
```

## First request

```python
import eggfetch

r = eggfetch.get("https://httpbin.org/get")
print(r.status_code)
print(r.text)
```

## Using a client

```python
import eggfetch

with eggfetch.Client(headers={"User-Agent": "my-app/1.0"}) as client:
    # Buffered response
    r = client.get("https://httpbin.org/get")
    print(r.json())

    # Streaming response
    with client.stream("GET", "https://httpbin.org/stream-bytes/10000") as r:
        for chunk in r.iter_bytes():
            print(f"chunk: {len(chunk)} bytes")
```

## Async client

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient() as client:
        r = await client.get("https://httpbin.org/get")
        print(r.status_code)
        data = await client.get("https://httpbin.org/ip")
        print(data.json())

asyncio.run(main())
```

## CLI

```bash
# GET request
eggfetch https://httpbin.org/get

# POST JSON
eggfetch -X POST https://httpbin.org/post --json '{"key": "value"}'

# With auth
eggfetch --auth user:pass https://httpbin.org/basic-auth/user/pass
```

## Key concepts

**Client lifecycle.** Create a `Client` (or `AsyncClient`) to reuse
connections, set default headers, and share configuration. Always close
the client when done, preferably via a `with` statement.

**Async engine.** All networking runs through a single Rust async engine.
The Python sync API blocks on this engine and releases the GIL, so
concurrent threads are not blocked.

**Streaming.** Use `client.stream()` for true network streaming that
reads chunks as they arrive without buffering the full response in
memory.

## Next steps

- [Installation](installation.md) -- platform support and feature flags
- [Migration from requests](../migration/from-requests.md) -- side-by-side comparison
- [Migration from HTTPX](../migration/from-httpx.md) -- side-by-side comparison
- [Cookbook](../cookbook/examples.md) -- practical examples
- [Feature matrix](../reference/feature-matrix.md) -- what is available where
