//! Response FFI functions.

use crate::handle::ResponseHandle;

/// Free a response handle.
///
/// # Safety
///
/// `handle` must have been returned by an eggfetch FFI function and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_free(handle: *mut ResponseHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Get the HTTP status code of the response.
///
/// Returns 0 if handle is null.
///
/// # Safety
///
/// `handle` must be a valid, non-freed response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_status(handle: *const ResponseHandle) -> u16 {
    handle.as_ref().map_or(0, |h| h.status)
}

/// Get the response URL as a newly allocated C string.
///
/// Returns null if handle is null. Caller must free with [`crate::handle::eggfetch_string_free`].
///
/// # Panics
///
/// Panics if the URL string contains an interior null byte.
///
/// # Safety
///
/// `handle` must be a valid, non-freed response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_url(
    handle: *const ResponseHandle,
) -> *mut std::os::raw::c_char {
    let Some(handle) = handle.as_ref() else {
        return std::ptr::null_mut();
    };
    crate::handle::FfiString::from_string(handle.url.clone()).into_raw()
}

/// Get the number of response headers.
///
/// Returns 0 if handle is null.
///
/// # Safety
///
/// `handle` must be a valid, non-freed response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_header_count(handle: *const ResponseHandle) -> usize {
    handle.as_ref().map_or(0, |h| h.headers.len())
}

/// Get a response header by index.
///
/// On success, sets `*name_out` and `*value_out` to newly allocated C strings.
/// Caller must free both with [`crate::handle::eggfetch_string_free`].
///
/// Returns 0 on success, -1 on error (invalid index or null handle).
///
/// # Panics
///
/// Panics if the header name or value contains an interior null byte.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed response handle.
/// - `name_out` and `value_out` must be valid pointers to `*mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_header(
    handle: *const ResponseHandle,
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

/// Get the response body as a newly allocated byte buffer.
///
/// On success, sets `*data_out` to a newly allocated buffer and `*len_out` to its length.
/// Caller must free the buffer with [`eggfetch_body_free`].
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed response handle.
/// - `data_out` and `len_out` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_body(
    handle: *const ResponseHandle,
    data_out: *mut *mut u8,
    len_out: *mut usize,
) -> i32 {
    let Some(handle) = handle.as_ref() else {
        return -1;
    };
    if data_out.is_null() || len_out.is_null() {
        return -1;
    }
    let len = handle.body.len();
    // Safety: We check len > 0 implicitly — alloc with len=0 returns a non-null dangling pointer
    // which is valid for zero-length reads. We use abort-on-layout-error for safety.
    let layout = std::alloc::Layout::array::<u8>(len).unwrap_or_else(|_| std::process::abort());
    let buf = std::alloc::alloc(layout);
    std::ptr::copy_nonoverlapping(handle.body.as_ptr(), buf, len);
    *data_out = buf;
    *len_out = len;
    0
}

/// Free a body buffer allocated by [`eggfetch_response_body`].
///
/// # Safety
///
/// `data` must have been returned by [`eggfetch_response_body`].
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_body_free(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        let layout = std::alloc::Layout::array::<u8>(len).unwrap_or_else(|_| std::process::abort());
        std::alloc::dealloc(data, layout);
    }
}
