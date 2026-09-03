//! Multipart form-data request body support.
//!
//! Provides a streaming multipart encoder that produces `multipart/form-data`
//! request bodies without buffering the entire payload in memory.
//!
//! # Example
//!
//! ```no_run
//! use eggfetch_core::multipart::Multipart;
//!
//! # async fn example() -> eggfetch_core::Result<()> {
//! let body = Multipart::new()
//!     .text("field", "value")?
//!     .bytes("file", "test.txt", "text/plain", bytes::Bytes::from("hello"))?
//!     .into_body();
//! # Ok(())
//! # }
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use http::HeaderValue;

use crate::body::{BoxBytesStream, RequestBody};
use crate::error::{Error, Result};
use crate::headers::Headers;

/// Counter mixed into fallback boundary seeds when the system RNG is
/// unavailable.
static FALLBACK_SEQUENCE: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Boundary
// ---------------------------------------------------------------------------

const BOUNDARY_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

/// A validated multipart boundary string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary(String);

impl Boundary {
    /// Generate a random boundary.
    ///
    /// Falls back to a time- and counter-seeded value if the system RNG
    /// fails, so multipart construction never aborts the process.
    // The fallback seed mixes a u128 nanosecond timestamp into a u64 state
    // and truncates hashed bytes to u8; both truncations are intentional
    // for this non-cryptographic degradation path.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn random() -> Self {
        let mut bytes = [0u8; 50];
        if getrandom::getrandom(&mut bytes).is_err() {
            // Degraded mode: derive pseudo-random bytes from the current
            // time plus a process-wide counter. Uniqueness within this
            // process is preserved, which is what boundary safety needs.
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64);
            let mut state = nanos
                ^ FALLBACK_SEQUENCE
                    .fetch_add(1, Ordering::Relaxed)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15);
            for byte in &mut bytes {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                *byte = (state >> 33) as u8;
            }
        }
        for byte in &mut bytes {
            *byte = BOUNDARY_CHARS[usize::from(*byte % 64)];
        }

        // BOUNDARY_CHARS are ASCII, so mapping through `char::from` is
        // lossless and cannot fail.
        Self(bytes.iter().map(|&byte| char::from(byte)).collect())
    }

    /// Create a boundary from a user-provided string.
    ///
    /// # Errors
    ///
    /// Returns an error if the boundary contains invalid characters or is
    /// too long.
    pub fn try_new(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::RequestBuild("boundary must not be empty".into()));
        }
        if s.len() > 69 {
            return Err(Error::RequestBuild(
                "boundary must not exceed 69 characters".into(),
            ));
        }
        for ch in s.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
                return Err(Error::RequestBuild(format!(
                    "boundary contains invalid character: {ch}"
                )));
            }
        }
        Ok(Self(s.to_owned()))
    }

    /// Returns the boundary string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Boundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// PartBody
// ---------------------------------------------------------------------------

/// Body of a multipart part.
pub enum PartBody {
    /// Fixed-size byte body.
    Bytes(Bytes),
    /// Streaming body with optional known length in bytes.
    Stream {
        /// The body stream.
        stream: BoxBytesStream,
        /// Known length in bytes, if available.
        length: Option<u64>,
    },
}

impl std::fmt::Debug for PartBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Render only the length: part bytes are file contents, which
            // must not appear in logs or panic messages.
            Self::Bytes(b) => f.debug_tuple("Bytes").field(&b.len()).finish(),
            Self::Stream { length, .. } => f
                .debug_struct("Stream")
                .field("length", length)
                .finish_non_exhaustive(),
        }
    }
}

impl PartBody {
    /// Returns the known length of this body, if available.
    #[must_use]
    pub fn len(&self) -> Option<u64> {
        match self {
            Self::Bytes(b) => Some(b.len() as u64),
            Self::Stream { length, .. } => *length,
        }
    }

    /// Returns `true` if the body has zero known length.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }

    /// Returns `true` if this body is a stream (non-replayable).
    #[must_use]
    pub fn is_stream(&self) -> bool {
        matches!(self, Self::Stream { .. })
    }
}

impl Default for PartBody {
    fn default() -> Self {
        Self::Bytes(Bytes::new())
    }
}

// ---------------------------------------------------------------------------
// Part
// ---------------------------------------------------------------------------

/// A single part in a multipart body.
pub struct Part {
    name: String,
    filename: Option<String>,
    content_type: Option<HeaderValue>,
    headers: Headers,
    body: PartBody,
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Part")
            .field("name", &self.name)
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("headers", &self.headers)
            .field("body", &self.body)
            .finish()
    }
}

impl Part {
    /// Create a new part with the given name and body.
    #[must_use]
    pub fn new(name: impl Into<String>, body: PartBody) -> Self {
        Self {
            name: name.into(),
            filename: None,
            content_type: None,
            headers: Headers::new(),
            body,
        }
    }

    /// Set the filename for this part.
    #[must_use]
    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Set the content type for this part.
    #[must_use]
    pub fn content_type(mut self, ct: HeaderValue) -> Self {
        self.content_type = Some(ct);
        self
    }

    /// Add a custom header to this part.
    ///
    /// # Errors
    ///
    /// Returns an error if the header name or value is invalid.
    pub fn try_header(
        mut self,
        name: impl TryInto<http::HeaderName>,
        value: impl TryInto<HeaderValue>,
    ) -> Result<Self> {
        let n = name
            .try_into()
            .map_err(|_| Error::InvalidHeaderName("invalid multipart part header name".into()))?;
        let v = value
            .try_into()
            .map_err(|_| Error::InvalidHeaderValue("invalid multipart part header value".into()))?;
        let hv: HeaderValue = v;
        // Keep the raw bytes: `HeaderValue` admits obs-text
        // (0x80..=0xFF), so converting with `to_str()` would
        // silently blank legitimate non-ASCII values.
        self.headers.insert_raw(n, hv);
        Ok(self)
    }

    /// Add a custom header to this part.
    ///
    /// Invalid header names or values are silently ignored. Prefer
    /// [`Self::try_header`] when the caller needs to surface header
    /// validation errors.
    #[must_use]
    pub fn header(
        mut self,
        name: impl TryInto<http::HeaderName>,
        value: impl TryInto<HeaderValue>,
    ) -> Self {
        if let Ok(n) = name.try_into() {
            if let Ok(v) = value.try_into() {
                let hv: HeaderValue = v;
                // Keep the raw bytes: `HeaderValue` admits obs-text
                // (0x80..=0xFF), so converting with `to_str()` would
                // silently blank legitimate non-ASCII values.
                self.headers.insert_raw(n, hv);
            }
        }
        self
    }

    /// Returns the field name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the filename, if set.
    #[must_use]
    pub fn filename_opt(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Returns the content type, if set.
    #[must_use]
    pub fn content_type_opt(&self) -> Option<&HeaderValue> {
        self.content_type.as_ref()
    }

    /// Returns the body.
    #[must_use]
    pub fn body(&self) -> &PartBody {
        &self.body
    }

    /// Consume and return the body.
    #[must_use]
    pub fn into_body(self) -> PartBody {
        self.body
    }
}

// ---------------------------------------------------------------------------
// Multipart
// ---------------------------------------------------------------------------

/// A multipart/form-data request body.
///
/// Builds the multipart body and provides encoding via [`into_body`] or
/// [`encoder`].
///
/// [`into_body`]: Multipart::into_body
/// [`encoder`]: Multipart::encoder
pub struct Multipart {
    boundary: Boundary,
    parts: Vec<Part>,
}

impl std::fmt::Debug for Multipart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Multipart")
            .field("boundary", &self.boundary)
            .field("parts", &self.parts)
            .finish()
    }
}

impl Default for Multipart {
    fn default() -> Self {
        Self::new()
    }
}

impl Multipart {
    /// Create a new multipart with a random boundary.
    #[must_use]
    pub fn new() -> Self {
        Self {
            boundary: Boundary::random(),
            parts: Vec::new(),
        }
    }

    /// Create a new multipart with a custom boundary.
    #[must_use]
    pub fn with_boundary(boundary: Boundary) -> Self {
        Self {
            boundary,
            parts: Vec::new(),
        }
    }

    /// Returns the boundary.
    #[must_use]
    pub fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    /// Returns the parts.
    #[must_use]
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Add a text field part.
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains invalid characters.
    pub fn text(mut self, name: &str, value: &str) -> Result<Self> {
        validate_field_name(name)?;
        self.parts.push(Part {
            name: name.to_owned(),
            filename: None,
            content_type: None,
            headers: Headers::new(),
            body: PartBody::Bytes(Bytes::copy_from_slice(value.as_bytes())),
        });
        Ok(self)
    }

    /// Add a file part from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the name or filename contains invalid characters.
    pub fn bytes(
        mut self,
        name: &str,
        filename: &str,
        content_type: &str,
        data: Bytes,
    ) -> Result<Self> {
        validate_field_name(name)?;
        validate_filename(filename)?;
        let ct = HeaderValue::from_str(content_type)
            .map_err(|e| Error::RequestBuild(format!("invalid content type: {e}")))?;
        self.parts.push(Part {
            name: name.to_owned(),
            filename: Some(filename.to_owned()),
            content_type: Some(ct),
            headers: Headers::new(),
            body: PartBody::Bytes(data),
        });
        Ok(self)
    }

    /// Add a file part from a stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the name or filename contains invalid characters.
    pub fn stream(
        mut self,
        name: &str,
        filename: &str,
        content_type: &str,
        stream: BoxBytesStream,
        length: Option<u64>,
    ) -> Result<Self> {
        validate_field_name(name)?;
        validate_filename(filename)?;
        let ct = HeaderValue::from_str(content_type)
            .map_err(|e| Error::RequestBuild(format!("invalid content type: {e}")))?;
        self.parts.push(Part {
            name: name.to_owned(),
            filename: Some(filename.to_owned()),
            content_type: Some(ct),
            headers: Headers::new(),
            body: PartBody::Stream { stream, length },
        });
        Ok(self)
    }

    /// Add a pre-built part.
    ///
    /// # Errors
    ///
    /// Returns an error if the part's name contains invalid characters.
    pub fn part(mut self, part: Part) -> Result<Self> {
        validate_field_name(&part.name)?;
        self.parts.push(part);
        Ok(self)
    }

    /// Returns `true` if all parts are replayable (all bytes, no streams).
    #[must_use]
    pub fn is_replayable(&self) -> bool {
        self.parts.iter().all(|p| !p.body.is_stream())
    }

    /// Calculate the total Content-Length, if all parts have known lengths.
    ///
    /// Uses checked arithmetic to prevent overflow.
    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        let mut total: u64 = 0;
        let boundary_str = self.boundary.as_str();
        for part in &self.parts {
            let header = format_part_header(boundary_str, part);
            total = total.checked_add(header.len() as u64)?;
            let body_len = part.body.len()?;
            total = total.checked_add(body_len)?;
            total = total.checked_add(2)?; // trailing \r\n
        }
        // Final boundary: --boundary--\r\n
        total = total.checked_add((boundary_str.len() + 6) as u64)?;
        Some(total)
    }

    /// Convert into a [`RequestBody`].
    ///
    /// If all parts are byte-backed, returns a buffered `RequestBody::Bytes`.
    /// Otherwise, returns a streaming `RequestBody::Stream`.
    ///
    /// Before encoding, checks that the boundary does not accidentally appear
    /// within any buffered part body. If a collision is detected, the boundary
    /// is regenerated. Streamed parts cannot be checked and are documented
    /// accordingly.
    #[must_use]
    pub fn into_body(mut self) -> RequestBody {
        if !self.ensure_no_boundary_collision() {
            return RequestBody::Stream {
                stream: Box::pin(MultipartEncoder::error(Error::RequestBuild(
                    "failed to generate a collision-free multipart boundary".into(),
                ))),
                length: None,
            };
        }
        if self.is_replayable() {
            let boundary_str = self.boundary.0.clone();
            let mut buf = BytesMut::new();
            for part in &self.parts {
                let header = format_part_header(&boundary_str, part);
                buf.extend_from_slice(&header);
                if let PartBody::Bytes(ref data) = part.body {
                    buf.extend_from_slice(data);
                }
                buf.extend_from_slice(b"\r\n");
            }
            let final_boundary = format!("--{boundary_str}--\r\n");
            buf.extend_from_slice(final_boundary.as_bytes());
            RequestBody::Bytes(buf.freeze())
        } else {
            let len = self.content_length();
            RequestBody::Stream {
                stream: Box::pin(MultipartEncoder::new(self)),
                // A length beyond `usize::MAX` cannot be represented as a
                // framing hint; report an unknown length rather than a
                // truncated (wrong) one.
                length: len.and_then(|l| usize::try_from(l).ok()),
            }
        }
    }

    /// Create a streaming encoder for this multipart body.
    ///
    /// Before encoding, checks that the boundary does not accidentally appear
    /// within any buffered part body. If a collision is detected, the boundary
    /// is regenerated. Streamed parts cannot be checked.
    #[must_use]
    pub fn encoder(mut self) -> MultipartEncoder {
        if !self.ensure_no_boundary_collision() {
            return MultipartEncoder::error(Error::RequestBuild(
                "failed to generate a collision-free multipart boundary".into(),
            ));
        }
        MultipartEncoder::new(self)
    }

    /// Check that the boundary does not appear within any buffered part body.
    ///
    /// If a collision is detected, the boundary is regenerated (up to 10
    /// attempts).
    ///
    /// Streamed part content is not checked because it is not yet
    /// available; this remains a documented limitation.
    ///
    /// Part names and filenames are deliberately not scanned: they are
    /// always emitted inside quoted `Content-Disposition` parameter
    /// values, and quote/CR/LF characters are rejected during
    /// validation, so they can never form the `--boundary` + CRLF
    /// delimiter sequence that would confuse a parser.
    fn ensure_no_boundary_collision(&mut self) -> bool {
        const MAX_ATTEMPTS: usize = 10;
        for _ in 0..MAX_ATTEMPTS {
            let boundary_bytes = self.boundary.0.as_bytes();
            let blen = boundary_bytes.len();
            let collision = self.parts.iter().any(|part| {
                if let PartBody::Bytes(ref data) = part.body {
                    data.windows(blen).any(|window| window == boundary_bytes)
                } else {
                    false
                }
            });
            if !collision {
                return true;
            }
            self.boundary = Boundary::random();
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_field_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::RequestBuild(
            "multipart field name must not be empty".into(),
        ));
    }
    for ch in name.chars() {
        // `"` would terminate the quoted-string; C0 controls (including
        // CR/LF/NUL) are not valid qdtext per RFC 9110 §5.6.4 and break
        // strict Content-Disposition parsers.
        if ch == '"' || ch.is_control() {
            return Err(Error::RequestBuild(format!(
                "multipart field name contains invalid character: {ch:?}"
            )));
        }
    }
    Ok(())
}

fn validate_filename(filename: &str) -> Result<()> {
    if filename.is_empty() {
        return Err(Error::RequestBuild(
            "multipart filename must not be empty".into(),
        ));
    }
    for ch in filename.chars() {
        // Same quoted-string rules as `validate_field_name`.
        if ch == '"' || ch.is_control() {
            return Err(Error::RequestBuild(format!(
                "multipart filename contains invalid character: {ch:?}"
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Header generation
// ---------------------------------------------------------------------------

/// Escape a value for inclusion inside a quoted-string per RFC 9110
/// §5.6.2.2 (`\` and `"` are quoted-pair escapes).
fn quote_string_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn format_part_header(boundary: &str, part: &Part) -> Vec<u8> {
    let mut header = String::new();
    header.push_str("--");
    header.push_str(boundary);
    header.push_str("\r\nContent-Disposition: form-data; name=\"");
    header.push_str(&quote_string_escape(&part.name));
    header.push('"');
    if let Some(ref filename) = part.filename {
        header.push_str("; filename=\"");
        header.push_str(&quote_string_escape(filename));
        header.push('"');
    }
    header.push_str("\r\n");
    let mut out = header.into_bytes();
    if let Some(ref ct) = part.content_type {
        out.extend_from_slice(b"Content-Type: ");
        out.extend_from_slice(ct.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    for (name, value) in part.headers.iter() {
        out.extend_from_slice(name.as_str().as_bytes());
        out.extend_from_slice(b": ");
        // Values were stored losslessly from the original bytes and may
        // carry valid HTTP/1.1 obs-text; emit them raw rather than
        // replacing them with an empty string.
        out.extend_from_slice(value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out
}

// ---------------------------------------------------------------------------
// Streaming encoder
// ---------------------------------------------------------------------------

/// Encoder state.
#[derive(Debug)]
enum State {
    /// Emitting the header for the current part.
    PartHeader,
    /// Emitting the body for the current part.
    PartBody,
    /// Emitting the trailing CRLF after a body.
    TrailingCrlf,
    /// Emitting the final boundary terminator.
    FinalBoundary,
    /// Done.
    Done,
}

/// A streaming multipart/form-data encoder.
///
/// Implements [`Stream`] to produce the encoded body incrementally without
/// buffering the entire payload.
pub struct MultipartEncoder {
    boundary: String,
    parts: Vec<Part>,
    header: Bytes,
    header_pos: usize,
    body: Option<PartBody>,
    state: State,
    part_idx: usize,
    error: Option<Error>,
}

impl std::fmt::Debug for MultipartEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultipartEncoder")
            .field("boundary", &self.boundary)
            .field("parts_count", &self.parts.len())
            .field("header", &self.header)
            .field("header_pos", &self.header_pos)
            .field("body", &self.body.is_some())
            .field("state", &self.state)
            .field("part_idx", &self.part_idx)
            .field("has_error", &self.error.is_some())
            .finish()
    }
}

impl MultipartEncoder {
    fn new(multipart: Multipart) -> Self {
        let boundary = multipart.boundary.0;
        let parts = multipart.parts;

        let is_empty = parts.is_empty();
        let header = if is_empty {
            Bytes::new()
        } else {
            Bytes::from(format_part_header(&boundary, &parts[0]))
        };

        Self {
            boundary,
            parts,
            header,
            header_pos: 0,
            body: None,
            state: if is_empty {
                State::FinalBoundary
            } else {
                State::PartHeader
            },
            part_idx: 0,
            error: None,
        }
    }

    fn error(error: Error) -> Self {
        Self {
            boundary: String::new(),
            parts: Vec::new(),
            header: Bytes::new(),
            header_pos: 0,
            body: None,
            state: State::Done,
            part_idx: 0,
            error: Some(error),
        }
    }
}

impl Stream for MultipartEncoder {
    type Item = Result<Bytes>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(error) = this.error.take() {
            return Poll::Ready(Some(Err(error)));
        }

        loop {
            match this.state {
                State::PartHeader => {
                    if this.header_pos < this.header.len() {
                        let remaining = Bytes::copy_from_slice(&this.header[this.header_pos..]);
                        this.header_pos = this.header.len();
                        return Poll::Ready(Some(Ok(remaining)));
                    }
                    this.body = Some(std::mem::take(&mut this.parts[this.part_idx].body));
                    this.state = State::PartBody;
                }
                State::PartBody => {
                    if let Some(ref mut body) = this.body {
                        match body {
                            PartBody::Bytes(b) => {
                                if !b.is_empty() {
                                    let data = std::mem::take(b);
                                    this.body = None;
                                    this.state = State::TrailingCrlf;
                                    return Poll::Ready(Some(Ok(data)));
                                }
                                this.body = None;
                                this.state = State::TrailingCrlf;
                            }
                            PartBody::Stream { stream, .. } => {
                                match stream.as_mut().poll_next(cx) {
                                    Poll::Ready(Some(Ok(chunk))) => {
                                        return Poll::Ready(Some(Ok(chunk)));
                                    }
                                    Poll::Ready(Some(Err(e))) => {
                                        this.body = None;
                                        this.state = State::Done;
                                        return Poll::Ready(Some(Err(e)));
                                    }
                                    Poll::Ready(None) => {
                                        this.body = None;
                                        this.state = State::TrailingCrlf;
                                    }
                                    Poll::Pending => return Poll::Pending,
                                }
                            }
                        }
                    } else {
                        this.state = State::TrailingCrlf;
                    }
                }
                State::TrailingCrlf => {
                    this.body = None;
                    if this.part_idx + 1 < this.parts.len() {
                        this.part_idx += 1;
                        this.header = Bytes::from(format_part_header(
                            &this.boundary,
                            &this.parts[this.part_idx],
                        ));
                        this.header_pos = 0;
                        this.state = State::PartHeader;
                    } else {
                        this.state = State::FinalBoundary;
                    }
                    return Poll::Ready(Some(Ok(Bytes::from_static(b"\r\n"))));
                }
                State::FinalBoundary => {
                    let fb = format!("--{}--\r\n", this.boundary);
                    this.state = State::Done;
                    return Poll::Ready(Some(Ok(Bytes::from(fb))));
                }
                State::Done => return Poll::Ready(None),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn boundary_random_is_valid() {
        let b = Boundary::random();
        assert_eq!(b.as_str().len(), 50);
        for ch in b.as_str().chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '_' || ch == '-',
                "invalid char in boundary: {ch}"
            );
        }
    }

    #[test]
    fn boundary_try_new_valid() {
        let b = Boundary::try_new("my-boundary_123").unwrap();
        assert_eq!(b.as_str(), "my-boundary_123");
    }

    #[test]
    fn boundary_try_new_empty_rejected() {
        assert!(Boundary::try_new("").is_err());
    }

    #[test]
    fn boundary_try_new_too_long_rejected() {
        let long = "a".repeat(70);
        assert!(Boundary::try_new(&long).is_err());
    }

    #[test]
    fn boundary_try_new_invalid_chars_rejected() {
        assert!(Boundary::try_new("has space").is_err());
        assert!(Boundary::try_new("has\"quote").is_err());
        assert!(Boundary::try_new("has/dot").is_err());
    }

    #[test]
    fn boundary_try_new_max_length_accepted() {
        let max = "a".repeat(69);
        assert!(Boundary::try_new(&max).is_ok());
    }

    #[test]
    fn boundary_substring_in_part_name_does_not_trigger_regeneration() {
        // Names live inside quoted Content-Disposition values and cannot
        // contain CR/LF, so a substring match with the boundary is not a
        // framing hazard and the explicit boundary must be preserved.
        let seed = Boundary::try_new("b").unwrap();
        let mp = Multipart::with_boundary(seed)
            .text("name-with-b-inside", "v")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let text = std::str::from_utf8(&data).unwrap();
                // The explicit boundary is preserved as the framing
                // delimiter; it also appears inside the quoted name,
                // which is harmless.
                assert!(text.starts_with("--b\r\n"));
                assert!(text.contains("\r\n--b--\r\n"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn single_text_field() {
        let mp = Multipart::with_boundary(Boundary::try_new("test-boundary").unwrap())
            .text("field", "value")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let expected = "\
                    --test-boundary\r\n\
                    Content-Disposition: form-data; name=\"field\"\r\n\
                    \r\n\
                    value\r\n\
                    --test-boundary--\r\n\
                ";
                assert_eq!(data.as_ref(), expected.as_bytes());
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn multiple_text_fields() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("a", "1")
            .unwrap()
            .text("b", "2")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("name=\"a\""));
                assert!(s.contains("1\r\n--b\r\n"));
                assert!(s.contains("name=\"b\""));
                assert!(s.contains("2\r\n--b--\r\n"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn bytes_file_part() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .bytes("file", "test.txt", "text/plain", Bytes::from("hello"))
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("filename=\"test.txt\""));
                assert!(s.contains("Content-Type: text/plain"));
                assert!(s.contains("hello"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn repeated_field_names() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("tag", "a")
            .unwrap()
            .text("tag", "b")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert_eq!(s.matches("name=\"tag\"").count(), 2);
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn known_total_length() {
        let mp = Multipart::with_boundary(Boundary::try_new("short").unwrap())
            .text("field", "value")
            .unwrap();
        let len = mp.content_length().unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                assert_eq!(len, data.len() as u64);
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn unknown_total_length_stream() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .stream(
                "file",
                "big.bin",
                "application/octet-stream",
                Box::pin(stream),
                None,
            )
            .unwrap();
        assert!(!mp.is_replayable());
        assert!(mp.content_length().is_none());
    }

    #[test]
    fn invalid_field_name_rejected() {
        assert!(Multipart::new().text("", "value").is_err());
        assert!(Multipart::new().text("bad\nname", "value").is_err());
        assert!(Multipart::new().text("bad\rname", "value").is_err());
        assert!(Multipart::new().text("bad\"name", "value").is_err());
    }

    #[test]
    fn invalid_filename_rejected() {
        assert!(Multipart::new()
            .bytes("f", "", "text/plain", Bytes::new())
            .is_err());
        assert!(Multipart::new()
            .bytes("f", "bad\n", "text/plain", Bytes::new())
            .is_err());
        assert!(Multipart::new()
            .bytes("f", "bad\r", "text/plain", Bytes::new())
            .is_err());
        assert!(Multipart::new()
            .bytes("f", "bad\"name", "text/plain", Bytes::new())
            .is_err());
    }

    #[test]
    fn filename_none_omits_filename_attribute() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("field", "value")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("name=\"field\""), "should contain field name");
                assert!(
                    !s.contains("filename="),
                    "should not contain filename= when none was set"
                );
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn backslash_escaped_in_quoted_strings() {
        // `\` is a quoted-pair escape inside `filename="..."`; without
        // escaping, strict parsers decode `path\to\file.txt` as
        // `pathtofile.txt`.
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .bytes("f", r"path\to\file.txt", "text/plain", Bytes::new())
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(
                    s.contains(r#"filename="path\\to\\file.txt""#),
                    "backslashes must be escaped in the quoted filename, got: {s}"
                );
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn bytes_input_various_sizes() {
        // Zero-byte part.
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .bytes(
                "empty",
                "empty.bin",
                "application/octet-stream",
                Bytes::new(),
            )
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("filename=\"empty.bin\""));
            }
            _ => panic!("expected bytes body"),
        }

        // Large part (10 KB).
        let large_data = Bytes::from(vec![0xAB; 10_240]);
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .bytes("big", "big.bin", "application/octet-stream", large_data)
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                assert!(data.len() > 10_240, "body should include the 10 KB payload");
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[tokio::test]
    async fn stream_drop_releases_resources() {
        use futures_util::stream;

        // Create a stream that tracks whether it was dropped.
        let dropped = Arc::new(AtomicBool::new(false));
        let d = dropped.clone();
        let tracked = stream::once(async move {
            let _guard = DropGuard(d);
            Ok::<Bytes, crate::Error>(Bytes::from("data"))
        });

        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .stream(
                "file",
                "test.bin",
                "application/octet-stream",
                Box::pin(tracked),
                None,
            )
            .unwrap();

        let mut encoder = mp.encoder();
        // Consume all output.
        while let Some(chunk) = encoder.next().await {
            drop(chunk);
        }

        // After consuming, the stream should have been dropped.
        assert!(
            dropped.load(Ordering::SeqCst),
            "stream should be dropped after encoder completes"
        );
    }

    struct DropGuard(Arc<AtomicBool>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn custom_per_part_headers() {
        let part = Part::new("field", PartBody::Bytes(Bytes::from("data")))
            .header("X-Custom", "value")
            .content_type(HeaderValue::from_static("text/plain"));
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .part(part)
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("x-custom: value"));
                assert!(s.contains("Content-Type: text/plain"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn custom_part_header_preserves_non_ascii_value() {
        // `HeaderValue` admits obs-text, so a non-ASCII value must be
        // emitted verbatim rather than silently blanked by a
        // failed `to_str()` conversion.
        let part =
            Part::new("field", PartBody::Bytes(Bytes::from("data"))).header("X-Note", "café");
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .part(part)
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("x-note: café"), "got: {s}");
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn custom_part_header_obs_text_value_is_never_blanked() {
        // Raw obs-text bytes must be retained by the part header store; a
        // failed UTF-8 conversion must never collapse them to "".
        let part = Part::new("field", PartBody::Bytes(Bytes::from("data")))
            .header("X-Note", HeaderValue::from_bytes(&[0x80]).unwrap());
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .part(part)
            .unwrap();
        match mp.into_body() {
            RequestBody::Bytes(data) => {
                assert!(data
                    .windows(b"x-note: \x80\r\n".len())
                    .any(|window| { window == b"x-note: \x80\r\n" }));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn content_type_obs_text_value_is_preserved() {
        let part = Part::new("field", PartBody::Bytes(Bytes::from("data")))
            .content_type(HeaderValue::from_bytes(b"text/plain; charset=\xFF").unwrap());
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .part(part)
            .unwrap();
        match mp.into_body() {
            RequestBody::Bytes(data) => assert!(data
                .windows(b"\xFF\r\n".len())
                .any(|window| { window == b"\xFF\r\n" })),
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn part_field_name_rejects_control_characters() {
        // C0 controls (beyond the previously rejected CR/LF) would be
        // emitted raw into the quoted-string of Content-Disposition,
        // which strict RFC 9110 parsers reject.
        assert!(Multipart::new().text("nul\x00field", "v").is_err());
        assert!(Multipart::new().text("tab\tfield", "v").is_err());
        assert!(Multipart::new().text("del\x7ffield", "v").is_err());
    }

    #[test]
    fn multipart_filename_rejects_control_characters() {
        let result = Multipart::new().bytes(
            "f",
            "bad\x1bname",
            "application/octet-stream",
            Bytes::from_static(b"d"),
        );
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid character"));
    }

    #[tokio::test]
    async fn streaming_encoder_produces_correct_output() {
        let mp = Multipart::with_boundary(Boundary::try_new("test").unwrap())
            .text("field", "hello")
            .unwrap();
        let mut encoder = mp.encoder();
        let mut output = BytesMut::new();
        while let Some(chunk) = encoder.next().await {
            output.extend_from_slice(&chunk.unwrap());
        }
        let expected = "\
            --test\r\n\
            Content-Disposition: form-data; name=\"field\"\r\n\
            \r\n\
            hello\r\n\
            --test--\r\n\
        ";
        assert_eq!(output.as_ref(), expected.as_bytes());
    }

    #[tokio::test]
    async fn streaming_encoder_empty_multipart() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap());
        let mut encoder = mp.encoder();
        let mut output = BytesMut::new();
        while let Some(chunk) = encoder.next().await {
            output.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(output.as_ref(), b"--b--\r\n");
    }

    #[tokio::test]
    async fn streaming_encoder_multiple_parts() {
        let stream =
            futures_util::stream::iter(vec![Ok(Bytes::from("chunk1")), Ok(Bytes::from("chunk2"))]);
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("a", "1")
            .unwrap()
            .stream(
                "file",
                "f.bin",
                "application/octet-stream",
                Box::pin(stream),
                Some(12),
            )
            .unwrap();
        let mut encoder = mp.encoder();
        let mut output = BytesMut::new();
        while let Some(chunk) = encoder.next().await {
            output.extend_from_slice(&chunk.unwrap());
        }
        let s = String::from_utf8(output.to_vec()).unwrap();
        assert!(s.contains("name=\"a\""));
        assert!(s.contains("1\r\n--b\r\n"));
        assert!(s.contains("filename=\"f.bin\""));
        assert!(s.contains("chunk1"));
        assert!(s.contains("chunk2"));
        assert!(s.ends_with("--b--\r\n"));
    }

    #[test]
    fn is_replayable_all_bytes() {
        let mp = Multipart::new().text("a", "1").unwrap();
        assert!(mp.is_replayable());
    }

    #[test]
    fn is_replayable_with_stream() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let mp = Multipart::new()
            .stream(
                "f",
                "f.bin",
                "application/octet-stream",
                Box::pin(stream),
                None,
            )
            .unwrap();
        assert!(!mp.is_replayable());
    }

    #[test]
    fn part_body_default_is_empty() {
        let body = PartBody::default();
        assert_eq!(body.len(), Some(0));
        assert!(!body.is_stream());
    }

    #[test]
    fn unicode_field_names_and_filenames() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("名前", "日本語の値")
            .unwrap()
            .bytes(
                "файл",
                "данные.bin",
                "application/octet-stream",
                Bytes::from("data"),
            )
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("name=\"名前\""));
                assert!(s.contains("日本語の値"));
                assert!(s.contains("name=\"файл\""));
                assert!(s.contains("filename=\"данные.bin\""));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn empty_field_value() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("field", "")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let expected = "\
                    --b\r\n\
                    Content-Disposition: form-data; name=\"field\"\r\n\
                    \r\n\
                    \r\n\
                    --b--\r\n\
                ";
                assert_eq!(data.as_ref(), expected.as_bytes());
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn zero_byte_file() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .bytes(
                "file",
                "empty.txt",
                "application/octet-stream",
                Bytes::new(),
            )
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("filename=\"empty.txt\""));
                assert!(s.contains("Content-Type: application/octet-stream"));
                assert!(s.contains("name=\"file\""));
                assert!(s.ends_with("--b--\r\n"));
                let expected = "\
                    --b\r\n\
                    Content-Disposition: form-data; name=\"file\"; filename=\"empty.txt\"\r\n\
                    Content-Type: application/octet-stream\r\n\
                    \r\n\
                    \r\n\
                    --b--\r\n\
                ";
                assert_eq!(data.as_ref(), expected.as_bytes());
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn mixed_known_and_unknown_length_parts() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("field", "value")
            .unwrap()
            .stream(
                "file",
                "big.bin",
                "application/octet-stream",
                Box::pin(stream),
                None,
            )
            .unwrap();
        assert!(!mp.is_replayable());
        assert!(mp.content_length().is_none());
        let body = mp.into_body();
        match body {
            RequestBody::Stream { length, .. } => {
                assert!(length.is_none());
            }
            _ => panic!("expected stream body"),
        }
    }

    #[test]
    fn exact_terminal_boundary_format() {
        let mp = Multipart::with_boundary(Boundary::try_new("bd").unwrap())
            .text("f", "v")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.ends_with("--bd--\r\n"));
                let final_boundary_start = s.rfind("--bd--\r\n").unwrap();
                let rest = &s[final_boundary_start..];
                assert_eq!(rest, "--bd--\r\n");
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn field_name_special_valid_characters() {
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .text("field_name", "a")
            .unwrap()
            .text("field-name", "b")
            .unwrap()
            .text("field.name", "c")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("name=\"field_name\""));
                assert!(s.contains("name=\"field-name\""));
                assert!(s.contains("name=\"field.name\""));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn content_length_overflow_with_many_parts() {
        let mut mp = Multipart::with_boundary(Boundary::try_new("b").unwrap());
        for i in 0..1000 {
            mp = mp.text(&format!("field{i}"), "value").unwrap();
        }
        let result = mp.content_length();
        assert!(result.is_some());
        let len = result.unwrap();
        assert!(len > 0);
    }

    #[test]
    fn multiple_custom_headers_per_part() {
        let part = Part::new("field", PartBody::Bytes(Bytes::from("data")))
            .header("X-First", "one")
            .header("X-Second", "two")
            .header("X-Third", "three");
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .part(part)
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(s.contains("x-first: one"));
                assert!(s.contains("x-second: two"));
                assert!(s.contains("x-third: three"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[tokio::test]
    async fn streaming_encoder_empty_stream() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let mp = Multipart::with_boundary(Boundary::try_new("b").unwrap())
            .stream(
                "file",
                "empty.bin",
                "application/octet-stream",
                Box::pin(stream),
                None,
            )
            .unwrap();
        let mut encoder = mp.encoder();
        let mut output = BytesMut::new();
        while let Some(chunk) = encoder.next().await {
            output.extend_from_slice(&chunk.unwrap());
        }
        let s = String::from_utf8(output.to_vec()).unwrap();
        assert!(s.contains("filename=\"empty.bin\""));
        assert!(s.contains("name=\"file\""));
        assert!(s.ends_with("--b--\r\n"));
        let expected = "\
            --b\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"empty.bin\"\r\n\
            Content-Type: application/octet-stream\r\n\
            \r\n\
            \r\n\
            --b--\r\n\
        ";
        assert_eq!(output.as_ref(), expected.as_bytes());
    }

    #[test]
    fn boundary_collision_in_buffered_body_regenerated() {
        let boundary = Boundary::try_new("COLLISION").unwrap();
        let collision_body = b"this body contains COLLISION inside it";
        let mp = Multipart::with_boundary(boundary.clone())
            .text("field", "value")
            .unwrap()
            .bytes(
                "file",
                "test.bin",
                "application/octet-stream",
                Bytes::from_static(collision_body),
            )
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                let boundary_header = "--COLLISION\r\n";
                assert!(
                    !s.contains(boundary_header),
                    "body should not contain the original boundary as a delimiter within part data"
                );
                assert!(s.contains("--"));
                assert!(s.contains("value"));
                assert!(s.contains("this body contains COLLISION inside it"));
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn boundary_collision_in_text_field_regenerated() {
        let boundary = Boundary::try_new("BD").unwrap();
        let mp = Multipart::with_boundary(boundary)
            .text("field", "contains BD in the middle")
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(
                    s.contains("contains BD in the middle"),
                    "body should contain the original text value"
                );
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn no_collision_when_boundary_absent_from_bodies() {
        let boundary = Boundary::try_new("NOCOLLISION").unwrap();
        let original_boundary = boundary.as_str().to_owned();
        let mp = Multipart::with_boundary(boundary)
            .text("field", "safe value")
            .unwrap()
            .bytes(
                "file",
                "test.bin",
                "text/plain",
                Bytes::from("no collision here"),
            )
            .unwrap();
        let body = mp.into_body();
        match body {
            RequestBody::Bytes(data) => {
                let s = String::from_utf8(data.to_vec()).unwrap();
                assert!(
                    s.contains(&format!("--{original_boundary}\r\n")),
                    "boundary should be unchanged when no collision exists"
                );
            }
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn part_body_bytes_debug_redacts_contents() {
        let part = PartBody::Bytes(Bytes::from("file-secret-contents"));
        let debug = format!("{part:?}");
        assert!(
            !debug.contains("file-secret-contents"),
            "PartBody Debug must not leak byte contents: {debug}"
        );
        assert!(
            debug.contains("Bytes(20)"),
            "PartBody Debug reports the length only: {debug}"
        );
    }
}
