//! Shared utilities and test server for eggfetch benchmarks.
//!
//! This crate provides a lightweight local HTTP server for end-to-end benchmarks
//! and shared setup helpers. It is not published.

#![allow(missing_docs)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// A minimal blocking HTTP server for benchmarks.
///
/// Serves fixed responses with configurable body size, delay, and chunked encoding.
/// Each connection handles exactly one request (Connection: close).
pub struct BenchServer {
    port: u16,
    requests_served: Arc<AtomicUsize>,
}

/// Configuration for [`BenchServer`].
pub struct BenchServerConfig {
    /// Response body size in bytes.
    pub body_size: usize,
    /// Delay before sending response headers (ms).
    pub response_delay_ms: u64,
    /// Use chunked transfer encoding with this chunk size (0 = unchunked).
    pub chunk_size: usize,
    /// Delay between chunks (ms).
    pub chunk_delay_ms: u64,
    /// Read and discard the full request body before responding.
    pub consume_request_body: bool,
}

impl Default for BenchServerConfig {
    fn default() -> Self {
        Self {
            body_size: 1024,
            response_delay_ms: 0,
            chunk_size: 0,
            chunk_delay_ms: 0,
            consume_request_body: false,
        }
    }
}

impl BenchServer {
    /// Start a benchmark server on a random available port.
    ///
    /// # Panics
    ///
    /// Panics if binding to `127.0.0.1:0` fails.
    #[allow(clippy::needless_pass_by_value)]
    pub fn start(config: BenchServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        // Blocking mode: no missed connections.
        listener.set_nonblocking(false).unwrap();

        let requests_served = Arc::new(AtomicUsize::new(0));
        let rs = requests_served.clone();

        thread::spawn(move || {
            let body: Vec<u8> = vec![b'x'; config.body_size];
            // Accept loop - blocking accept, one connection per iteration.
            // Listener is dropped when this thread exits (shutdown).
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        rs.fetch_add(1, Ordering::Relaxed);
                        let cfg = BenchServerConfig {
                            body_size: config.body_size,
                            response_delay_ms: config.response_delay_ms,
                            chunk_size: config.chunk_size,
                            chunk_delay_ms: config.chunk_delay_ms,
                            consume_request_body: config.consume_request_body,
                        };
                        let body_clone = body.clone();
                        thread::spawn(move || {
                            handle_connection(stream, &cfg, &body_clone);
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        let server = Self {
            port,
            requests_served,
        };
        // Wait for the server thread to start accepting connections.
        for _ in 0..200 {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        server
    }

    /// Base URL for this server instance.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Port number.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Number of requests accepted by the server.
    pub fn requests_served(&self) -> usize {
        self.requests_served.load(Ordering::Relaxed)
    }

    /// Shut down the server. The server thread will exit when the process ends.
    #[allow(clippy::unused_self)]
    pub fn shutdown(&self) {}
}

fn handle_connection(mut stream: TcpStream, config: &BenchServerConfig, body: &[u8]) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut reader = BufReader::new(stream.try_clone().unwrap());

    // Read until blank line (end of HTTP headers).
    let mut content_length: usize = 0;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) if line.trim().is_empty() => break,
            Ok(_) => {
                if config.consume_request_body {
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("content-length:") {
                        if let Some(val) = line.split(':').nth(1) {
                            content_length = val.trim().parse().unwrap_or(0);
                        }
                    }
                }
            }
        }
    }

    // Consume request body if configured.
    if config.consume_request_body && content_length > 0 {
        let mut remaining = content_length;
        let mut buf = [0u8; 8192];
        while remaining > 0 {
            let to_read = remaining.min(buf.len());
            match reader.read(&mut buf[..to_read]) {
                Ok(0) | Err(_) => return,
                Ok(n) => remaining -= n,
            }
        }
    }

    if config.response_delay_ms > 0 {
        thread::sleep(Duration::from_millis(config.response_delay_ms));
    }

    let response = if config.chunk_size > 0 {
        "HTTP/1.1 200 OK\r\n\
             Content-Type: application/octet-stream\r\n\
             Transfer-Encoding: chunked\r\n\
             Connection: close\r\n\
             \r\n"
            .to_owned()
    } else {
        format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/octet-stream\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            body.len()
        )
    };

    if stream.write_all(response.as_bytes()).is_err() {
        return;
    }

    if config.chunk_size > 0 {
        let mut offset = 0;
        while offset < body.len() {
            let end = (offset + config.chunk_size).min(body.len());
            let chunk = &body[offset..end];
            let header = format!("{:x}\r\n", chunk.len());
            if stream.write_all(header.as_bytes()).is_err() {
                return;
            }
            if stream.write_all(chunk).is_err() {
                return;
            }
            if stream.write_all(b"\r\n").is_err() {
                return;
            }
            offset = end;
            if config.chunk_delay_ms > 0 && offset < body.len() {
                thread::sleep(Duration::from_millis(config.chunk_delay_ms));
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n");
    } else {
        let _ = stream.write_all(body);
    }
}
