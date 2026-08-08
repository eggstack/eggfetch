//! Unix domain socket (UDS) transport.
//!
//! Provides end-to-end HTTP over Unix domain sockets on supported platforms.
//! On non-Unix targets, attempts to use UDS produce a clear error at the
//! transport layer rather than a compile-time failure.
//!
//! # Architecture
//!
//! UDS transport bypasses the hyper client entirely. The request body is
//! buffered, sent as raw HTTP/1.1 bytes over the Unix socket, and the
//! response is parsed back into an eggfetch `Response`. This keeps the
//! implementation narrow: a single-purpose socket handler rather than a
//! generalized connector framework.

use bytes::{Bytes, BytesMut};

use crate::body::{RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::response::Response;

/// An HTTP-over-Unix-domain-socket handler.
///
/// Each handler owns the UDS path and creates fresh connections per request
/// (no connection reuse across different UDS paths). This provides natural
/// pool isolation: two different UDS paths cannot share connections.
#[derive(Debug, Clone)]
pub(crate) struct UdsHandler {
    /// Path to the Unix domain socket.
    path: String,
}

impl UdsHandler {
    /// Create a new UDS handler for the given socket path.
    pub(crate) fn new(path: String) -> Self {
        Self { path }
    }

    /// Send an HTTP request over the Unix domain socket and return the
    /// response.
    ///
    /// The request body is fully buffered before sending. The response is
    /// parsed from the raw HTTP/1.1 bytes received from the socket.
    pub(crate) async fn send_request(
        &self,
        method: &http::Method,
        url: &url::Url,
        headers: &Headers,
        body: RequestBody,
        version: http::Version,
    ) -> Result<Response> {
        // Buffer the request body.
        let body_bytes = body.into_bytes().await?;

        // Build the raw HTTP/1.1 request.
        let mut raw_request = BytesMut::new();

        // Request line.
        let path = if let Some(query) = url.query() {
            format!("{}?{}", url.path(), query)
        } else {
            url.path().to_owned()
        };
        let version_str = match version {
            http::Version::HTTP_10 => "HTTP/1.0",
            _ => "HTTP/1.1",
        };
        raw_request.extend_from_slice(format!("{method} {path} {version_str}\r\n").as_bytes());

        // Ensure Host header is present.
        let mut headers = headers.clone();
        if !headers.contains("host") {
            let host = if let Some(port) = url.port() {
                format!("{}:{}", url.host_str().unwrap_or(""), port)
            } else {
                url.host_str().unwrap_or("").to_owned()
            };
            headers.insert("host", &host)?;
        }

        // Headers.
        for (name, value) in headers.iter() {
            raw_request.extend_from_slice(
                format!("{}: {}\r\n", name, value.to_str().unwrap_or("")).as_bytes(),
            );
        }

        // End of headers.
        raw_request.extend_from_slice(b"\r\n");

        // Body.
        raw_request.extend_from_slice(&body_bytes);

        // Connect and send over Unix socket.
        #[cfg(unix)]
        {
            let raw_request = raw_request.freeze();
            send_over_uds(&self.path, &raw_request, url.clone()).await
        }

        #[cfg(not(unix))]
        {
            let _ = raw_request;
            Err(Error::Unsupported(
                "Unix domain sockets are not supported on this platform".into(),
            ))
        }
    }
}

/// Connect to a Unix domain socket, send the raw request, and parse the
/// response.
#[cfg(unix)]
async fn send_over_uds(path: &str, raw_request: &Bytes, url: url::Url) -> Result<Response> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::UnixStream::connect(path)
        .await
        .map_err(|e| Error::Connect(format!("UDS connect to {path} failed: {e}")))?;

    stream
        .write_all(raw_request)
        .await
        .map_err(|e| Error::Connect(format!("UDS write failed: {e}")))?;

    // Read the full response.
    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .await
        .map_err(|e| Error::Connect(format!("UDS read failed: {e}")))?;

    // Parse HTTP/1.1 response.
    parse_http_response(&response_bytes, url)
}

/// Parse a raw HTTP/1.1 response into an eggfetch `Response`.
fn parse_http_response(raw: &[u8], url: url::Url) -> Result<Response> {
    // Find the end of headers.
    let header_end = find_header_end(raw).ok_or_else(|| {
        Error::Protocol("failed to parse HTTP response from UDS: no header boundary found".into())
    })?;

    let header_bytes = &raw[..header_end];
    let body_bytes = &raw[header_end + 4..]; // Skip \r\n\r\n

    // Parse status line.
    let header_str = std::str::from_utf8(header_bytes)
        .map_err(|e| Error::Protocol(format!("invalid UTF-8 in response headers: {e}")))?;

    let mut lines = header_str.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| Error::Protocol("empty status line".into()))?;

    let (status_code, _reason) = parse_status_line(status_line)?;

    // Parse headers.
    let mut resp_headers = http::HeaderMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if let (Ok(name), Ok(value)) = (
                http::HeaderName::from_bytes(name.as_bytes()),
                http::HeaderValue::from_str(value),
            ) {
                resp_headers.append(name, value);
            }
        }
    }

    let status = http::StatusCode::from_u16(status_code)
        .map_err(|e| Error::Protocol(format!("invalid status code: {e}")))?;

    // Wrap body as a streaming response.
    let body_data = Bytes::copy_from_slice(body_bytes);
    let stream = futures_util::stream::once(async move { Ok(body_data) });
    let body = ResponseBody::streaming(Box::pin(stream));

    Ok(Response::new(
        status,
        http::Version::HTTP_11,
        resp_headers,
        url,
        body,
    ))
}

/// Find the `\r\n\r\n` boundary between headers and body.
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Parse the HTTP status line (e.g., `HTTP/1.1 200 OK`).
fn parse_status_line(line: &str) -> Result<(u16, &str)> {
    let mut parts = line.splitn(3, ' ');
    let _version = parts
        .next()
        .ok_or_else(|| Error::Protocol("missing HTTP version in status line".into()))?;
    let code_str = parts
        .next()
        .ok_or_else(|| Error::Protocol("missing status code in status line".into()))?;
    let reason = parts.next().unwrap_or("");
    let code: u16 = code_str
        .parse()
        .map_err(|e| Error::Protocol(format!("invalid status code '{code_str}': {e}")))?;
    Ok((code, reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_header_end_basic() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        let pos = find_header_end(data);
        assert_eq!(pos, Some(34));
    }

    #[test]
    fn find_header_end_missing() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n";
        assert!(find_header_end(data).is_none());
    }

    #[test]
    fn parse_status_line_ok() {
        let (code, reason) = parse_status_line("HTTP/1.1 200 OK").unwrap();
        assert_eq!(code, 200);
        assert_eq!(reason, "OK");
    }

    #[test]
    fn parse_status_line_not_found() {
        let (code, reason) = parse_status_line("HTTP/1.1 404 Not Found").unwrap();
        assert_eq!(code, 404);
        assert_eq!(reason, "Not Found");
    }

    #[test]
    fn parse_status_line_invalid() {
        assert!(parse_status_line("NOT HTTP").is_err());
    }
}
