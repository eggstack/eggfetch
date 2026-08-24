//! Error FFI functions.

use crate::handle::ErrorHandle;

/// Free an error handle.
///
/// # Safety
///
/// `handle` must have been returned by an eggfetch FFI function and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_error_free(handle: *mut ErrorHandle) {
    crate::ffi_guard!((), {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

/// Get the error kind as a newly allocated C string.
///
/// Caller must free with [`crate::handle::eggfetch_string_free`].
/// Returns null if handle is null.
///
/// # Panics
///
/// Panics if the error kind string contains an interior null byte.
///
/// # Safety
///
/// `handle` must be a valid, non-freed error handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_error_kind(
    handle: *const ErrorHandle,
) -> *mut std::os::raw::c_char {
    crate::ffi_guard!(std::ptr::null_mut(), {
        let Some(handle) = handle.as_ref() else {
            return std::ptr::null_mut();
        };
        crate::handle::FfiString::from_string(handle.kind.clone()).into_raw()
    })
}

/// Get the error message as a newly allocated C string.
///
/// Caller must free with [`crate::handle::eggfetch_string_free`].
/// Returns null if handle is null.
///
/// # Panics
///
/// Panics if the error message string contains an interior null byte.
///
/// # Safety
///
/// `handle` must be a valid, non-freed error handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_error_message(
    handle: *const ErrorHandle,
) -> *mut std::os::raw::c_char {
    crate::ffi_guard!(std::ptr::null_mut(), {
        let Some(handle) = handle.as_ref() else {
            return std::ptr::null_mut();
        };
        crate::handle::FfiString::from_string(handle.message.clone()).into_raw()
    })
}

/// Check whether an error handle is non-null (i.e., an error occurred).
///
/// Returns 1 if the pointer is non-null, 0 if null.
///
/// # Safety
///
/// `handle` may be null.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_error_is_error(handle: *const ErrorHandle) -> i32 {
    crate::ffi_guard!(0, { i32::from(!handle.is_null()) })
}
