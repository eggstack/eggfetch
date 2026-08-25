//! Request builder FFI functions.

use crate::handle::RequestHandle;

/// Free a request handle.
///
/// # Safety
///
/// `handle` must have been returned by an eggfetch FFI function and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_free(handle: *mut RequestHandle) {
    crate::ffi_guard!((), {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

/// Add a header to the request.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `name` and `value` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_header(
    handle: *mut RequestHandle,
    name: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(name) = crate::handle::cstr_to_string(name) else {
            return -1;
        };
        let Some(value) = crate::handle::cstr_to_string(value) else {
            return -1;
        };
        update_request(handle, |rb| rb.header(&name, &value))
    })
}

/// Add a query parameter to the request URL.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `key` and `value` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_query(
    handle: *mut RequestHandle,
    key: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(key) = crate::handle::cstr_to_string(key) else {
            return -1;
        };
        let Some(value) = crate::handle::cstr_to_string(value) else {
            return -1;
        };
        update_request(handle, |rb| rb.query(&key, &value))
    })
}

/// Set the request body from a byte buffer.
///
/// Returns 0 on success, -1 if handle or data are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `data` must point to at least `len` bytes of valid memory.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_body(
    handle: *mut RequestHandle,
    data: *const u8,
    len: usize,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        if data.is_null() {
            return -1;
        }
        let slice = std::slice::from_raw_parts(data, len);
        update_request(handle, |rb| rb.bytes(slice))
    })
}

/// Set the request body from a string.
///
/// Returns 0 on success, -1 if handle or body are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `body` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_body_str(
    handle: *mut RequestHandle,
    body: *const std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(body) = crate::handle::cstr_to_string(body) else {
            return -1;
        };
        update_request(handle, |rb| rb.bytes(body.into_bytes()))
    })
}

/// Set a per-request timeout in seconds.
///
/// Overrides the client-level timeout for this request.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed request handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_timeout(handle: *mut RequestHandle, secs: u64) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_request(handle, |rb| {
            rb.timeout(eggfetch_core::Timeout::from_secs(secs))
        })
    })
}

/// Set basic auth credentials on the request.
///
/// Overrides any client-level auth for this request.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `username` and `password` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_auth_basic(
    handle: *mut RequestHandle,
    username: *const std::os::raw::c_char,
    password: *const std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(username_str) = crate::handle::cstr_to_string(username) else {
            return -1;
        };
        let Some(password_str) = crate::handle::cstr_to_string(password) else {
            return -1;
        };
        let Ok(auth) = eggfetch_core::AuthScheme::basic(username_str, password_str) else {
            return -1;
        };
        update_request(handle, |rb| rb.auth(auth))
    })
}

/// Set a bearer auth token on the request.
///
/// Overrides any client-level auth for this request.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed request handle.
/// - `token` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_auth_bearer(
    handle: *mut RequestHandle,
    token: *const std::os::raw::c_char,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(token_str) = crate::handle::cstr_to_string(token) else {
            return -1;
        };
        let Ok(auth) = eggfetch_core::AuthScheme::bearer(token_str) else {
            return -1;
        };
        update_request(handle, |rb| rb.auth(auth))
    })
}

/// Remove auth from this request (opt out of client-level auth).
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed request handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_without_auth(handle: *mut RequestHandle) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_request(handle, eggfetch_core::RequestBuilder::without_auth)
    })
}

/// Enable or disable automatic decompression for this request.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed request handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_decompress(
    handle: *mut RequestHandle,
    enabled: i32,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_request(handle, |rb| rb.decompress(enabled != 0))
    })
}

/// Set a per-request redirect policy.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed request handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_request_redirect_policy(
    handle: *mut RequestHandle,
    follow: i32,
    max_redirects: usize,
) -> i32 {
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_request(handle, |rb| {
            rb.redirect_policy(eggfetch_core::RedirectPolicy::new(
                follow != 0,
                max_redirects,
            ))
        })
    })
}

fn update_request<F>(handle: &mut RequestHandle, update: F) -> i32
where
    F: FnOnce(eggfetch_core::RequestBuilder) -> eggfetch_core::RequestBuilder,
{
    let Some(builder) = handle.0.take() else {
        return -1;
    };
    handle.0 = Some(update(builder));
    0
}
