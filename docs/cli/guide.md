# eggfetch CLI Guide

eggfetch includes a full-featured command-line HTTP client. It supports all HTTP methods, headers, request bodies, multipart uploads, authentication, proxies, retries, streaming, and machine-readable output formats.

## Installation

```sh
# From source (requires Rust toolchain)
cargo install --path crates/eggfetch-cli

# Or via cargo install from a registry
cargo install eggfetch-cli
```

## Basic Usage

```sh
# GET request (default method)
eggfetch https://example.com

# POST with a body
eggfetch -X POST https://api.example.com/data --json '{"key": "value"}'

# Follow redirects (on by default)
eggfetch https://example.com/redirect --follow
```

## HTTP Methods

Use `-X` or `--method` to specify the HTTP method. If omitted, the method is auto-detected: POST when a body is provided, GET otherwise.

```sh
eggfetch -X GET https://example.com
eggfetch -X POST https://api.example.com
eggfetch -X PUT https://api.example.com/resource
eggfetch -X PATCH https://api.example.com/resource
eggfetch -X DELETE https://api.example.com/resource
eggfetch -X HEAD https://example.com
eggfetch -X OPTIONS https://example.com
```

## Headers

Add headers with `-H`. Can be repeated.

```sh
eggfetch -H "Accept: application/json" -H "X-Custom: value" https://example.com
```

## Query Parameters

Add query parameters with `-q`. Can be repeated.

```sh
eggfetch -q "q=rust" -q "page=1" https://api.example.com/search
```

## Request Body

### Raw body string

```sh
eggfetch -X POST --body "raw payload" https://example.com
```

### Body from file

```sh
eggfetch -X POST --body-file payload.json https://example.com
eggfetch -X POST --body-file - https://example.com  # read from stdin
```

### JSON body

Automatically sets `Content-Type: application/json`.

```sh
eggfetch -X POST --json '{"name": "Alice", "age": 30}' https://api.example.com/users
```

### Form data

Encodes as `application/x-www-form-urlencoded`.

```sh
eggfetch -X POST --form "name=Alice" --form "age=30" https://example.com
```

### Multipart file uploads

Use `--file` for multipart form-data. Format: `NAME=@PATH[:FILENAME]`.

```sh
eggfetch -X POST --file "document=@report.pdf" https://upload.example.com
eggfetch -X POST --file "photo=@image.jpg:avatar.jpg" https://upload.example.com
```

Combine with form fields:

```sh
eggfetch -X POST \
  --form "description=My photo" \
  --file "photo=@photo.jpg" \
  https://upload.example.com
```

## Output Control

### Write to file

```sh
eggfetch -o response.json https://example.com
```

### Download mode

Derives filename from `Content-Disposition` or the URL path.

```sh
eggfetch --download https://example.com/file.zip
```

### Prevent overwriting

```sh
eggfetch --download --no-clobber https://example.com/file.zip
```

### Include response headers

```sh
eggfetch -i https://example.com
```

### Headers only

```sh
eggfetch --headers-only https://example.com
```

### Suppress body

```sh
eggfetch --no-body https://example.com
```

## Output Formats

### JSON output

Structured JSON with status, headers, body, and timing.

```sh
eggfetch --json-output https://example.com
```

### Newline-delimited JSON

One JSON object per line, useful for piping.

```sh
eggfetch --ndjson https://api.example.com/stream
```

### Base64 body encoding

For binary responses.

```sh
eggfetch --json-output --base64 https://example.com/binary-file
```

## Timeouts

### General timeout

Applies to pool, connect, write, and read phases.

```sh
eggfetch --timeout 30 https://example.com
```

### Phase-specific timeouts

```sh
eggfetch --connect-timeout 5 --read-timeout 60 https://slow.example.com
eggfetch --total-timeout 120 https://example.com
```

## Redirects

Follow redirects is on by default (up to 20). Disable or customize:

```sh
eggfetch --follow https://example.com/redirect
eggfetch --no-follow https://example.com/redirect
eggfetch --max-redirects 5 https://example.com/redirect
```

## Authentication

### Basic auth

```sh
eggfetch --auth "user:password" https://api.example.com
```

### Bearer token

```sh
eggfetch --bearer "my-token" https://api.example.com
```

## Cookies

### Send cookies

```sh
eggfetch --cookie "session=abc123" --cookie "lang=en" https://example.com
```

### Cookie jar

Read and write cookies to a file:

```sh
eggfetch --cookie-jar cookies.txt https://example.com/login
eggfetch --cookie-jar cookies.txt https://example.com/dashboard
```

## Proxy

```sh
# HTTP proxy
eggfetch --proxy "http://proxy:8080" https://example.com

# Proxy with authentication
eggfetch --proxy "http://proxy:8080" --proxy-auth "user:pass" https://example.com

# NO_PROXY bypass
eggfetch --proxy "http://proxy:8080" --no-proxy "localhost,.internal.com" https://example.com
```

## TLS/SSL

### Disable certificate verification

```sh
eggfetch --no-verify https://self-signed.example.com
```

### Custom CA bundle

```sh
eggfetch --cacert /path/to/ca-bundle.pem https://example.com
```

### Client certificate (mTLS)

```sh
eggfetch --cert /path/to/cert.pem --key /path/to/key.pem https://example.com
```

## Retry

```sh
eggfetch --retry 3 https://api.example.com
eggfetch --retry 5 --retry-delay 2 https://api.example.com
```

## Protocol Version

```sh
eggfetch --http1 https://example.com
eggfetch --http2 https://example.com
eggfetch --http3 https://example.com
```

`--http2`/`--http3` require a CLI build with the corresponding core feature
compiled in. The default build enables `cookies`, `multipart`, and `proxy`
but **not** `http2` or `http3`; requesting an uncompiled protocol fails
instead of silently downgrading.

## Decompression

```sh
# Disable automatic decompression
eggfetch --no-compress https://example.com

# Limit decoded body size
eggfetch --max-body-size 10485760 https://example.com

# Limit decompression ratio (guards against zip bombs)
eggfetch --max-decompression-ratio 100.0 https://example.com
```

## Status Checking

Exit with code 6 on any non-2xx response:

```sh
eggfetch --check-status https://example.com/404
echo $?  # 6
```

## Debugging

Verbose mode prints request and response details:

```sh
eggfetch -v https://example.com
```

## Shell Completions

Generate shell completions for bash, zsh, fish, PowerShell, or elvish:

```sh
eggfetch --generate-completion bash > /etc/bash_completion.d/eggfetch
eggfetch --generate-completion zsh > ~/.zfunc/_eggfetch
eggfetch --generate-completion fish > ~/.config/fish/completions/eggfetch.fish
```

## Environment Variables

| Variable | Description |
|---|---|
| `EGGFETCH_AUTH` | Basic auth as `USER:PASS` |
| `EGGFETCH_BEARER` | Bearer token |
| `EGGFETCH_PROXY` | Proxy URL |
| `EGGFETCH_PROXY_AUTH` | Proxy auth as `USER:PASS` |

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 2 | Usage error (bad arguments, invalid URL) |
| 3 | Connection error (network, TLS, proxy) |
| 4 | Timeout |
| 5 | Protocol error (HTTP/2, decompression, redirects) |
| 6 | HTTP status error (with `--check-status`, on any non-2xx) |
| 7 | I/O error |
| 130 | Interrupted (Ctrl+C) |

## Practical Examples

### API calls

```sh
# Fetch JSON from an API
eggfetch -H "Accept: application/json" https://api.example.com/users

# Create a resource
eggfetch -X POST \
  -H "Content-Type: application/json" \
  --json '{"name": "Alice", "email": "alice@example.com"}' \
  https://api.example.com/users

# Authenticate
eggfetch --bearer "$TOKEN" https://api.example.com/me
```

### File downloads

```sh
# Download a file
eggfetch --download https://example.com/archive.tar.gz

# Download with progress (verbose)
eggfetch -v --download https://example.com/large-file.zip

# Pipe to another command
eggfetch https://api.example.com/data | jq '.results[]'
```

### JSON scripting

```sh
# Extract a field from JSON response
eggfetch --json-output https://api.example.com/status | jq '.status'

# Post and check response
eggfetch --json-output -X POST --json '{"query": "test"}' https://api.example.com/search | jq '.count'
```

### Full example with auth, proxy, and retries

```sh
eggfetch \
  -X GET \
  -H "Accept: application/json" \
  --bearer "$API_TOKEN" \
  --proxy "http://corporate-proxy:8080" \
  --retry 3 \
  --timeout 30 \
  --json-output \
  https://api.example.com/data
```
