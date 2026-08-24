//! Opaque handle types for FFI.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use eggfetch_core::Client;

/// Opaque handle to a client builder.
///
/// Single-thread, single-use. Must be consumed by building a client or freed.
pub struct ClientBuilderHandle(pub(crate) Option<eggfetch_core::ClientBuilder>);

/// Opaque handle to an HTTP client.
///
/// Thread-safe: may be shared across threads.
pub struct ClientHandle(pub(crate) Client);

/// Opaque handle to a request builder.
///
/// Single-thread, single-use. Must be freed after use.
pub struct RequestHandle(pub(crate) Option<eggfetch_core::RequestBuilder>);

/// Opaque handle to a completed response.
///
/// Single-thread, single-use. Must be freed after body is consumed.
pub struct ResponseHandle {
    pub(crate) status: u16,
    pub(crate) url: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

/// Opaque handle to an error.
///
/// Single-thread, single-use. Must be freed after inspection.
pub struct ErrorHandle {
    pub(crate) kind: String,
    pub(crate) message: String,
}

/// Owned C string returned by FFI. Must be freed with [`eggfetch_string_free`].
///
/// # Panics
///
/// Panics if the Rust string contains an interior null byte.
#[repr(C)]
pub struct FfiString {
    ptr: *mut c_char,
}

impl FfiString {
    /// Create an owned C string from a Rust [`String`].
    ///
    /// # Panics
    ///
    /// Panics if the string contains an interior null byte.
    ///
    /// # Safety
    ///
    /// Caller must free the returned [`FfiString`] with [`eggfetch_string_free`].
    #[must_use]
    pub unsafe fn from_string(s: String) -> Self {
        let c = CString::new(s).unwrap_or_else(|_| CString::new("<invalid utf8>").unwrap());
        Self { ptr: c.into_raw() }
    }

    /// Create an owned C string from a static str.
    ///
    /// # Panics
    ///
    /// Panics if the string contains an interior null byte.
    ///
    /// # Safety
    ///
    /// Caller must free the returned [`FfiString`] with [`eggfetch_string_free`].
    #[must_use]
    pub unsafe fn from_static(s: &'static str) -> Self {
        Self::from_string(s.to_owned())
    }

    /// Return the raw pointer. Caller takes ownership.
    #[must_use]
    pub fn into_raw(self) -> *mut c_char {
        let ptr = self.ptr;
        // Safety: We intentionally do not drop self here — the caller owns the pointer.
        let _ = std::mem::ManuallyDrop::new(self);
        ptr
    }
}

impl Drop for FfiString {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // Safety: ptr was created by CString::into_raw, so it's valid to reconstruct.
            unsafe {
                drop(CString::from_raw(self.ptr));
            }
        }
    }
}

/// Free a string returned by an eggfetch FFI function.
///
/// # Safety
///
/// `s` must have been returned by an eggfetch FFI function and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_string_free(s: *mut c_char) {
    crate::ffi_guard!((), {
        if !s.is_null() {
            drop(CString::from_raw(s));
        }
    });
}

/// Read a C string from a pointer, returning `None` if null.
///
/// # Safety
///
/// `ptr` must be null or point to a valid null-terminated C string.
pub(crate) unsafe fn cstr_to_opt(ptr: *const c_char) -> Option<&'static str> {
    if ptr.is_null() {
        None
    } else {
        CStr::from_ptr(ptr).to_str().ok()
    }
}

/// Convert an eggfetch error into an owned error handle.
///
/// Returns a `Box<ErrorHandle>` suitable for FFI raw-pointer return.
#[must_use]
#[allow(clippy::unnecessary_box_returns)]
pub(crate) fn error_to_handle(e: &eggfetch_core::Error) -> Box<ErrorHandle> {
    Box::new(ErrorHandle {
        kind: e.kind().to_owned(),
        message: e.to_string(),
    })
}
