#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
)]

//! The CLI's opt-in diagnostics channel.
//!
//! The engine emits `tracing` events when it drops a wire value it cannot
//! interpret. With the CLI's `tracing` feature the operator sees that on
//! stderr; without it the drop is silent. The two tests below pin both
//! halves of that difference against the same fixture, so the feature's
//! effect is observed rather than assumed.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The diagnostic the engine emits when it drops a non-UTF-8 header value.
const DIAGNOSTIC: &str = "dropped non-UTF8 content-encoding";

/// A server that answers with a `content-encoding` header that is not UTF-8.
struct CorruptHeaderServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CorruptHeaderServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        listener
            .set_nonblocking(true)
            .expect("failed to set nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let shutdown = Arc::new(AtomicBool::new(false));

        let sd = shutdown.clone();
        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        thread::spawn(move || {
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let mut buffer = [0_u8; 1024];
                            let _ = stream.read(&mut buffer);
                            let _ = stream.write_all(&response_with_non_utf8_header());
                            let _ = stream.flush();
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
            shutdown,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

impl Drop for CorruptHeaderServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A response whose `content-encoding` value is a raw non-UTF-8 byte.
///
/// The byte is legal on the wire as an opaque header value, and it is not
/// valid UTF-8, which is exactly the condition the engine reports.
fn response_with_non_utf8_header() -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Encoding: ");
    response.push(0xff);
    response.extend_from_slice(b"\r\n\r\nok");
    response
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
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {}: {e}", bin.display()));

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code(),
    )
}

#[test]
#[cfg(feature = "tracing")]
fn engine_diagnostics_reach_stderr_when_the_feature_is_on() {
    let server = CorruptHeaderServer::start();
    let url = server.url();

    let (stdout, stderr, code) = run_cli(&[&url]);

    assert_eq!(code, Some(0), "the request must succeed: {stderr}");
    assert!(
        stdout.contains("ok"),
        "the response body must still be delivered: {stdout}"
    );
    assert!(
        stderr.contains(DIAGNOSTIC),
        "the engine diagnostic must reach stderr: {stderr}"
    );
}

#[test]
#[cfg(not(feature = "tracing"))]
fn engine_diagnostics_are_silent_without_the_feature() {
    // The other half of the same fixture: without the feature there is no
    // subscriber, so the drop is silent — but the request still succeeds.
    // The two tests together show the feature is what makes the diagnostic
    // observable.
    let server = CorruptHeaderServer::start();
    let url = server.url();

    let (stdout, stderr, code) = run_cli(&[&url]);

    assert_eq!(code, Some(0), "the request must succeed: {stderr}");
    assert!(
        stdout.contains("ok"),
        "the response body must still be delivered: {stdout}"
    );
    assert!(
        !stderr.contains(DIAGNOSTIC),
        "the diagnostic must not be emitted without the feature: {stderr}"
    );
}
