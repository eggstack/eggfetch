# CLI Deep Dive

The CLI crate (`eggfetch-cli`) is a thin binary adapter over `eggfetch-core`. It handles argument parsing, output formatting, and exit codes. No HTTP logic lives here.

See also: [overview.md](overview.md).

## Architecture

```
main.rs → clap arg parsing → ClientBuilder configuration → RequestBuilder → Response streaming → stdout/file
```

The CLI creates an `eggfetch_core::Client` via `ClientBuilder`, configures it from command-line flags, constructs requests via `RequestBuilder`, and streams the response body to stdout or a file.

## Argument Model

```
eggfetch [METHOD] URL [OPTIONS]
```

METHOD defaults to GET (or POST when a body is provided).

### Mapping to Core API

| CLI Flag | Core API |
|----------|----------|
| `-H name:value` | `RequestBuilder::header()` |
| `-q key=value` | `RequestBuilder::query()` |
| `--auth user:pass` | `ClientBuilder::auth()` |
| `--bearer TOKEN` | `ClientBuilder::auth()` |
| `--proxy URL` | `ClientBuilder::proxy()` |
| `--verify`/`--no-verify` | `ClientBuilder::tls_config()` |
| `--cacert PATH` | `ClientBuilder::tls_config()` |
| `--cert`/`--key` | `ClientBuilder::tls_config()` |
| `--follow`/`--no-follow` | `ClientBuilder::redirect_policy()` |
| `--max-redirects N` | `ClientBuilder::redirect_policy()` |
| `--timeout SECS` | `ClientBuilder::timeout()` |
| `--retry N` | `ClientBuilder::retry()` |
| `--http1`/`--http2`/`--http3` | `ClientBuilder::http_version_policy()` |
| `--no-compress` | `ClientBuilder::automatic_decompression(false)` |

### Environment Variables

| Variable | Maps To |
|----------|---------|
| `EGGFETCH_AUTH` | `--auth` |
| `EGGFETCH_BEARER` | `--bearer` |
| `EGGFETCH_PROXY` | `--proxy` |
| `EGGFETCH_PROXY_AUTH` | Proxy auth |

## Body Modes

Body sources are mutually exclusive (except `--form` + `--file`):

| Flag | Body Type |
|------|-----------|
| `--json` | JSON with auto `Content-Type: application/json` |
| `--body` | Raw body string |
| `--body-file` | Read from file (or `-` for stdin) |
| `--form` | `application/x-www-form-urlencoded` |
| `--file` | Multipart file parts |

`--form` + `--file` combines text fields and files into a multipart body.

## Output Modes

| Mode | Flag | Behavior |
|------|------|----------|
| Human | (default) | Body to stdout, verbose to stderr with `-v` |
| Headers | `--include` | Response headers to stderr before body |
| Headers-only | `--headers-only` | Status line + headers, no body |
| JSON | `--json-output` | Structured JSON with status, headers, elapsed, history |
| NDJSON | `--ndjson` | Newline-delimited JSON with redirect hops |

### JSON Output Structure

```json
{
  "url": "...",
  "status": 200,
  "version": "HTTP/1.1",
  "headers": {...},
  "elapsed_ms": 123,
  "history": [...],
  "body_length": 456,
  "body_base64": "..."
}
```

## Streaming

The CLI streams the response body via `Response::bytes_stream()` and writes chunks incrementally using `tokio::io::AsyncWriteExt`. No full-body buffering occurs.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (HTTP errors included unless `--check-status`) |
| 2 | CLI usage/configuration error |
| 3 | DNS/connect/TLS/proxy transport error |
| 4 | Timeout (any phase) |
| 5 | Protocol/decompression/body limit error |
| 6 | HTTP status failure (with `--check-status`) |
| 7 | Output/file I/O error |
| 130 | Interrupted (Ctrl-C) |

## File Output

- `--output PATH`: write body to file, creating or overwriting.
- `--no-clobber`: prevent overwrite of existing files.
- `--download`: derive filename from `Content-Disposition` header or URL path, with counter-based deduplication.
