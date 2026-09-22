//! Private streaming state and runtime ownership.

pub(super) const STATE_STREAMING: u8 = 0;
pub(super) const STATE_BUFFERED: u8 = 1;
pub(super) const STATE_CONSUMED: u8 = 2;
pub(super) const STATE_CLOSED: u8 = 3;

/// Keeps a synchronous client's runtime alive while a streaming response
/// outlives that client.
#[derive(Clone)]
pub(crate) struct RuntimeLease(Option<crate::client::RuntimeGuard>);

impl RuntimeLease {
    pub(crate) fn new(runtime: crate::client::RuntimeGuard) -> Self {
        Self(Some(runtime))
    }
}

impl Drop for RuntimeLease {
    fn drop(&mut self) {
        drop(self.0.take());
    }
}

#[derive(Clone, Copy)]
pub(super) enum StreamMode {
    Decoded,
    Raw,
}
