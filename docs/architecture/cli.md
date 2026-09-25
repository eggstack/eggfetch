# CLI Deep Dive

The CLI crate (`eggfetch-cli`) is a thin binary adapter over `eggfetch-core`. It handles argument parsing, output formatting, and exit codes. No HTTP logic lives here.

See also: [overview.md](overview.md).

## Architecture

```
args.rs → main.rs orchestration → ClientBuilder → RequestBuilder → Response streaming → stdout/file
              ↘ input.rs / output.rs / files.rs / errors.rs
```

The split is private and responsibility-oriented: `args.rs` owns the clap
schema, `input.rs` owns request input parsing, `output.rs` owns human and
machine formatting, `files.rs` owns filename/open policy, and `errors.rs`
owns stable exit-code classification. None of these modules owns HTTP logic.

The CLI creates an `eggfetch_core::Client` via `ClientBuilder`, configures it from command-line flags, constructs requests via `RequestBuilder`, and streams the response body to stdout or a file.

## Argument Model

```
eggfetch URL [OPTIONS]
```

The URL is the only positional argument. The method is set with `-X`/`--method` and defaults to GET (or POST when a body is provided).

### Mapping to Core API

| CLI Flag | Core API |
|----------|----------|
| `-X`/`--method M` | Request method (default GET, POST with body) |
| `-H name:value` | `RequestBuilder::header()` |
| `-q key=value` | `RequestBuilder::query()` |
| `--auth user:pass` | `ClientBuilder::auth()` |
| `--bearer TOKEN` | `ClientBuilder::auth()` |
| `--proxy URL` | `ClientBuilder::proxy()` |
| `--proxy-auth user:pass` | Proxy auth (`EGGFETCH_PROXY_AUTH`) |
| `--no-proxy` | Proxy bypass |
| `--cookie`/`--cookie-jar` | Cookie handling |
| `--no-verify` | `ClientBuilder::tls_config()` (verification off) |
| `--cacert PATH` | `ClientBuilder::tls_config()` |
| `--cert`/`--key` | `ClientBuilder::tls_config()` |
| `--follow`/`--no-follow` | `ClientBuilder::redirect_policy()` |
| `--max-redirects N` | `ClientBuilder::redirect_policy()` |
| `--timeout SECS` | `RequestBuilder::timeout()` (per-request) |
| `--connect-timeout`/`--read-timeout`/`--total-timeout SECS` | Per-phase `RequestBuilder::timeout()` overrides |
| `--retry N` | `ClientBuilder::retry()` |
| `--retry-delay SECS` | Retry backoff delay |
| `--max-body-size N` | `ClientBuilder::max_decoded_body_size` (client-level) |
| `--max-decompression-ratio N` | `ClientBuilder::max_decompression_ratio` (client-level) |
| `--no-body` | Suppress response-body output |
| `--http1`/`--http2`/`--http3` | `ClientBuilder::http_version_policy()` |
| `--no-compress` | `RequestBuilder::decompress(false)` (per-request; no client-level call) |
| `--check-status` | Exit 6 on HTTP error status |
| `--base64` | Include `body_base64` in JSON output |
| `-v`/`--verbose` | Verbose request/response info to stderr |
| `--generate-completion SHELL` | Print shell completions (bash/zsh/fish/powershell/elvish) and exit |

`--auth`/`--bearer` and `--json-output`/`--ndjson` are mutually exclusive
(rejected with exit 2). `--follow`/`--no-follow` is a runtime override pair
(`--no-follow` wins at dispatch), but passing both explicitly is rejected at
parse time by clap (`conflicts_with`, exit 2) — use exactly one flag.
`--http1`/`--http2`/`--http3` are checked
manually (more than one fails with exit 2 via the usage path). mTLS requires
both `--cert` and `--key` together. `--proxy-auth`/`--no-proxy` require
`--proxy`. `--cookie-jar` reads `NAME=VALUE` lines only; the Netscape jar format is not parsed.

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

### Protocol and compression support

`--http2`/`--http3` require a CLI build with the corresponding core feature
compiled in; the default build enables `cookies`, `multipart`, and `proxy`
but **not** `http2` or `http3`. Likewise the default build compiles **no**
compression decoders: the CLI never sends `Accept-Encoding`, so compliant
servers reply identity-encoded. If a server sends an encoded body anyway,
decoding fails with exit 5 unless `--no-compress` passes the bytes through
raw. Requesting an uncompiled capability fails instead of silently
downgrading.

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
  "version": "1.1",
  "headers": [[name, value], ...],
  "elapsed_ms": 123,
  "history": [...],
  "body_length": 456,
  "errors": [...],
  "body_base64": "..."
}
```

`version` uses `version_string()` (`"1.1"`/`"2"`/`"3"`). `errors` is always present; `body_base64` appears only with `--base64`.

## Streaming

The CLI streams the human/file response body via `Response::bytes_stream()` and writes chunks incrementally using `tokio::io::AsyncWriteExt`. No full-body buffering occurs on that path. JSON/NDJSON modes instead buffer via `Response::bytes().await` before emitting.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (HTTP errors included unless `--check-status`) |
| 2 | CLI usage/configuration error (incl. cert/hostname-verification config errors) |
| 3 | DNS/connect/TLS/pool/proxy transport error |
| 4 | Timeout (any phase) |
| 5 | Protocol/decompression/body limit error |
| 6 | HTTP status failure (with `--check-status`) |
| 7 | Hyper/network/file I/O error |
| 130 | Interrupted (Ctrl-C) |

## File Output

- `-o`/`--output PATH`: write body to file, creating or overwriting (JSON/NDJSON modes also honor `--output`).
- `-i`/`--include`: response headers to stderr before body.
- `--output PATH`: write body to file, creating or overwriting.
- `--no-clobber`: prevent overwrite of existing files.
- `--download`: derive filename from `Content-Disposition` header or URL path, with counter-based deduplication.
