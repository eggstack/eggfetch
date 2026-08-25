//! Eggfetch Response for Node.js.

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use std::collections::HashMap;
use std::ptr;

/// HTTP response from eggfetch.
#[napi]
pub struct EggfetchResponse {
    status: u32,
    url: String,
    /// All header pairs in wire order. Duplicates (e.g. multiple
    /// `Set-Cookie` headers) are preserved; use `getAll` to observe
    /// every value for a name.
    headers: Vec<(String, String)>,
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
            let mut headers = Vec::with_capacity(header_count);
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
                    headers.push((n, v));
                }
            }

            let mut body_ptr = ptr::null_mut();
            let mut body_len = 0;
            let body = if eggfetch_ffi::eggfetch_response_body(resp, &mut body_ptr, &mut body_len)
                == 0
                && body_len > 0
                && !body_ptr.is_null()
            {
                let body = std::slice::from_raw_parts(body_ptr, body_len).to_vec();
                eggfetch_ffi::eggfetch_body_free(body_ptr, body_len);
                body
            } else {
                Vec::new()
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

    /// Response body as a Node.js `Buffer`.
    #[napi(getter)]
    pub fn bytes(&self) -> Buffer {
        Buffer::from(self.body.clone())
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
    ///
    /// When a header appears multiple times, the object joins the values
    /// with `", "` (standard HTTP combining). Use [`Self::get_all`] to
    /// retrieve every value individually.
    #[napi(getter)]
    pub fn headers(&self) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::with_capacity(self.headers.len());
        for (name, value) in &self.headers {
            match map.entry(name.clone()) {
                std::collections::hash_map::Entry::Occupied(mut existing) => {
                    let joined = existing.get_mut();
                    joined.push_str(", ");
                    joined.push_str(value);
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(value.clone());
                }
            }
        }
        map
    }

    /// All values for a response header, case-insensitively.
    ///
    /// Unlike the `headers` object, this preserves duplicate headers
    /// such as multiple `Set-Cookie` lines.
    // N-API method arguments must be owned (`FromNapiValue`) values;
    // taking `&str` is not expressible here.
    #[allow(clippy::needless_pass_by_value)]
    #[napi]
    pub fn get_all(&self, name: String) -> Vec<String> {
        self.headers
            .iter()
            .filter(|(header_name, _)| header_name.eq_ignore_ascii_case(&name))
            .map(|(_, value)| value.clone())
            .collect()
    }

    /// Whether the response status is 2xx.
    #[napi(getter)]
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}
