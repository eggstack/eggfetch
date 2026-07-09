//! Local TCP test server for connection management tests.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Configuration for a test server instance.
pub struct TestServerConfig {
    /// If true, send `Connection: close` in responses.
    pub close_connection: bool,
    /// Delay before responding (ms). 0 = immediate.
    pub response_delay_ms: u64,
    /// If true, read and discard the request body.
    pub consume_body: bool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            close_connection: false,
            response_delay_ms: 0,
            consume_body: true,
        }
    }
}

/// A running local TCP test server.
pub struct TestServer {
    port: u16,
    connections_accepted: Arc<AtomicUsize>,
    #[allow(dead_code)]
    requests_served: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Start a test server on a random available port.
    ///
    /// Uses a blocking accept loop. The blocking accept is intentional: a
    /// non-blocking accept with a busy-sleep thread interferes with
    /// hyper-util's connection pool in `#[tokio::test]` (current-thread
    /// runtime). To shut down, we open a dummy connection from this thread
    /// to ourselves, which unblocks `accept` and lets the loop observe the
    /// shutdown flag.
    ///
    /// # Panics
    ///
    /// Panics if the server cannot bind to a local port.
    pub fn start(config: &TestServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        let port = listener.local_addr().unwrap().port();
        let connections_accepted = Arc::new(AtomicUsize::new(0));
        let requests_served = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        let ca = connections_accepted.clone();
        let rs = requests_served.clone();
        let sd = shutdown.clone();
        let close = config.close_connection;
        let delay = config.response_delay_ms;
        let consume = config.consume_body;

        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                if let Ok((stream, _)) = listener.accept() {
                    ca.fetch_add(1, Ordering::SeqCst);
                    let rs = rs.clone();
                    thread::spawn(move || {
                        handle_connection(stream, close, delay, consume, &rs);
                    });
                } else if sd.load(Ordering::Relaxed) {
                    break;
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        });

        Self {
            port,
            connections_accepted,
            requests_served,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Returns the port the server is listening on.
    #[allow(dead_code)]
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the base URL of the server (e.g. `http://127.0.0.1:12345/`).
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// Returns the number of TCP connections accepted so far.
    #[must_use]
    pub fn connections_accepted(&self) -> usize {
        self.connections_accepted.load(Ordering::SeqCst)
    }

    /// Returns the total number of HTTP requests served.
    #[allow(dead_code)]
    #[must_use]
    pub fn requests_served(&self) -> usize {
        self.requests_served.load(Ordering::SeqCst)
    }

    /// Shut down the server and wait for the accept loop to exit.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn handle_connection(
    stream: std::net::TcpStream,
    close_connection: bool,
    response_delay_ms: u64,
    consume_body: bool,
    requests_served: &Arc<AtomicUsize>,
) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut reader = BufReader::new(stream);

    loop {
        let mut rl = String::new();
        match reader.read_line(&mut rl) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if rl.trim().is_empty() {
            break;
        }

        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                break;
            }
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(val) = line.split(':').nth(1) {
                    content_length = val.trim().parse().unwrap_or(0);
                }
            }
        }

        if consume_body && content_length > 0 {
            let mut body = vec![0u8; content_length];
            let _ = std::io::Read::read(&mut reader, &mut body);
        }

        requests_served.fetch_add(1, Ordering::SeqCst);

        if response_delay_ms > 0 {
            thread::sleep(Duration::from_millis(response_delay_ms));
        }

        let connection_header = if close_connection {
            "close"
        } else {
            "keep-alive"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: {connection_header}\r\nContent-Length: 2\r\n\r\nOK"
        );

        let stream = reader.get_mut();
        if stream.write_all(response.as_bytes()).is_err() {
            break;
        }
        if stream.flush().is_err() {
            break;
        }

        if close_connection {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            break;
        }
    }
}
