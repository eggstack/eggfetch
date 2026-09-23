//! Blocking loopback HTTP forward proxy used only by the e2e benchmarks.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// A small HTTP-only proxy fixture. CONNECT and chunked request bodies are
/// rejected explicitly; neither is used by the proxy-overhead benchmark.
pub struct BenchProxy {
    port: u16,
    stop: Arc<AtomicBool>,
    connections_served: Arc<AtomicUsize>,
    listener_thread: Option<JoinHandle<()>>,
}

impl BenchProxy {
    /// Start the fixture on a loopback ephemeral port.
    ///
    /// # Panics
    ///
    /// Panics if the loopback listener cannot be created or configured.
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy");
        let port = listener.local_addr().expect("proxy address").port();
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let stop = Arc::new(AtomicBool::new(false));
        let connections_served = Arc::new(AtomicUsize::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_count = Arc::clone(&connections_served);
        let listener_thread = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        thread_count.fetch_add(1, Ordering::Relaxed);
                        if let Ok(handler) = thread::Builder::new()
                            .name("eggfetch-bench-proxy-client".into())
                            .spawn(move || handle_connection(stream))
                        {
                            handlers.push(handler);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
            for handler in handlers {
                let _ = handler.join();
            }
        });
        Self {
            port,
            stop,
            connections_served,
            listener_thread: Some(listener_thread),
        }
    }

    /// Return the proxy's HTTP URL.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Number of accepted client connections.
    pub fn connections_served(&self) -> usize {
        self.connections_served.load(Ordering::Relaxed)
    }

    /// Stop accepting connections and join the listener thread.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.listener_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BenchProxy {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// Keep parsing, forwarding, and framing adjacent: this small fixture has a
// single-request protocol boundary and its tests exercise that whole flow.
#[allow(clippy::too_many_lines)]
fn handle_connection(mut client: TcpStream) {
    let _ = client.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = client.set_write_timeout(Some(Duration::from_secs(10)));
    let Ok(clone) = client.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(clone);
    let mut request_line = String::new();
    if reader
        .read_line(&mut request_line)
        .ok()
        .is_none_or(|read| read == 0)
    {
        return;
    }
    let fields: Vec<_> = request_line.split_whitespace().collect();
    if fields.len() != 3 || !fields[2].starts_with("HTTP/1.") {
        respond(&mut client, "400 Bad Request");
        return;
    }
    let method = fields[0];
    let target = fields[1];
    if method.eq_ignore_ascii_case("CONNECT") {
        respond(&mut client, "501 Not Implemented");
        return;
    }

    let mut headers = Vec::new();
    let mut host = None;
    let mut content_length = 0_usize;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {
                let Some((name, value)) = line.split_once(':') else {
                    respond(&mut client, "400 Bad Request");
                    return;
                };
                if name.eq_ignore_ascii_case("host") {
                    host = Some(value.trim().to_owned());
                } else if name.eq_ignore_ascii_case("content-length") {
                    let Ok(length) = value.trim().parse::<usize>() else {
                        respond(&mut client, "400 Bad Request");
                        return;
                    };
                    content_length = length;
                } else if name.eq_ignore_ascii_case("transfer-encoding") {
                    respond(&mut client, "501 Not Implemented");
                    return;
                }
                headers.push(line.clone());
            }
        }
    }

    let (authority, path) = if let Ok(url) = url::Url::parse(target) {
        if url.scheme() != "http" {
            respond(&mut client, "400 Bad Request");
            return;
        }
        let Some(authority) = url.host_str().map(|h| match url.port() {
            Some(port) => format!("{h}:{port}"),
            None => h.to_owned(),
        }) else {
            respond(&mut client, "400 Bad Request");
            return;
        };
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        (authority, path)
    } else if let Some(host) = host {
        (host, target.to_owned())
    } else {
        respond(&mut client, "400 Bad Request");
        return;
    };

    let Ok(mut upstream) = TcpStream::connect(&authority) else {
        respond(&mut client, "502 Bad Gateway");
        return;
    };
    let _ = upstream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = upstream.set_write_timeout(Some(Duration::from_secs(10)));
    if write!(upstream, "{method} {path} HTTP/1.1\r\n").is_err() {
        return;
    }
    for header in headers {
        let name = header.split_once(':').map_or("", |(name, _)| name);
        if name.eq_ignore_ascii_case("proxy-connection")
            || name.eq_ignore_ascii_case("proxy-authorization")
            || name.eq_ignore_ascii_case("connection")
        {
            continue;
        }
        if upstream.write_all(header.as_bytes()).is_err() {
            return;
        }
    }
    if upstream.write_all(b"Connection: close\r\n\r\n").is_err() {
        return;
    }
    let mut body = (&mut reader).take(content_length as u64);
    match std::io::copy(&mut body, &mut upstream) {
        Ok(copied) if copied == content_length as u64 => {}
        Ok(_) | Err(_) => return,
    }
    let _ = std::io::copy(&mut upstream, &mut client);
}

fn respond(client: &mut TcpStream, status: &str) {
    let _ = write!(
        client,
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
}

#[cfg(test)]
mod tests {
    use super::BenchProxy;
    use eggfetch_core::{Client, Proxy};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    type RequestRecords = Arc<Mutex<Vec<Vec<u8>>>>;

    fn origin() -> (String, RequestRecords, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let records = Arc::clone(&seen);
        let handle = thread::spawn(move || {
            for _ in 0..3 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        return;
                    }
                    request.push(byte[0]);
                }
                records.lock().unwrap().push(request);
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .unwrap();
            }
        });
        (format!("http://{address}"), seen, handle)
    }

    #[tokio::test]
    async fn forwards_complete_origin_form_requests_and_repeated_gets() {
        let (origin_url, records, origin_thread) = origin();
        let mut proxy = BenchProxy::start();
        let client = Client::builder()
            .proxy(Proxy::http(&proxy.url()).unwrap())
            .build();
        for path in ["/first?q=1", "/second?q=two", "/third"] {
            let url = format!("{origin_url}{path}");
            let mut response = client.get(&url).unwrap().send().await.unwrap();
            assert_eq!(response.bytes().await.unwrap().as_ref(), b"ok");
        }
        origin_thread.join().unwrap();
        let requests = records.lock().unwrap();
        assert_eq!(requests.len(), 3);
        for (request, path) in requests
            .iter()
            .zip(["/first?q=1", "/second?q=two", "/third"])
        {
            let text = String::from_utf8_lossy(request);
            assert!(
                text.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
                "{text}"
            );
            let expected_host = origin_url.trim_start_matches("http://");
            assert!(
                text.lines().any(|line| {
                    line.split_once(':').is_some_and(|(name, value)| {
                        name.eq_ignore_ascii_case("host") && value.trim() == expected_host
                    })
                }),
                "{text}"
            );
            assert!(!text.to_ascii_lowercase().contains("proxy-connection:"));
            assert!(text.ends_with("\r\n\r\n"));
        }
        assert_eq!(proxy.connections_served(), 3);
        proxy.shutdown();
    }
}
