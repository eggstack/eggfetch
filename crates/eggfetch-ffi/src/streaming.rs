//! Streaming response FFI functions.
//!
//! Provides streaming body access via a chunk-at-a-time interface.
//! The response headers are available immediately; body chunks arrive
//! as the network delivers them.

use std::ptr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};

use crate::handle::{ClientHandle, ErrorHandle, RequestHandle};
use crate::runtime::blocking_send;

/// State shared between a streaming handle and any in-flight `next` call.
///
/// `Arc`-shared so [`eggfetch_response_stream_free`] only releases the
/// caller's reference: a concurrently parked `next` keeps its own clone,
/// so freeing the handle can never tear down state that call still uses.
struct StreamState {
    /// Body chunk source, behind a mutex so `next` and `cancel` never form
    /// aliasing `&mut` borrows when called concurrently from different
    /// threads. `next` temporarily takes the receiver out of the slot so
    /// a concurrent `next` finds it empty and returns null instead of
    /// interleaving chunk reads.
    rx: Mutex<Option<mpsc::Receiver<Result<Bytes, eggfetch_core::Error>>>>,
    /// Description of the error that ended the stream early, if any.
    /// Set when the producer observes a mid-stream failure; query via
    /// [`eggfetch_response_stream_error`]. Behind a mutex for the same
    /// aliasing reason as `rx`.
    last_error: Mutex<Option<String>>,
    /// Cancellation flag. Writers set it via
    /// [`eggfetch_response_stream_cancel`]; the parked reader and the
    /// producer task observe it through the paired watch receivers.
    cancel_tx: watch::Sender<bool>,
}

impl StreamState {
    fn new(rx: mpsc::Receiver<Result<Bytes, eggfetch_core::Error>>) -> Self {
        let (cancel_tx, _) = watch::channel(false);
        Self {
            rx: Mutex::new(Some(rx)),
            last_error: Mutex::new(None),
            cancel_tx,
        }
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.cancel_tx.subscribe()
    }
}

/// Opaque handle to a streaming response.
///
/// Headers and status are available immediately. Use
/// [`eggfetch_response_stream_next`] to read body chunks.
/// Free with [`eggfetch_response_stream_free`].
pub struct StreamingResponseHandle {
    status: u16,
    url: String,
    headers: Vec<(String, String)>,
    state: Arc<StreamState>,
}

/// A single chunk from a streaming response.
///
/// `data` points to `len` bytes of valid memory.
/// Free with [`eggfetch_stream_chunk_free`].
pub struct StreamChunk {
    /// Pointer to the chunk data. null if the chunk is empty.
    pub data: *mut u8,
    /// Length of the chunk data in bytes.
    pub len: usize,
}

/// Send a request and return a streaming response handle.
///
/// The response headers are available immediately. Body chunks are read
/// via [`eggfetch_response_stream_next`].
///
/// On success, returns a streaming response handle.
/// On failure, sets `*err_out` to a new error handle.
///
/// # Safety
///
/// - `client` must be a valid, non-freed handle.
/// - `request` must be a valid, non-freed handle (consumed by this call).
/// - `err_out` must be null or point to a `*mut ErrorHandle` slot.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_send_streaming(
    client: *const ClientHandle,
    request: *mut RequestHandle,
    err_out: *mut *mut ErrorHandle,
) -> *mut StreamingResponseHandle {
    crate::ffi_guard!(
        {
            if !err_out.is_null() {
                let err = eggfetch_core::Error::Connect("panic at FFI boundary".into());
                *err_out = Box::into_raw(crate::handle::error_to_handle(&err));
            }
            ptr::null_mut()
        },
        {
            if client.is_null() {
                if !request.is_null() {
                    drop(Box::from_raw(request));
                }
                if !err_out.is_null() {
                    let err = eggfetch_core::Error::Connect("null client handle".into());
                    *err_out = Box::into_raw(crate::handle::error_to_handle(&err));
                }
                return ptr::null_mut();
            }
            let Some(request) = request.as_mut() else {
                if !err_out.is_null() {
                    let err = eggfetch_core::Error::Connect("null request handle".into());
                    *err_out = Box::into_raw(crate::handle::error_to_handle(&err));
                }
                return ptr::null_mut();
            };

            let client_clone = (*client).0.clone();
            let request_box = Box::from_raw(request);
            let Some(rb) = request_box.0 else {
                if !err_out.is_null() {
                    let err = eggfetch_core::Error::RequestBuild(
                        "request handle already consumed".into(),
                    );
                    *err_out = Box::into_raw(crate::handle::error_to_handle(&err));
                }
                return ptr::null_mut();
            };

            let (tx, rx) = mpsc::channel(16);
            let state = Arc::new(StreamState::new(rx));
            let mut cancel_watch = state.subscribe();

            let result = blocking_send(async move {
                let mut resp = Box::pin(rb.send()).await?;
                let status = resp.status().as_u16();
                let url = resp.url().to_string();
                let headers: Vec<(String, String)> = resp
                    .headers()
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.as_str().to_owned(),
                            String::from_utf8_lossy(v.as_bytes()).into_owned(),
                        )
                    })
                    .collect();

                let stream = resp.bytes_stream()?;

                tokio::spawn(async move {
                    let mut stream = stream;
                    while let Some(chunk) = stream.next().await {
                        if *cancel_watch.borrow_and_update() {
                            break;
                        }
                        match chunk {
                            Ok(data) => {
                                if tx.send(Ok(data)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e)).await;
                                break;
                            }
                        }
                    }
                    drop(client_clone);
                });

                Ok::<_, eggfetch_core::Error>(StreamingResponseHandle {
                    status,
                    url,
                    headers,
                    state,
                })
            })
            .and_then(std::convert::identity);

            match result {
                Ok(handle) => Box::into_raw(Box::new(handle)),
                Err(ref e) => {
                    if !err_out.is_null() {
                        *err_out = Box::into_raw(crate::handle::error_to_handle(e));
                    }
                    ptr::null_mut()
                }
            }
        }
    )
}

/// Free a streaming response handle.
///
/// Cancels the stream and releases the caller's reference to the shared
/// state. Because that state is `Arc`-shared with any in-flight `next`
/// call, freeing the handle while another thread is parked in
/// [`eggfetch_response_stream_next`] is safe: the parked call finishes
/// against the surviving clone (observing the cancellation) and the
/// state is released when it returns. Using the handle pointer itself
/// after `free` remains forbidden, as with every eggfetch handle.
///
/// # Safety
///
/// `handle` must have been returned by [`eggfetch_client_send_streaming`] and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_free(handle: *mut StreamingResponseHandle) {
    crate::ffi_guard!((), {
        if !handle.is_null() {
            let h = Box::from_raw(handle);
            // See `cancel`: record the flag even with no live receivers.
            h.state.cancel_tx.send_replace(true);
            // Dropping `h` releases only the caller's Arc reference; a
            // parked `next` holds its own clone of `h.state`.
        }
    });
}

/// Get the HTTP status code of a streaming response.
///
/// Returns 0 if handle is null.
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_status(
    handle: *const StreamingResponseHandle,
) -> u16 {
    crate::ffi_guard!(0, { handle.as_ref().map_or(0, |h| h.status) })
}

/// Get the response URL as a newly allocated C string.
///
/// Returns null if handle is null or the URL contains an interior null
/// byte. Caller must free with [`crate::handle::eggfetch_string_free`].
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_url(
    handle: *const StreamingResponseHandle,
) -> *mut std::os::raw::c_char {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(handle) = handle.as_ref() else {
            return ptr::null_mut();
        };
        crate::handle::FfiString::from_string(handle.url.clone())
            .map_or_else(ptr::null_mut, crate::handle::FfiString::into_raw)
    })
}

/// Get the number of response headers.
///
/// Returns 0 if handle is null.
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_header_count(
    handle: *const StreamingResponseHandle,
) -> usize {
    crate::ffi_guard!(0, { handle.as_ref().map_or(0, |h| h.headers.len()) })
}

/// Get a response header by index.
///
/// On success, sets `*name_out` and `*value_out` to newly allocated C strings.
/// Caller must free both with [`crate::handle::eggfetch_string_free`].
///
/// Returns 0 on success, -1 on error (invalid index, null handle, or a
/// name/value containing an interior null byte).
///
/// # Safety
///
/// - `handle` must be a valid, non-freed streaming response handle.
/// - `name_out` and `value_out` must be valid pointers to `*mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_header(
    handle: *const StreamingResponseHandle,
    index: usize,
    name_out: *mut *mut std::os::raw::c_char,
    value_out: *mut *mut std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_ref() else {
            return -1;
        };
        if name_out.is_null() || value_out.is_null() {
            return -1;
        }
        let Some((name, value)) = handle.headers.get(index) else {
            return -1;
        };
        let Some(name_str) = crate::handle::FfiString::from_string(name.clone()) else {
            return -1;
        };
        let Some(value_str) = crate::handle::FfiString::from_string(value.clone()) else {
            return -1;
        };
        *name_out = name_str.into_raw();
        *value_out = value_str.into_raw();
        0
    })
}

/// Read the next chunk from the streaming response.
///
/// Blocks until a chunk is available. Returns null when the stream is
/// exhausted or has been cancelled; query
/// [`eggfetch_response_stream_error`] to distinguish cancellation,
/// mid-stream failures, and allocation failures from a clean end-of-body.
///
/// While this call is parked it waits on both the chunk channel and the
/// cancellation watch, so [`eggfetch_response_stream_cancel`] (or
/// [`eggfetch_response_stream_free`]) wakes it immediately instead of
/// waiting for the producer to deliver another chunk. A concurrent `next`
/// call from another thread finds the empty receiver slot and returns
/// null rather than interleaving with the parked reader.
///
/// A `blocking_send` failure (runtime creation failure, task panic/drop)
/// is terminal: the receiver was moved into the parked future and cannot
/// be restored, and the producer's sender is disconnected once the
/// receiver is dropped. Later `next` calls return null with the recorded
/// error available via [`eggfetch_response_stream_error`].
///
/// Caller must free the returned chunk with [`eggfetch_stream_chunk_free`].
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
#[allow(
    clippy::too_many_lines,
    reason = "FFI stream cancellation, ownership, and error state must stay in one transition"
)]
pub unsafe extern "C" fn eggfetch_response_stream_next(
    handle: *mut StreamingResponseHandle,
) -> *mut StreamChunk {
    crate::ffi_guard!(ptr::null_mut(), {
        // What a parked receive resolved to. The receiver comes back
        // only when the stream should continue after this chunk.
        struct RecvOutcome {
            chunk: Option<Result<Bytes, eggfetch_core::Error>>,
            rx: Option<mpsc::Receiver<Result<Bytes, eggfetch_core::Error>>>,
        }
        let Some(handle) = handle.as_ref() else {
            return ptr::null_mut();
        };
        // Clone the shared state up-front: everything below works only
        // with this Arc, so a concurrent `free` releasing the caller's
        // reference cannot invalidate memory this call still touches.
        let state = Arc::clone(&handle.state);

        // Take the receiver out of the slot so a concurrent `next` sees
        // an empty slot and returns null instead of interleaving reads.
        let mut rx = {
            let Ok(mut guard) = state.rx.lock() else {
                if let Ok(mut last) = state.last_error.lock() {
                    *last = Some("stream receiver mutex poisoned".to_owned());
                }
                return ptr::null_mut();
            };
            match guard.take() {
                Some(rx) => rx,
                None => return ptr::null_mut(),
            }
        };

        // Observe any cancellation that already happened before we start
        // waiting, then select on the watch so a later cancel cannot be
        // lost between the check and the park.
        let mut cancelled = state.subscribe();
        let outcome: Option<RecvOutcome> = if *cancelled.borrow_and_update() {
            None
        } else {
            Some(
                match blocking_send(async move {
                    tokio::select! {
                        chunk = rx.recv() => RecvOutcome { chunk, rx: Some(rx) },
                        () = async { let _ = cancelled.changed().await; } => {
                            RecvOutcome { chunk: None, rx: None }
                        }
                    }
                }) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        // Terminal: the receiver was moved into the future and
                        // is dropped with it; the producer sender disconnects.
                        // Record the cause so the host can distinguish this
                        // from clean EOF via `eggfetch_response_stream_error`.
                        if let Ok(mut last) = state.last_error.lock() {
                            *last = Some(error.to_string());
                        }
                        return ptr::null_mut();
                    }
                },
            )
        };

        let (chunk, rx) = match outcome {
            // Pre-existing or in-flight cancellation: the receiver was
            // dropped inside the future (or never moved), ending it.
            None
            | Some(RecvOutcome {
                chunk: None,
                rx: None,
            }) => {
                if let Ok(mut last) = state.last_error.lock() {
                    *last = Some("stream cancelled".to_owned());
                }
                return ptr::null_mut();
            }
            Some(RecvOutcome { chunk, rx }) => (chunk, rx),
        };

        match chunk {
            Some(Ok(data)) => {
                if *state.subscribe().borrow_and_update() {
                    // A chunk raced with cancellation: end the stream
                    // here and surface why via `last_error` so hosts
                    // can distinguish this from a natural end-of-body.
                    if let Ok(mut last) = state.last_error.lock() {
                        *last = Some("stream cancelled".to_owned());
                    }
                    return ptr::null_mut();
                }
                let len = data.len();
                let buf = if len > 0 {
                    let Ok(layout) = std::alloc::Layout::array::<u8>(len) else {
                        if let Ok(mut last) = state.last_error.lock() {
                            *last = Some("out of memory: chunk layout unusable".to_owned());
                        }
                        return ptr::null_mut();
                    };
                    let buf = std::alloc::alloc(layout);
                    if buf.is_null() {
                        if let Ok(mut last) = state.last_error.lock() {
                            *last = Some("out of memory allocating chunk buffer".to_owned());
                        }
                        return ptr::null_mut();
                    }
                    std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len);
                    buf
                } else {
                    ptr::null_mut()
                };
                // The stream continues: hand the receiver back for the
                // next call.
                if let Some(rx) = rx {
                    match state.rx.lock() {
                        Ok(mut guard) => *guard = Some(rx),
                        Err(_) => {
                            // Poisoned lock: the stream cannot continue
                            // after this chunk; record why so the host's
                            // final null return is distinguishable from
                            // clean EOF.
                            if let Ok(mut last) = state.last_error.lock() {
                                *last = Some("stream receiver mutex poisoned".to_owned());
                            }
                        }
                    }
                }
                Box::into_raw(Box::new(StreamChunk { data: buf, len }))
            }
            Some(Err(e)) => {
                // Record the failure so hosts can distinguish a truncated
                // stream from a clean end-of-body via
                // `eggfetch_response_stream_error`.
                if let Ok(mut last) = state.last_error.lock() {
                    *last = Some(e.to_string());
                }
                ptr::null_mut()
            }
            None => ptr::null_mut(),
        }
    })
}

/// Get the error that ended the stream early, if any.
///
/// Returns null when the stream ended cleanly, has not been consumed to
/// completion, or the handle is invalid. When a mid-stream failure
/// occurred (network reset, decompression error, ...) or the stream was
/// cancelled via [`eggfetch_response_stream_cancel`], returns a newly
/// allocated C string describing it; caller must free with
/// [`crate::handle::eggfetch_string_free`].
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_error(
    handle: *const StreamingResponseHandle,
) -> *mut std::os::raw::c_char {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(handle) = handle.as_ref() else {
            return ptr::null_mut();
        };
        let Ok(last) = handle.state.last_error.lock() else {
            return ptr::null_mut();
        };
        match last.as_ref() {
            Some(message) => crate::handle::FfiString::from_string(message.clone())
                .map_or_else(ptr::null_mut, crate::handle::FfiString::into_raw),
            None => ptr::null_mut(),
        }
    })
}

/// Free a stream chunk.
///
/// # Safety
///
/// - `chunk` must have been returned by [`eggfetch_response_stream_next`].
/// - Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_stream_chunk_free(chunk: *mut StreamChunk) {
    crate::ffi_guard!((), {
        if !chunk.is_null() {
            let c = Box::from_raw(chunk);
            if !c.data.is_null() && c.len > 0 {
                // A chunk this API allocated always had a computable
                // layout, so failure here implies caller corruption;
                // leak rather than abort the host process.
                if let Ok(layout) = std::alloc::Layout::array::<u8>(c.len) {
                    std::alloc::dealloc(c.data, layout);
                }
            }
        }
    });
}

/// Cancel an in-progress streaming response.
///
/// Subsequent calls to [`eggfetch_response_stream_next`] will return null.
/// The cancellation is published on a watch channel that a parked `next`
/// selects on, so a blocked reader wakes immediately — even when the
/// producer is itself parked waiting for an idle server to send more
/// data. This call never aliases the handle mutably and may be called
/// from any thread while `next` is blocked.
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_cancel(handle: *mut StreamingResponseHandle) {
    crate::ffi_guard!((), {
        let Some(handle) = handle.as_ref() else {
            return;
        };
        // Publishing the flag wakes a parked `next` through its watch
        // receiver and makes the producer task stop forwarding chunks.
        // `send_replace` (not `send`) so the flag is recorded even when
        // every current receiver is gone — a later `next` must still
        // observe the cancellation.
        handle.state.cancel_tx.send_replace(true);
    });
}
