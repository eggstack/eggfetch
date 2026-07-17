#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
//! Local TCP test server for connection management and streaming tests.

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
    /// Custom response body. If None, defaults to b"OK".
    pub response_body: Option<Vec<u8>>,
    /// If true, send response using chunked transfer encoding.
    /// Each chunk is sent with an optional inter-chunk delay.
    pub chunked: bool,
    /// Delay between chunked response chunks (ms). 0 = no delay.
    pub chunk_delay_ms: u64,
    /// After this many chunks have been sent, sleep `chunk_stall_ms`
    /// milliseconds before the next chunk (only meaningful when
    /// `chunked` is true).
    pub chunk_stall_after: Option<usize>,
    /// Stall duration in ms after `chunk_stall_after` chunks.
    pub chunk_stall_ms: u64,
    /// If set, send a redirect response with this status code and the
    /// value as the Location header. The body is empty for redirects.
    pub redirect: Option<(u16, String)>,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            close_connection: false,
            response_delay_ms: 0,
            consume_body: true,
            response_body: None,
            chunked: false,
            chunk_delay_ms: 0,
            chunk_stall_after: None,
            chunk_stall_ms: 0,
            redirect: None,
        }
    }
}

/// A running local TCP test server.
pub struct TestServer {
    port: u16,
    #[allow(dead_code)]
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
        let resp_body = config.response_body.clone();
        let chunked = config.chunked;
        let chunk_delay = config.chunk_delay_ms;
        let chunk_stall_after = config.chunk_stall_after;
        let chunk_stall_ms = config.chunk_stall_ms;
        let redirect = config.redirect.clone();

        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                if let Ok((stream, _)) = listener.accept() {
                    ca.fetch_add(1, Ordering::SeqCst);
                    let rs = rs.clone();
                    let conn_config = ConnectionConfig {
                        close_connection: close,
                        response_delay_ms: delay,
                        consume_body: consume,
                        response_body: resp_body.clone(),
                        chunked,
                        chunk_delay_ms: chunk_delay,
                        redirect: redirect.clone(),
                    };
                    thread::spawn(move || {
                        handle_connection(
                            stream,
                            &conn_config,
                            chunk_stall_after,
                            chunk_stall_ms,
                            &rs,
                        );
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
    #[allow(dead_code)]
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

fn send_redirect(
    stream: &mut std::net::TcpStream,
    status: u16,
    location: &str,
    close_connection: bool,
) -> bool {
    let status_text = match status {
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        _ => "Redirect",
    };
    let conn = if close_connection {
        "close"
    } else {
        "keep-alive"
    };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\nLocation: {location}\r\n\
         Connection: {conn}\r\nContent-Length: 0\r\n\r\n"
    );
    if stream.write_all(response.as_bytes()).is_err() {
        return false;
    }
    if stream.flush().is_err() {
        return false;
    }
    if close_connection {
        let _ = stream.shutdown(std::net::Shutdown::Both);
        false
    } else {
        true
    }
}

fn send_chunked(
    stream: &mut std::net::TcpStream,
    body: &[u8],
    conn: &str,
    chunk_delay_ms: u64,
    chunk_stall_after: Option<usize>,
    chunk_stall_ms: u64,
) -> bool {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
         Connection: {conn}\r\nTransfer-Encoding: chunked\r\n\r\n"
    );
    if stream.write_all(header.as_bytes()).is_err() {
        return false;
    }
    let chunk_size = 3;
    for (chunks_sent, chunk) in body.chunks(chunk_size).enumerate() {
        if chunk_delay_ms > 0 {
            thread::sleep(Duration::from_millis(chunk_delay_ms));
        }
        if let Some(stall_after) = chunk_stall_after {
            if chunks_sent == stall_after && chunk_stall_ms > 0 {
                thread::sleep(Duration::from_millis(chunk_stall_ms));
            }
        }
        if stream
            .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
            .is_err()
        {
            return false;
        }
        if stream.write_all(chunk).is_err() {
            return false;
        }
        if stream.write_all(b"\r\n").is_err() {
            return false;
        }
    }
    stream.write_all(b"0\r\n\r\n").is_ok() && stream.flush().is_ok()
}

fn send_normal(stream: &mut std::net::TcpStream, body: &[u8], conn: &str) -> bool {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\
         Connection: {conn}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(response.as_bytes()).is_ok()
        && stream.write_all(body).is_ok()
        && stream.flush().is_ok()
}

fn handle_connection(
    stream: std::net::TcpStream,
    config: &ConnectionConfig,
    chunk_stall_after: Option<usize>,
    chunk_stall_ms: u64,
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

        if config.consume_body && content_length > 0 {
            let mut body = vec![0u8; content_length];
            let mut total = 0usize;
            while total < content_length {
                match std::io::Read::read(&mut reader, &mut body[total..]) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n,
                }
            }
        }

        requests_served.fetch_add(1, Ordering::SeqCst);

        if config.response_delay_ms > 0 {
            thread::sleep(Duration::from_millis(config.response_delay_ms));
        }

        let stream = reader.get_mut();

        if let Some((status, ref location)) = config.redirect {
            if !send_redirect(stream, status, location, config.close_connection) {
                break;
            }
            continue;
        }

        let body = config.response_body.as_deref().unwrap_or(b"OK");
        let conn = if config.close_connection {
            "close"
        } else {
            "keep-alive"
        };

        let ok = if config.chunked {
            send_chunked(
                stream,
                body,
                conn,
                config.chunk_delay_ms,
                chunk_stall_after,
                chunk_stall_ms,
            )
        } else {
            send_normal(stream, body, conn)
        };

        if !ok || config.close_connection {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            break;
        }
    }
}

/// Per-connection configuration derived from `TestServerConfig`.
struct ConnectionConfig {
    close_connection: bool,
    response_delay_ms: u64,
    consume_body: bool,
    response_body: Option<Vec<u8>>,
    chunked: bool,
    chunk_delay_ms: u64,
    redirect: Option<(u16, String)>,
}
