//! Client FFI functions.

use std::ptr;

use eggfetch_core::Client;

use crate::handle::{ClientHandle, ErrorHandle, RequestHandle};
use crate::runtime::blocking_send;

/// Create a new client with default settings.
///
/// Returns null on allocation failure.
///
/// # Safety
///
/// Caller must free the returned handle with [`client_free`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_new() -> *mut ClientHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        Box::into_raw(Box::new(ClientHandle(Client::new())))
    })
}

/// Free a client handle.
///
/// # Safety
///
/// `handle` must have been returned by [`eggfetch_client_new`] and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_free(handle: *mut ClientHandle) {
    crate::ffi_guard!((), {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

/// Begin constructing a request with the given HTTP method and URL.
///
/// Returns null on allocation failure or if method/url are invalid.
///
/// # Safety
///
/// - `client` must be a valid, non-freed handle.
/// - `method` and `url` must be valid null-terminated C strings.
/// - Caller must free the returned handle with [`request_free`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_request(
    client: *const ClientHandle,
    method: *const std::os::raw::c_char,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(method_str) = crate::handle::cstr_to_opt(method) else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        let Ok(http_method) = method_str.parse() else {
            return ptr::null_mut();
        };
        match client.0.request(http_method, url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a GET request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_get(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.get(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a POST request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_post(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.post(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a PUT request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_put(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.put(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a PATCH request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_patch(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.patch(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a DELETE request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_delete(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.delete(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin a HEAD request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_head(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.head(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Convenience: begin an OPTIONS request.
///
/// # Safety
///
/// Same as [`eggfetch_client_request`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_options(
    client: *const ClientHandle,
    url: *const std::os::raw::c_char,
) -> *mut RequestHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(client) = client.as_ref() else {
            return ptr::null_mut();
        };
        let Some(url_str) = crate::handle::cstr_to_opt(url) else {
            return ptr::null_mut();
        };
        match client.0.options(url_str) {
            Ok(rb) => Box::into_raw(Box::new(RequestHandle(Some(rb)))),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Send a request and receive the response synchronously.
///
/// This blocks the calling thread while the async I/O completes.
///
/// On success, returns a response handle (caller must free with [`crate::response::eggfetch_response_free`]).
/// On failure, sets `*err_out` to a new error handle (caller must free with [`crate::error::eggfetch_error_free`]).
/// If `err_out` is null, the error is silently dropped.
///
/// # Safety
///
/// - `request` must be a valid, non-freed handle. The handle is consumed (freed) by this call.
/// - `err_out` must be null or point to a `*mut ErrorHandle` slot.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_send(
    client: *const ClientHandle,
    request: *mut RequestHandle,
    err_out: *mut *mut ErrorHandle,
) -> *mut crate::handle::ResponseHandle {
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

            // Consume the request by taking ownership of the builder.
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

            // Send the request and read the body in a single async block to
            // avoid cross-runtime issues with streaming body handles.
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
                let body = resp.bytes().await?;
                Ok::<_, eggfetch_core::Error>((status, url, headers, body))
            });

            match result {
                Ok((status, url, headers, body)) => {
                    Box::into_raw(Box::new(crate::handle::ResponseHandle {
                        status,
                        url,
                        headers,
                        body: body.to_vec(),
                    }))
                }
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
