//! Command-line entry point for eggfetch.
//!
//! This binary is a thin wrapper around [`eggfetch_core`]. Argument parsing,
//! output formatting, and exit code mapping live here; all HTTP behavior
//! belongs to the core crate.

fn main() {
    println!("eggfetch {} (skeleton)", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_dependency_is_wired() {
        let _client = eggfetch_core::Client::new();
    }
}
