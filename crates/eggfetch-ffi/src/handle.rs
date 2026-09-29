//! Opaque handle types for FFI.

use std::ffi::CString;
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
    pub(crate) body: bytes::Bytes,
}

/// Opaque handle to an error.
///
/// Single-thread, single-use. Must be freed after inspection.
pub struct ErrorHandle {
    pub(crate) kind: String,
    pub(crate) message: String,
}

/// Owned C string returned by FFI. Must be freed with [`eggfetch_string_free`].
#[repr(C)]
pub struct FfiString {
    ptr: *mut c_char,
}

impl FfiString {
    /// Create an owned C string from a Rust [`String`].
    ///
    /// Returns `None` if the string contains an interior null byte. Callers
    /// must surface that case as a null pointer or error code; silently
    /// substituting placeholder content would corrupt the reported value.
    ///
    /// # Safety
    ///
    /// Caller must free the returned [`FfiString`] with [`eggfetch_string_free`].
    #[must_use]
    pub unsafe fn from_string(s: String) -> Option<Self> {
        let c = CString::new(s).ok()?;
        Some(Self { ptr: c.into_raw() })
    }

    /// Create an owned C string from a static str.
    ///
    /// Returns `None` if the string contains an interior null byte.
    ///
    /// # Safety
    ///
    /// Caller must free the returned [`FfiString`] with [`eggfetch_string_free`].
    #[must_use]
    pub unsafe fn from_static(s: &'static str) -> Option<Self> {
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

/// Read a C string from a pointer, returning an owned copy (`None` if null).
///
/// Returns owned data because a raw pointer carries no lifetime to borrow
/// against; every caller immediately consumes the value into owned state.
/// Maximum C-string scan length (1 MiB) to bound `cstr_to_string`.
///
/// The C ABI requires scanning for NUL, but an unterminated pointer must not
/// read unbounded memory: fail closed past this limit. `MAX_FFI_BODY_LEN`
/// (256 MiB) covers only `request_body` byte buffers, not NUL-terminated strings.
pub(crate) const MAX_CSTR_LEN: usize = 1024 * 1024;
///
/// # Safety
///
/// `ptr` must be null or point to a valid null-terminated C string.
/// This performs a bounded scan for the terminator (up to `MAX_CSTR_LEN`);
/// unterminated or over-long inputs return `None` instead of reading OOB.
pub(crate) unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        // Bounded `strnlen`: stop at NUL or MAX_CSTR_LEN.
        let mut len = 0usize;
        while len < MAX_CSTR_LEN {
            let byte = unsafe { *ptr.add(len) }.cast_unsigned();
            if byte == 0 {
                break;
            }
            len += 1;
        }
        if len >= MAX_CSTR_LEN {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
        String::from_utf8(slice.to_vec()).ok()
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
        // Redact `user:pass@` userinfo: core messages may echo the request
        // URL, and the C string crosses the ABI verbatim otherwise.
        message: redact_credentials(&e.to_string()),
    })
}

/// Redact `user:pass@` userinfo from error strings.
///
/// Replaces every `://<userinfo>@` with `://<redacted>@` (mirrors the
/// Python binding's redaction so C callers get the same guarantee).
fn redact_credentials(message: &str) -> String {
    let mut result = message.to_owned();
    let mut search_from = 0;
    while let Some(scheme_pos) = result[search_from..].find("://") {
        let userinfo_start = search_from + scheme_pos + 3;
        let rest = &result[userinfo_start..];
        let Some(at_pos) = rest.find('@') else {
            break;
        };
        let terminator = [
            rest.find('/'),
            rest.find(' '),
            rest.find('"'),
            rest.find('\''),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
        if at_pos < terminator {
            result.replace_range(userinfo_start..=userinfo_start + at_pos, "<redacted>@");
            search_from = userinfo_start + "<redacted>@".len();
        } else {
            search_from = userinfo_start;
        }
        if search_from >= result.len() {
            break;
        }
    }
    result
}
