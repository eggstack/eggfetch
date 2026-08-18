# HTTPX/httpcore 1.0.9 Trace Vocabulary

Pinned reference for the `trace` extension callback event names.

## Event model

httpcore uses a `Trace` context manager that emits three phases for each event:

- `{event_name}.started` — operation begun
- `{event_name}.complete` — operation succeeded; info dict contains `return_value`
- `{event_name}.failed` — operation failed; info dict contains `exception`

The callback signature is `trace(event_name: str, info: dict) -> None` (sync) or
`async def trace(event_name: str, info: dict) -> None` (async). The async form
is used by `AsyncClient`; the sync form by `Client`.

## Event names by subsystem

### Connection (`httpcore.connection`)

| Event | Info keys (started) | Info keys (complete) |
|-------|--------------------|--------------------|
| `connect_tcp` | `host`, `port`, `local_address`, `timeout`, `socket_options` | `return_value` (NetworkStream) |
| `connect_unix_socket` | `path`, `timeout`, `socket_options` | `return_value` (NetworkStream) |
| `start_tls` | `ssl_context`, `server_hostname`, `timeout` | `return_value` (NetworkStream) |
| `retry` | (same as last attempt kwargs) | — |
| `close` | — | — |

### HTTP/1.1 (`httpcore.http11`)

| Event | Info keys (started) | Info keys (complete) |
|-------|--------------------|--------------------|
| `send_request_headers` | `request` (Request) | — |
| `send_request_body` | `request` (Request) | — |
| `receive_response_headers` | `request` (Request) | `return_value` = `(http_version, status, reason_phrase, headers)` |
| `receive_response_body` | `request` (Request) | — |
| `response_closed` | — | — |

### HTTP/2 (`httpcore.http2`)

| Event | Notes |
|-------|-------|
| `connect_tcp` | Same as connection layer |
| `start_tls` | Same as connection layer |
| `send_request_headers` | H2 HEADERS frame |
| `send_request_body` | H2 DATA frames |
| `receive_response_headers` | H2 HEADERS frame |
| `receive_response_body` | H2 DATA frames |
| `response_closed` | Stream closed |

### HTTP proxy (`httpcore.http_proxy`)

| Event | Notes |
|-------|-------|
| `connect_tcp` | To proxy host |
| `start_tls` | TLS to proxy (for HTTPS proxy) |
| `send_request_headers` | CONNECT or forwarded request |
| `receive_response_headers` | CONNECT 200 or forwarded response |

### SOCKS5 proxy (`httpcore.socks_proxy`)

| Event | Notes |
|-------|-------|
| `connect_tcp` | To SOCKS proxy |
| `start_tls` | TLS after SOCKS handshake |
| `send_request_headers` | Through SOCKS tunnel |
| `receive_response_headers` | Through SOCKS tunnel |

## EggFetch mapping

EggFetch's `TraceEvent` enum maps to these names via `event_to_httpcore_name()`:

| `TraceEvent` variant | httpcore event name |
|---------------------|-------------------|
| `ConnectTcp` | `connect_tcp.{phase}` |
| `ConnectUnixSocket` | `connect_unix_socket.{phase}` |
| `StartTls` | `start_tls.{phase}` |
| `Retry` | `retry.{phase}` |
| `Close` | `close.{phase}` |
| `SendRequestHeaders` | `send_request_headers.{phase}` |
| `SendRequestBody` | `send_request_body.{phase}` |
| `ReceiveResponseHeaders` | `receive_response_headers.{phase}` |
| `ReceiveResponseBody` | `receive_response_body.{phase}` |
| `ResponseClosed` | `response_closed.{phase}` |

## Implementation notes

- Connector-level events (`connect_tcp`, `start_tls`) happen inside hyper's
  connector layer and are not directly hookable. EggFetch emits HTTP-level
  events (`send_request_headers`, `receive_response_headers`) from the
  transport send functions.
- The `request` info key in httpcore events contains the full `Request` object.
  EggFetch's typed events carry only scalar metadata (method, target, status)
  to avoid exposing internal types.
- httpcore's `Trace` context manager acquires the GIL only at callback delivery
  points. EggFetch's `TraceObserver` trait is `Send + Sync` and invoked
  synchronously within the async context.
