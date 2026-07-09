//! Async client entry point.

/// Async HTTP client placeholder.
///
/// The actual engine lands in Milestone B. Until then, constructing a
/// client returns a stub that can be used to anchor API shape.
#[derive(Debug, Default, Clone)]
pub struct Client {
    _private: (),
}

impl Client {
    /// Create a new client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructs() {
        let _client = Client::new();
    }
}
