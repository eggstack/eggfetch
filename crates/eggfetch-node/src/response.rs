//! Eggfetch Response for Node.js.

use napi_derive::napi;
use std::collections::HashMap;
use std::ptr;

/// HTTP response from eggfetch.
#[napi]
pub struct EggfetchResponse {
    status: u32,
    url: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

// Safety: Response data is fully consumed at creation and owned by this struct
unsafe impl Send for EggfetchResponse {}

impl EggfetchResponse {
    /// Create from a raw FFI response handle, consuming it immediately.
    pub(crate) fn from_raw(resp: *mut eggfetch_ffi::ResponseHandle) -> Self {
        unsafe {
            let status = u32::from(eggfetch_ffi::eggfetch_response_status(resp));
            let url_ptr = eggfetch_ffi::eggfetch_response_url(resp);
            let url = if url_ptr.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(url_ptr)
                    .to_string_lossy()
                    .into_owned()
            };
            if !url_ptr.is_null() {
                eggfetch_ffi::eggfetch_string_free(url_ptr);
            }

            let header_count = eggfetch_ffi::eggfetch_response_header_count(resp);
            let mut headers = HashMap::with_capacity(header_count);
            for i in 0..header_count {
                let mut name: *mut std::os::raw::c_char = ptr::null_mut();
                let mut value: *mut std::os::raw::c_char = ptr::null_mut();
                let rc = eggfetch_ffi::eggfetch_response_header(resp, i, &mut name, &mut value);
                if rc == 0 {
                    let n = if name.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(name)
                            .to_string_lossy()
                            .into_owned()
                    };
                    let v = if value.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(value)
                            .to_string_lossy()
                            .into_owned()
                    };
                    if !name.is_null() {
                        eggfetch_ffi::eggfetch_string_free(name);
                    }
                    if !value.is_null() {
                        eggfetch_ffi::eggfetch_string_free(value);
                    }
                    headers.insert(n, v);
                }
            }

            let text_ptr = eggfetch_ffi::eggfetch_response_text(resp);
            let body = if text_ptr.is_null() {
                Vec::new()
            } else {
                let s = std::ffi::CStr::from_ptr(text_ptr)
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes();
                eggfetch_ffi::eggfetch_string_free(text_ptr);
                s
            };

            eggfetch_ffi::eggfetch_response_free(resp);

            Self {
                status,
                url,
                headers,
                body,
            }
        }
    }
}

#[napi]
impl EggfetchResponse {
    /// HTTP status code.
    #[napi(getter)]
    pub fn status(&self) -> u32 {
        self.status
    }

    /// Response URL.
    #[napi(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Response body as text.
    #[napi(getter)]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Response body as JSON (parsed).
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid JSON.
    #[napi(getter)]
    pub fn json(&self) -> napi::Result<serde_json::Value> {
        serde_json::from_slice(&self.body)
            .map_err(|e| napi::Error::from_reason(format!("JSON parse error: {e}")))
    }

    /// Response headers as an object.
    #[napi(getter)]
    pub fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }

    /// Whether the response status is 2xx.
    #[napi(getter)]
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}
