//! Streaming response FFI functions.
//!
//! Provides streaming body access via a chunk-at-a-time interface.
//! The response headers are available immediately; body chunks arrive
//! as the network delivers them.

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::handle::{ClientHandle, ErrorHandle, RequestHandle};
use crate::runtime::blocking_send;

/// Opaque handle to a streaming response.
///
/// Headers and status are available immediately. Use
/// [`eggfetch_response_stream_next`] to read body chunks.
/// Free with [`eggfetch_response_stream_free`].
pub struct StreamingResponseHandle {
    status: u16,
    url: String,
    headers: Vec<(String, String)>,
    rx: mpsc::Receiver<Result<Bytes, eggfetch_core::Error>>,
    cancel: Arc<AtomicBool>,
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
    if client.is_null() {
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
    let rb = std::ptr::read(&request_box.0);
    std::mem::forget(request_box);

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();

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

        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            let mut stream = stream;
            while let Some(chunk) = stream.next().await {
                if cancel_clone.load(Ordering::Relaxed) {
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
            rx,
            cancel,
        })
    });

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

/// Free a streaming response handle.
///
/// # Safety
///
/// `handle` must have been returned by [`eggfetch_client_send_streaming`] and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_free(handle: *mut StreamingResponseHandle) {
    if !handle.is_null() {
        let h = Box::from_raw(handle);
        h.cancel.store(true, Ordering::Relaxed);
        drop(h.rx);
    }
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
    handle.as_ref().map_or(0, |h| h.status)
}

/// Get the response URL as a newly allocated C string.
///
/// Returns null if handle is null. Caller must free with [`crate::handle::eggfetch_string_free`].
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_url(
    handle: *const StreamingResponseHandle,
) -> *mut std::os::raw::c_char {
    let Some(handle) = handle.as_ref() else {
        return ptr::null_mut();
    };
    crate::handle::FfiString::from_string(handle.url.clone()).into_raw()
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
    handle.as_ref().map_or(0, |h| h.headers.len())
}

/// Get a response header by index.
///
/// On success, sets `*name_out` and `*value_out` to newly allocated C strings.
/// Caller must free both with [`crate::handle::eggfetch_string_free`].
///
/// Returns 0 on success, -1 on error.
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
    let Some(handle) = handle.as_ref() else {
        return -1;
    };
    if name_out.is_null() || value_out.is_null() {
        return -1;
    }
    let Some((name, value)) = handle.headers.get(index) else {
        return -1;
    };
    *name_out = crate::handle::FfiString::from_string(name.clone()).into_raw();
    *value_out = crate::handle::FfiString::from_string(value.clone()).into_raw();
    0
}

/// Read the next chunk from the streaming response.
///
/// Blocks until a chunk is available. Returns null when the stream is
/// exhausted or has been cancelled.
///
/// Caller must free the returned chunk with [`eggfetch_stream_chunk_free`].
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_next(
    handle: *mut StreamingResponseHandle,
) -> *mut StreamChunk {
    let Some(handle) = handle.as_mut() else {
        return ptr::null_mut();
    };
    match handle.rx.blocking_recv() {
        Some(Ok(data)) => {
            if handle.cancel.load(Ordering::Acquire) {
                return ptr::null_mut();
            }
            let len = data.len();
            let buf = if len > 0 {
                let layout =
                    std::alloc::Layout::array::<u8>(len).unwrap_or_else(|_| std::process::abort());
                let buf = std::alloc::alloc(layout);
                std::ptr::copy_nonoverlapping(data.as_ptr(), buf, len);
                buf
            } else {
                ptr::null_mut()
            };
            Box::into_raw(Box::new(StreamChunk { data: buf, len }))
        }
        _ => ptr::null_mut(),
    }
}

/// Free a stream chunk.
///
/// # Safety
///
/// `chunk` must have been returned by [`eggfetch_response_stream_next`].
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_stream_chunk_free(chunk: *mut StreamChunk) {
    if !chunk.is_null() {
        let c = Box::from_raw(chunk);
        if !c.data.is_null() && c.len > 0 {
            let layout =
                std::alloc::Layout::array::<u8>(c.len).unwrap_or_else(|_| std::process::abort());
            std::alloc::dealloc(c.data, layout);
        }
    }
}

/// Cancel an in-progress streaming response.
///
/// Subsequent calls to [`eggfetch_response_stream_next`] will return null.
///
/// # Safety
///
/// `handle` must be a valid, non-freed streaming response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_stream_cancel(handle: *mut StreamingResponseHandle) {
    if let Some(handle) = handle.as_mut() {
        handle.cancel.store(true, Ordering::Release);
        handle.rx.close();
    }
}
