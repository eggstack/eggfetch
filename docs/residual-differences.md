# Residual Differences from HTTPX 0.28.1

Known intentional differences where EggFetch cannot match HTTPX/httpcore behavior due to underlying transport constraints.

## `stream_id` — HTTP/2 response stream identifier

**Status**: Residual difference (no implementation path without Hyper stack replacement)

HTTPX/httpcore exposes `Response.extensions["stream_id"]` as an integer for HTTP/2 responses, derived from the h2 stream identifier.

**EggFetch**: `stream_id` is not available. Hyper 1.10.1 consumes `h2::client::ResponseFuture` without extracting the stream ID, and `hyper::body::Incoming` wraps `h2::RecvStream` privately. `hyper_util::client::legacy::ResponseFuture` erases the h2 future entirely. There is no public API path from hyper response to stream ID.

**h2 0.4.15** exposes `StreamId` on `ResponseFuture::stream_id()` and `RecvStream::stream_id()`, but these are not accessible through the hyper abstraction layer.

**Impact**: Python `Response.extensions` will not contain `stream_id` for H2 responses. This is a narrow metadata-only difference; it does not affect request/response semantics.

**Resolution path**: Would require either (a) upstream hyper/hyper-util to expose stream ID via response extensions, or (b) replacing the Hyper client with direct h2 usage. Neither is warranted for a metadata field.
