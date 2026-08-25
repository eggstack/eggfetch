//! Additional response FFI functions for convenience.

use crate::handle::ResponseHandle;

/// Convenience: get the response body as a text string.
///
/// Returns a newly allocated C string. Caller must free with [`crate::handle::eggfetch_string_free`].
/// Returns null on error, if handle is null, if the body is not valid
/// UTF-8, or if it contains an interior null byte.
///
/// # Safety
///
/// `handle` must be a valid, non-freed response handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_response_text(
    handle: *const ResponseHandle,
) -> *mut std::os::raw::c_char {
    crate::ffi_guard!(std::ptr::null_mut(), {
        let Some(handle) = handle.as_ref() else {
            return std::ptr::null_mut();
        };
        match std::str::from_utf8(&handle.body) {
            Ok(s) => crate::handle::FfiString::from_string(s.to_owned())
                .map_or_else(std::ptr::null_mut, crate::handle::FfiString::into_raw),
            Err(_) => std::ptr::null_mut(),
        }
    })
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
    crate::ffi_guard!(0, {
        handle
            .as_ref()
            .map_or(0, |h| i32::from((200..300).contains(&h.status)))
    })
}
