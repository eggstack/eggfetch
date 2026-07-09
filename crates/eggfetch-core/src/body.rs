//! Body model for requests and responses.

use bytes::Bytes;

use crate::error::{Error, Result};

/// Request body.
#[derive(Debug, Clone, Default)]
pub enum RequestBody {
    /// Empty body.
    #[default]
    Empty,
    /// Byte body.
    Bytes(Bytes),
}

impl RequestBody {
    /// Returns `true` if the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns the length of the body.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Bytes(b) => b.len(),
        }
    }

    /// Convert into a hyper-compatible body.
    pub(crate) fn into_hyper_body(self) -> http_body_util::Full<Bytes> {
        match self {
            Self::Empty => http_body_util::Full::new(Bytes::new()),
            Self::Bytes(b) => http_body_util::Full::new(b),
        }
    }
}

impl From<Bytes> for RequestBody {
    fn from(b: Bytes) -> Self {
        Self::Bytes(b)
    }
}

impl From<Vec<u8>> for RequestBody {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(Bytes::from(v))
    }
}

impl From<&[u8]> for RequestBody {
    fn from(s: &[u8]) -> Self {
        Self::Bytes(Bytes::copy_from_slice(s))
    }
}

impl From<String> for RequestBody {
    fn from(s: String) -> Self {
        Self::Bytes(Bytes::from(s))
    }
}

impl From<&str> for RequestBody {
    fn from(s: &str) -> Self {
        Self::Bytes(Bytes::from(s.to_owned()))
    }
}

/// Response body handle.
///
/// In Milestone B this buffers the entire body. Streaming support
/// arrives in Milestone E.
#[derive(Debug)]
pub struct ResponseBody {
    bytes: Bytes,
}

impl ResponseBody {
    /// Create a response body from bytes.
    pub(crate) fn from_bytes(b: Bytes) -> Self {
        Self { bytes: b }
    }

    /// Returns `true` if the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the length of the body.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Consume the body and return the bytes.
    ///
    /// # Errors
    ///
    /// This method currently never fails, but returns `Result` for API
    /// consistency with [`ResponseBody::text`].
    #[allow(clippy::unnecessary_wraps)]
    pub fn bytes(self) -> Result<Bytes> {
        Ok(self.bytes)
    }

    /// Consume the body and return it as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid UTF-8.
    pub fn text(self) -> Result<String> {
        let bytes = self.bytes();
        String::from_utf8(bytes?.to_vec()).map_err(|e| Error::Body(e.to_string()))
    }
}
