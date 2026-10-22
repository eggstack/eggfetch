#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
)]

//! Fail-closed behavior for CLI protocol flags the built binary cannot honor.
//!
//! `--http2` and `--http3` are strict protocol controls. When the built
//! binary does not have the matching core feature compiled in, the CLI must
//! reject the request before any connection is attempted and name the missing
//! capability instead of silently negotiating a different protocol.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Wording the capability failure must not reuse.
const GENERIC_MARKER: &str = "HTTP client not available";

/// A minimal TCP server that counts accepted connections.
///
/// It never speaks a real protocol: the tests only need to know whether the
/// CLI opened a connection at all.
struct CountingServer {
    port: u16,
    accepted: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CountingServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        listener
            .set_nonblocking(true)
            .expect("failed to set nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let shutdown = Arc::new(AtomicBool::new(false));
        let accepted = Arc::new(AtomicUsize::new(0));

        let sd = shutdown.clone();
        let ac = accepted.clone();
        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        ac.fetch_add(1, Ordering::SeqCst);
                        thread::spawn(move || {
                            // Read the client's first bytes, then close so a
                            // client that expects a protocol handshake fails
                            // fast instead of waiting for a timeout.
                            let mut stream = stream;
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let mut buffer = [0_u8; 256];
                            let _ = stream.read(&mut buffer);
                        });
                    }
                    Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            port,
            accepted,
            shutdown,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    fn accepted_connections(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

impl Drop for CountingServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("failed to get test binary path")
        .parent()
        .expect("failed to get test binary dir")
        .parent()
        .expect("failed to get target dir")
        .to_path_buf();
    path.push("eggfetch");
    #[cfg(windows)]
    {
        path.set_extension("exe");
    }
    path
}

fn run_cli(args: &[&str]) -> (String, String, Option<i32>) {
    let bin = binary_path();
    let output = std::process::Command::new(&bin)
        .args(args)
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {}: {e}", bin.display()));

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code(),
    )
}

#[test]
#[cfg(not(feature = "http2"))]
fn http2_flag_without_the_capability_fails_closed() {
    let server = CountingServer::start();
    let (stdout, stderr, code) = run_cli(&["--http2", &server.url()]);

    assert_eq!(code, Some(5), "stderr: {stderr}");
    assert!(
        stderr.contains("HTTP/2"),
        "the failure must name the missing capability: {stderr}"
    );
    assert!(
        !stderr.contains(GENERIC_MARKER),
        "the failure must not reuse the generic missing-client wording: {stderr}"
    );
    assert!(
        stdout.is_empty(),
        "a rejected protocol request must not produce a response: {stdout}"
    );
    assert_eq!(
        server.accepted_connections(),
        0,
        "a rejected protocol request must not open a connection"
    );
}

#[test]
#[cfg(not(feature = "http3"))]
fn http3_flag_without_the_capability_fails_closed() {
    let server = CountingServer::start();
    let (stdout, stderr, code) = run_cli(&["--http3", &server.url()]);

    assert_eq!(code, Some(5), "stderr: {stderr}");
    assert!(
        stderr.contains("HTTP/3"),
        "the failure must name the missing capability: {stderr}"
    );
    assert!(
        !stderr.contains(GENERIC_MARKER),
        "the failure must not reuse the generic missing-client wording: {stderr}"
    );
    assert!(
        stdout.is_empty(),
        "a rejected protocol request must not produce a response: {stdout}"
    );
    assert_eq!(
        server.accepted_connections(),
        0,
        "a rejected protocol request must not open a connection"
    );
}

#[test]
#[cfg(feature = "http2")]
fn http2_flag_with_the_capability_is_not_rejected() {
    // The guard must not block a protocol this build supports: the CLI tries
    // the request and the connection is attempted.
    let server = CountingServer::start();
    let url = server.url();
    let (_stdout, stderr, _code) = run_cli(&["--http2", &url, "--timeout", "5"]);

    assert!(
        !stderr.contains("is not supported by this build"),
        "a build with HTTP/2 support must not reject the flag: {stderr}"
    );
    assert!(
        !stderr.contains(GENERIC_MARKER),
        "the capability wording must not reuse the generic missing-client text: {stderr}"
    );
    assert_eq!(
        server.accepted_connections(),
        1,
        "the request must be attempted when the capability is compiled in"
    );
}

#[test]
#[cfg(feature = "http3")]
fn http3_flag_with_the_capability_is_not_rejected() {
    // The guard must not block a protocol this build supports. The fixture
    // speaks no QUIC, so the request still fails — but not with the
    // capability rejection.
    let server = CountingServer::start();
    let url = server.url();
    let (_stdout, stderr, _code) = run_cli(&["--http3", &url, "--connect-timeout", "2"]);

    assert!(
        !stderr.contains("is not supported by this build"),
        "a build with HTTP/3 support must not reject the flag: {stderr}"
    );
    assert!(
        !stderr.contains(GENERIC_MARKER),
        "the capability wording must not reuse the generic missing-client text: {stderr}"
    );
}

#[test]
fn default_feature_set_is_unchanged() {
    // The published default CLI capability set must not change: the new
    // protocol features are opt-in and the core dependency keeps its feature
    // list.
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("failed to read the CLI manifest");

    assert!(
        manifest.contains("[features]"),
        "the CLI must declare its own feature set: {manifest}"
    );
    assert!(
        manifest.contains("default = []"),
        "the CLI default feature set must stay empty: {manifest}"
    );
    assert!(
        manifest.contains("http2 = [\"eggfetch-core/http2\"]"),
        "the http2 feature must forward to the core feature: {manifest}"
    );
    assert!(
        manifest.contains("http3 = [\"eggfetch-core/http3\"]"),
        "the http3 feature must forward to the core feature: {manifest}"
    );

    let core_dependency = manifest
        .lines()
        .find(|line| line.starts_with("eggfetch-core ="))
        .expect("the CLI must depend on eggfetch-core");
    assert!(
        !core_dependency.contains("http2") && !core_dependency.contains("http3"),
        "the default core dependency must not pull in a protocol capability: {core_dependency}"
    );
    assert!(
        core_dependency.contains("\"cookies\"")
            && core_dependency.contains("\"multipart\"")
            && core_dependency.contains("\"proxy\""),
        "the default core dependency feature list must stay unchanged: {core_dependency}"
    );
}
