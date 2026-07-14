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

use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use http::HeaderValue;

use crate::body::{BoxBytesStream, RequestBody};
use crate::error::{Error, Result};
use crate::headers::Headers;

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
    /// # Panics
    ///
    /// Panics if the system random number generator fails.
    #[must_use]
    pub fn random() -> Self {
        let mut bytes = [0u8; 50];
        getrandom::getrandom(&mut bytes).expect("getrandom failed");
        for byte in &mut bytes {
            *byte = BOUNDARY_CHARS[usize::from(*byte % 64)];
        }

        // All chars are from BOUNDARY_CHARS which are valid ASCII/UTF-8.
        Self(String::from_utf8(bytes.to_vec()).expect("boundary chars are valid UTF-8"))
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
            Self::Bytes(b) => f.debug_tuple("Bytes").field(b).finish(),
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
    #[must_use]
    pub fn header(
        mut self,
        name: impl TryInto<http::HeaderName>,
        value: impl TryInto<HeaderValue>,
    ) -> Self {
        if let Ok(n) = name.try_into() {
            if let Ok(v) = value.try_into() {
                let hv: HeaderValue = v;
                let _ = self.headers.insert(n.as_str(), hv.to_str().unwrap_or(""));
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
        self.ensure_no_boundary_collision();
        if self.is_replayable() {
            let boundary_str = self.boundary.0.clone();
            let mut buf = BytesMut::new();
            for part in &self.parts {
                let header = format_part_header(&boundary_str, part);
                buf.extend_from_slice(header.as_bytes());
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
            #[allow(clippy::cast_possible_truncation)]
            RequestBody::Stream {
                stream: Box::pin(self.encoder()),
                length: len.map(|l| l as usize),
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
        self.ensure_no_boundary_collision();
        MultipartEncoder::new(self)
    }

    /// Check that the boundary does not appear within any buffered part body.
    ///
    /// If a collision is detected, the boundary is regenerated (up to 10
    /// attempts). Streamed parts are not checked because their content is
    /// not yet available; this is documented as a known limitation.
    fn ensure_no_boundary_collision(&mut self) {
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
                return;
            }
            self.boundary = Boundary::random();
        }
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
        if ch == '"' || ch == '\r' || ch == '\n' {
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
        if ch == '"' || ch == '\r' || ch == '\n' {
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

fn format_part_header(boundary: &str, part: &Part) -> String {
    let mut header = String::new();
    header.push_str("--");
    header.push_str(boundary);
    header.push_str("\r\nContent-Disposition: form-data; name=\"");
    header.push_str(&part.name);
    header.push('"');
    if let Some(ref filename) = part.filename {
        header.push_str("; filename=\"");
        header.push_str(filename);
        header.push('"');
    }
    header.push_str("\r\n");
    if let Some(ref ct) = part.content_type {
        header.push_str("Content-Type: ");
        header.push_str(ct.to_str().unwrap_or("application/octet-stream"));
        header.push_str("\r\n");
    }
    for (name, value) in part.headers.iter() {
        header.push_str(name.as_str());
        header.push_str(": ");
        header.push_str(value.to_str().unwrap_or(""));
        header.push_str("\r\n");
    }
    header.push_str("\r\n");
    header
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
            Bytes::from(format_part_header(&boundary, &parts[0]).into_bytes())
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
        }
    }
}

impl Stream for MultipartEncoder {
    type Item = Result<Bytes>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

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
                        this.header = Bytes::from(
                            format_part_header(&this.boundary, &this.parts[this.part_idx])
                                .into_bytes(),
                        );
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
            .bytes("f", "bad\"name", "text/plain", Bytes::new())
            .is_err());
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
}
