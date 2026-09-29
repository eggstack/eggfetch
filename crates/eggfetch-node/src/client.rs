//! Eggfetch Client for Node.js.

use napi_derive::napi;
use std::ptr;

use eggfetch_ffi::ErrorHandle;

/// Shared ownership for the FFI client handle.
///
/// In-flight requests hold an `Arc` clone for the duration of the
/// `spawn_blocking` future, so `Drop for EggfetchClient` cannot free the
/// handle while a request is outstanding. This does not rely on napi-rs
/// `this`-reference tracking (napi 2.x codegen); the `napi = "2"` pin in
/// `crates/eggfetch-node/Cargo.toml` must be kept with this comment.
struct ClientHandleInner {
    ptr: std::sync::Mutex<*mut eggfetch_ffi::ClientHandle>,
}

// SAFETY: the pointee is `Send + Sync` per eggfetch-ffi; the raw pointer is
// only accessed under the mutex and is freed exactly once by
// `Drop for ClientHandleInner`.
unsafe impl Send for ClientHandleInner {}
unsafe impl Sync for ClientHandleInner {}

impl Drop for ClientHandleInner {
    fn drop(&mut self) {
        // Free even on poison (no leak); poison reports the inner ptr.
        let mut guard = self
            .ptr
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ptr = std::mem::replace(&mut *guard, std::ptr::null_mut());
        if !ptr.is_null() {
            unsafe {
                eggfetch_ffi::eggfetch_client_free(ptr);
            }
        }
    }
}

/// HTTP client wrapping eggfetch-ffi.
///
/// The underlying `ClientHandle` is shared via `Arc<ClientHandleInner>` so
/// in-flight `spawn_blocking` requests extend its lifetime independently of
/// napi-rs finalization order. The raw pointer preserves provenance (no
/// `usize ↔ *mut` round-trip); the underlying `ClientHandle` is `Send + Sync`
/// per eggfetch-ffi documentation.
///
/// # Lifetime safety for in-flight requests
///
/// Each request clones the `Arc` into its `'static` future before entering
/// `spawn_blocking`. `Drop for ClientHandleInner` frees the FFI handle only
/// when the last clone (client plus all in-flight requests) is dropped, so
/// a JS garbage-collection of the client object cannot free the handle
/// underneath a running request. Do not replace this with a pattern that
/// frees the handle from `Drop for EggfetchClient` directly without holding
/// the shared `Arc` alive in the future.
///
/// This remains an experimental prototype (see `lib.rs`): requests execute
/// through the synchronous C ABI inside `spawn_blocking`.
#[napi]
pub struct EggfetchClient {
    inner: std::sync::Arc<ClientHandleInner>,
}

#[napi]
impl EggfetchClient {
    /// Create a new client with default settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying FFI client allocation fails.
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        let inner = unsafe { eggfetch_ffi::eggfetch_client_new() };
        if inner.is_null() {
            return Err(napi::Error::from_reason(
                "failed to create eggfetch client: allocation failed or runtime unavailable",
            ));
        }
        Ok(Self {
            inner: std::sync::Arc::new(ClientHandleInner {
                ptr: std::sync::Mutex::new(inner),
            }),
        })
    }

    /// Send a GET request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL is invalid.
    #[napi]
    pub async fn get(&self, url: String) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("GET", &url, None).await
    }

    /// Send a POST request with optional body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL/body is invalid.
    #[napi]
    pub async fn post(
        &self,
        url: String,
        body: Option<String>,
    ) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("POST", &url, body.as_deref()).await
    }

    /// Send a PUT request with optional body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL/body is invalid.
    #[napi]
    pub async fn put(
        &self,
        url: String,
        body: Option<String>,
    ) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("PUT", &url, body.as_deref()).await
    }

    /// Send a PATCH request with optional body.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL/body is invalid.
    #[napi]
    pub async fn patch(
        &self,
        url: String,
        body: Option<String>,
    ) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("PATCH", &url, body.as_deref()).await
    }

    /// Send a DELETE request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL is invalid.
    #[napi]
    pub async fn delete(&self, url: String) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("DELETE", &url, None).await
    }

    /// Send a HEAD request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL is invalid.
    #[napi]
    pub async fn head(&self, url: String) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("HEAD", &url, None).await
    }

    /// Send an OPTIONS request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the URL is invalid.
    #[napi]
    pub async fn options(&self, url: String) -> napi::Result<crate::EggfetchResponse> {
        self.send_request("OPTIONS", &url, None).await
    }

    /// Send a request with a custom method.
    ///
    /// The optional body is UTF-8 text only; arbitrary binary payloads
    /// (interior NUL) are rejected with an error. Use a byte-oriented
    /// API for binary uploads.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the method/URL is invalid.
    #[napi]
    pub async fn request(
        &self,
        method: String,
        url: String,
        body: Option<String>,
    ) -> napi::Result<crate::EggfetchResponse> {
        self.send_request(&method, &url, body.as_deref()).await
    }
}

impl EggfetchClient {
    fn send_request(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
    ) -> impl std::future::Future<Output = napi::Result<crate::EggfetchResponse>> + Send {
        // Clone the shared handle so the FFI client outlives this future
        // even if the JS object is collected first.
        let shared = std::sync::Arc::clone(&self.inner);
        let method = method.to_owned();
        let url = url.to_owned();
        let body = body.map(String::from);

        async move {
            napi::bindgen_prelude::spawn_blocking(move || {
                let client = shared
                    .ptr
                    .lock()
                    .map_or(std::ptr::null_mut(), |guard| *guard);
                if client.is_null() {
                    return Err(napi::Error::from_reason("client is closed"));
                }
                // Keep `shared` alive for the whole blocking section.
                let _keep_alive = &shared;
                let method_c = std::ffi::CString::new(method)
                    .map_err(|e| napi::Error::from_reason(format!("invalid method string: {e}")))?;
                let url_c = std::ffi::CString::new(url)
                    .map_err(|e| napi::Error::from_reason(format!("invalid url string: {e}")))?;

                let req = unsafe {
                    eggfetch_ffi::eggfetch_client_request(client, method_c.as_ptr(), url_c.as_ptr())
                };
                if req.is_null() {
                    return Err(napi::Error::from_reason("failed to create request"));
                }

                if let Some(body_str) = &body {
                    let body_c = match std::ffi::CString::new(body_str.as_str()) {
                        Ok(body_c) => body_c,
                        Err(e) => {
                            unsafe {
                                eggfetch_ffi::eggfetch_request_free(req);
                            }
                            return Err(napi::Error::from_reason(format!(
                                "invalid body string: {e}"
                            )));
                        }
                    };
                    let rc =
                        unsafe { eggfetch_ffi::eggfetch_request_body_str(req, body_c.as_ptr()) };
                    if rc != 0 {
                        unsafe {
                            eggfetch_ffi::eggfetch_request_free(req);
                        }
                        return Err(napi::Error::from_reason(format!(
                            "failed to set request body (code {rc})"
                        )));
                    }
                }

                let mut err: *mut ErrorHandle = ptr::null_mut();
                let resp = unsafe { eggfetch_ffi::eggfetch_client_send(client, req, &raw mut err) };

                if resp.is_null() {
                    if !err.is_null() {
                        let kind = unsafe { eggfetch_ffi::eggfetch_error_kind(err) };
                        let msg = unsafe { eggfetch_ffi::eggfetch_error_message(err) };
                        let kind_str = if kind.is_null() {
                            "unknown".to_owned()
                        } else {
                            unsafe { std::ffi::CStr::from_ptr(kind) }
                                .to_string_lossy()
                                .into_owned()
                        };
                        let msg_str = if msg.is_null() {
                            "unknown error".to_owned()
                        } else {
                            unsafe { std::ffi::CStr::from_ptr(msg) }
                                .to_string_lossy()
                                .into_owned()
                        };
                        unsafe {
                            if !kind.is_null() {
                                eggfetch_ffi::eggfetch_string_free(kind);
                            }
                            if !msg.is_null() {
                                eggfetch_ffi::eggfetch_string_free(msg);
                            }
                            eggfetch_ffi::eggfetch_error_free(err);
                        }
                        return Err(napi::Error::from_reason(format!(
                            "eggfetch error [{kind_str}]: {msg_str}"
                        )));
                    }
                    return Err(napi::Error::from_reason(
                        "request failed with unknown error",
                    ));
                }

                crate::EggfetchResponse::from_raw(resp)
            })
            .await
            .map_err(|e| napi::Error::from_reason(format!("request worker failed: {e}")))?
        }
    }
}
