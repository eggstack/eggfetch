//! Header container.

use http::header::{HeaderMap, HeaderName, HeaderValue};

use crate::error::{Error, Result};

/// Case-insensitive HTTP header container.
#[derive(Debug, Default, Clone)]
pub struct Headers {
    inner: HeaderMap,
}

impl Headers {
    /// Create an empty header container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a header, replacing any existing value with the same name.
    ///
    /// # Errors
    ///
    /// Returns an error if the name or value is not valid.
    pub fn insert(&mut self, name: &str, value: &str) -> Result<()> {
        validate_header_name(name)?;
        validate_header_value(value)?;
        let name =
            HeaderName::try_from(name).map_err(|e| Error::InvalidHeaderName(e.to_string()))?;
        let value =
            HeaderValue::try_from(value).map_err(|e| Error::InvalidHeaderValue(e.to_string()))?;
        self.inner.insert(name, value);
        Ok(())
    }

    /// Append a header value, allowing multiple values for the same name.
    ///
    /// # Errors
    ///
    /// Returns an error if the name or value is not valid.
    pub fn append(&mut self, name: &str, value: &str) -> Result<()> {
        validate_header_name(name)?;
        validate_header_value(value)?;
        let name =
            HeaderName::try_from(name).map_err(|e| Error::InvalidHeaderName(e.to_string()))?;
        let value =
            HeaderValue::try_from(value).map_err(|e| Error::InvalidHeaderValue(e.to_string()))?;
        self.inner.append(name, value);
        Ok(())
    }

    /// Get a header value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&HeaderValue> {
        self.inner.get(name)
    }

    /// Get a header value as a string by name.
    #[must_use]
    pub fn get_str(
        &self,
        name: &str,
    ) -> Option<std::result::Result<&str, http::header::ToStrError>> {
        self.inner.get(name).map(|v| v.to_str())
    }

    /// Returns `true` if the headers contain a value for the given name.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.inner.contains_key(name)
    }

    /// Returns the number of headers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if there are no headers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over all header name-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&HeaderName, &HeaderValue)> {
        self.inner.iter()
    }

    /// Iterate over all header names.
    pub fn keys(&self) -> impl Iterator<Item = &HeaderName> {
        self.inner.keys()
    }

    /// Extend these headers with another set of headers.
    pub fn extend(&mut self, other: Self) {
        self.inner.extend(other.inner);
    }

    /// Consume and return the inner `HeaderMap`.
    #[must_use]
    pub fn into_inner(self) -> HeaderMap {
        self.inner
    }
}

impl From<HeaderMap> for Headers {
    fn from(inner: HeaderMap) -> Self {
        Self { inner }
    }
}

fn validate_header_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidHeaderName(
            "header name must not be empty".into(),
        ));
    }
    if name.contains('\n') || name.contains('\r') {
        return Err(Error::InvalidHeaderName(
            "header name must not contain newlines".into(),
        ));
    }
    Ok(())
}

fn validate_header_value(value: &str) -> Result<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(Error::InvalidHeaderValue(
            "header value must not contain bare newlines (CR/LF)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut h = Headers::new();
        h.insert("X-Custom", "value1").unwrap();
        assert_eq!(h.get("x-custom").unwrap().to_str().unwrap(), "value1");
    }

    #[test]
    fn append_duplicates() {
        let mut h = Headers::new();
        h.append("Set-Cookie", "a=1").unwrap();
        h.append("Set-Cookie", "b=2").unwrap();
        let values: Vec<_> = h.inner.get_all("set-cookie").iter().collect();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn contains() {
        let mut h = Headers::new();
        h.insert("Content-Type", "text/plain").unwrap();
        assert!(h.contains("content-type"));
        assert!(!h.contains("x-missing"));
    }

    #[test]
    fn empty_name_rejected() {
        let mut h = Headers::new();
        assert!(h.insert("", "value").is_err());
    }

    #[test]
    fn newline_in_name_rejected() {
        let mut h = Headers::new();
        assert!(h.insert("X-Bad\n", "value").is_err());
    }

    #[test]
    fn newline_in_value_rejected() {
        let mut h = Headers::new();
        assert!(h.insert("X-Bad", "value\r\ninjection").is_err());
    }

    #[test]
    fn from_headermap() {
        let mut hm = HeaderMap::new();
        hm.insert("X-Test", HeaderValue::from_static("hello"));
        let h = Headers::from(hm);
        assert_eq!(h.get("x-test").unwrap().to_str().unwrap(), "hello");
    }
}
