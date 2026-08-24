//! Eggfetch Client for Node.js.

use napi_derive::napi;
use std::ptr;

use eggfetch_ffi::ErrorHandle;

/// HTTP client wrapping eggfetch-ffi.
///
/// The client pointer is stored as `usize` to satisfy napi's `Send` requirement
/// for async futures. The underlying `ClientHandle` is `Send + Sync` per
/// eggfetch-ffi documentation.
#[napi]
pub struct EggfetchClient {
    inner: usize,
}

impl Default for EggfetchClient {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl EggfetchClient {
    /// Create a new client with default settings.
    ///
    /// # Panics
    ///
    /// Panics if the underlying FFI client allocation fails.
    #[napi(constructor)]
    pub fn new() -> Self {
        let inner = unsafe { eggfetch_ffi::eggfetch_client_new() };
        assert!(!inner.is_null(), "failed to create eggfetch client");
        Self {
            inner: inner as usize,
        }
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
        let client_ptr = self.inner;
        let method = method.to_owned();
        let url = url.to_owned();
        let body = body.map(String::from);

        async move {
            napi::bindgen_prelude::spawn_blocking(move || {
                let client = client_ptr as *mut eggfetch_ffi::ClientHandle;
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
                    unsafe {
                        eggfetch_ffi::eggfetch_request_body_str(req, body_c.as_ptr());
                    }
                }

                let mut err: *mut ErrorHandle = ptr::null_mut();
                let resp = unsafe { eggfetch_ffi::eggfetch_client_send(client, req, &mut err) };

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

                Ok(crate::EggfetchResponse::from_raw(resp))
            })
            .await
            .map_err(|e| napi::Error::from_reason(format!("request worker failed: {e}")))?
        }
    }
}

impl Drop for EggfetchClient {
    fn drop(&mut self) {
        if self.inner != 0 {
            unsafe {
                eggfetch_ffi::eggfetch_client_free(self.inner as *mut eggfetch_ffi::ClientHandle);
            }
        }
    }
}
