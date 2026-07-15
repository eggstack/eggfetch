//! Additional response FFI functions for convenience.

use crate::handle::ResponseHandle;

/// Convenience: get the response body as a text string.
///
/// Returns a newly allocated C string. Caller must free with [`crate::handle::eggfetch_string_free`].
/// Returns null on error or if handle is null.
///
/// # Panics
///
/// Panics if the body contains valid UTF-8 with an interior null byte.
///
/// # Safety
///
/// `handle` must be a valid, non-freed response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_text(
    handle: *const ResponseHandle,
) -> *mut std::os::raw::c_char {
    let Some(handle) = handle.as_ref() else {
        return std::ptr::null_mut();
    };
    match std::str::from_utf8(&handle.body) {
        Ok(s) => crate::handle::FfiString::from_string(s.to_owned()).into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Check if the response status indicates success (2xx).
///
/// Returns 1 if success, 0 otherwise. Returns 0 if handle is null.
///
/// # Safety
///
/// `handle` may be null.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_is_success(handle: *const ResponseHandle) -> i32 {
    handle
        .as_ref()
        .map_or(0, |h| i32::from((200..300).contains(&h.status)))
}
