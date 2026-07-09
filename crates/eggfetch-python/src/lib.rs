//! Python bindings for eggfetch.
//!
//! Status: skeleton. PyO3/maturin packaging lands in Milestone F. Until
//! then, this crate simply re-exports [`eggfetch_core`] so the workspace
//! builds and the dependency boundary is exercised.

#[cfg(test)]
mod tests {
    #[test]
    fn core_is_reachable() {
        let _ = eggfetch_core::Client::new();
    }
}
