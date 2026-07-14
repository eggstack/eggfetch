# Milestone X Plan: Full CLI

## Objective

Turn `eggfetch-cli` into a practical, scriptable HTTP client that uses only `eggfetch-core`. The CLI should expose common HTTP workflows, preserve streaming, provide stable exit codes, and support both human-readable and machine-readable output.

## Scope

Implement:

- common HTTP methods and arbitrary methods
- headers and query parameters
- raw, JSON, form, and multipart/file bodies
- redirects, cookies, auth, proxies, TLS, retries, and protocol controls
- streamed upload/download
- response headers/body/timing output
- output files and stdout safety
- JSON/NDJSON metadata modes
- deterministic exit-code taxonomy
- shell completion and man-page generation if low cost

The CLI must contain no independent networking semantics.

## Command shape

Recommended baseline:

```text
eggfetch [METHOD] URL [OPTIONS]
```

Examples:

```text
eggfetch GET https://example.com
eggfetch POST URL --json '{"a":1}'
eggfetch POST URL --form a=1 --file upload=@path
eggfetch URL --follow --header 'Authorization: Bearer ...'
```

Use `clap` behind the CLI crate only.

## Input model

Support repeatable:

- `--header/-H NAME:VALUE`
- `--query/-q NAME=VALUE`
- `--form NAME=VALUE`
- `--file NAME=@PATH` with optional filename/content type syntax

Body modes must be mutually exclusive unless form+files intentionally combine.

Support `--body`, `--body-file`, and stdin (`-`). Prevent accidental terminal reads/writes where possible and document binary behavior.

## Streaming and output

Default body output to stdout. Headers/timing should go to stderr unless `--include` or machine mode is requested, preserving pipeline safety.

Support:

- `--output/-o PATH`
- `--download` filename derivation with safe path handling
- `--include` headers
- `--headers-only`
- `--no-body`
- progress display only on TTY and disabled in machine mode
- streaming writes without whole-body buffering

Do not overwrite files unless policy/flag permits.

## Configuration options

Map directly to core:

- timeout fields
- follow/max redirects
- cookies/cookie jar file only if persistence is implemented
- Basic/Bearer auth
- proxy/NO_PROXY
- verify/custom CA/client cert
- retry policy
- decompression and decoded-body limits
- HTTP version selection

Secrets must be redacted from verbose output and process diagnostics where possible. Warn that command-line arguments may be visible to other processes; allow token/password input from environment/file/stdin.

## Output modes

Human mode:

- concise status, headers, timings, body
- optional verbose connection/redirect summary

Machine mode:

```text
--json-output
--ndjson
```

Include stable fields: URL, status, HTTP version, headers as multi-value structure, elapsed timings, redirect history, body metadata, errors. Binary body should be omitted, base64-encoded only by explicit flag, or written separately.

## Exit codes

Define stable categories, for example:

- 0 success including HTTP errors unless `--check-status`
- 2 CLI usage/configuration
- 3 DNS/connect/TLS/proxy transport
- 4 timeout
- 5 protocol/decompression/body limit
- 6 HTTP status failure under `--check-status`
- 7 output/file I/O
- 130 interrupted

Document exact mapping and avoid leaking raw Rust enum values.

## Tests

Required:

- argument parsing snapshots/unit tests
- local-server GET/POST/JSON/form/files
- redirects/cookies/auth
- proxy/TLS options
- streaming large download/upload
- stdout/stderr separation
- binary body handling
- output overwrite/path safety
- machine JSON schema stability
- exit-code mapping
- Ctrl-C cancellation and partial-file policy
- Windows/macOS/Linux smoke tests

Use subprocess integration tests against local fixtures.

## Packaging

Produce standalone binaries in release workflows. Consider Homebrew/cargo-binstall later. Keep the binary name distinct from unrelated `httpx` tools.

## Acceptance criteria

- CLI uses only core APIs
- all common request types work
- large bodies stream
- output and exit behavior are script-safe
- secrets are redacted
- machine schema and exit codes are documented and tested
- binaries build on supported platforms
