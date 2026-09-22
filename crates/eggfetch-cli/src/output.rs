//! Private CLI presentation and machine-output helpers.

use std::time::Duration;

use serde_json::{json, Value};

pub(crate) fn version_string(v: http::Version) -> &'static str {
    match v {
        http::Version::HTTP_09 => "0.9",
        http::Version::HTTP_10 => "1.0",
        http::Version::HTTP_11 => "1.1",
        http::Version::HTTP_2 => "2",
        http::Version::HTTP_3 => "3",
        _ => "unknown",
    }
}

const SECRET_HEADER_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

pub(crate) fn is_secret_header(name: &str) -> bool {
    SECRET_HEADER_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

pub(crate) fn is_binary_content_type(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.contains("octet-stream")
        || ct.contains("image/")
        || ct.contains("audio/")
        || ct.contains("video/")
        || ct.contains("application/pdf")
        || ct.contains("application/zip")
        || ct.contains("application/gzip")
}

pub(crate) fn format_header_value(name: &str, value: &str, redact: bool) -> String {
    if redact && is_secret_header(name) {
        if name.eq_ignore_ascii_case("authorization")
            || name.eq_ignore_ascii_case("proxy-authorization")
        {
            if let Some(space_pos) = value.find(' ') {
                let scheme = &value[..space_pos];
                format!("{scheme} <redacted>")
            } else {
                "<redacted>".to_owned()
            }
        } else {
            "<redacted>".to_owned()
        }
    } else {
        value.to_owned()
    }
}

pub(crate) fn format_headers(
    headers: &http::HeaderMap,
    verbose: bool,
    redact_secrets: bool,
) -> String {
    let mut out = String::new();
    for (name, value) in headers {
        if !verbose && name.as_str() == "set-cookie" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            out.push_str(name.as_str());
            out.push_str(": ");
            out.push_str(&format_header_value(name.as_str(), v, redact_secrets));
            out.push_str("\r\n");
        }
    }
    out
}

pub(crate) fn format_headers_machine(headers: &http::HeaderMap) -> Vec<Value> {
    headers
        .iter()
        .map(|(name, value)| {
            let raw = value.to_str().unwrap_or("<binary>");
            json!([name.as_str(), format_header_value(name.as_str(), raw, true)])
        })
        .collect()
}

pub(crate) fn build_json_response(
    response: &eggfetch_core::Response,
    elapsed: Duration,
    body_len: Option<usize>,
    include_body_b64: bool,
    body_b64: Option<&str>,
    errors: &[String],
) -> Value {
    let headers = format_headers_machine(response.headers());

    let history: Vec<Value> = response
        .history()
        .iter()
        .map(|entry| {
            json!({
                "status": entry.status().as_u16(),
                "url": entry.url().to_string(),
                "version": version_string(entry.version()),
            })
        })
        .collect();

    let mut obj = json!({
        "url": response.url().to_string(),
        "status": response.status().as_u16(),
        "version": version_string(response.version()),
        "headers": headers,
        "elapsed_ms": elapsed.as_millis(),
        "history": history,
        "body_length": body_len,
        "errors": errors,
    });

    if include_body_b64 {
        if let Some(b64) = body_b64 {
            obj["body_base64"] = json!(b64);
        }
    }

    obj
}

// Reduce a server-supplied filename to a safe single path component.
//
// Strips any directory components (`/` and `\`, including Windows
// separators), then rejects empty names, `.`, and `..` so a hostile

pub(crate) fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = if chunk.len() > 1 {
            u32::from(chunk[1])
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            u32::from(chunk[2])
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
