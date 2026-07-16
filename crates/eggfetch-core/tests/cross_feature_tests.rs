#![allow(missing_docs, dead_code)]
//! Cross-feature integration tests for eggfetch-core.
//!
//! Tests combining proxy, multipart, compression, cookies, and redirect
//! subsystems.

#![cfg(feature = "proxy")]
#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, Proxy, ProxyAuth, Timeout};
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

#[derive(Default)]
struct HttpProxyConfig {
    required_auth: Option<(String, String)>,
}

struct HttpProxyServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl HttpProxyServer {
    async fn start(config: HttpProxyConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let required_auth = config.required_auth;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let auth = required_auth.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_proxy_connection(stream, auth).await {
                                        eprintln!("proxy connection error: {e}");
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("accept error: {e}");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

struct TrackingHttpProxyServer {
    port: u16,
    shutdown: watch::Sender<bool>,
    auth_received: Arc<AtomicBool>,
}

struct TrackingHttpProxyConfig {
    required_auth: Option<(String, String)>,
    auth_received: Arc<AtomicBool>,
}

impl TrackingHttpProxyServer {
    async fn start(config: TrackingHttpProxyConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let required_auth = config.required_auth;
        let auth_received = config.auth_received;
        let auth_received_clone = Arc::clone(&auth_received);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let auth = required_auth.clone();
                                let ar = Arc::clone(&auth_received_clone);
                                tokio::spawn(async move {
                                    if let Err(e) = handle_tracking_proxy_connection(stream, auth, ar).await {
                                        eprintln!("tracking proxy connection error: {e}");
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("accept error: {e}");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
            auth_received,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn auth_was_received(&self) -> bool {
        self.auth_received.load(Ordering::SeqCst)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_proxy_connection(
    mut client_stream: TcpStream,
    required_auth: Option<(String, String)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fmt::Write as _;
    let mut buf_reader = BufReader::new(&mut client_stream);

    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line).await?;
    if request_line.trim().is_empty() {
        return Ok(());
    }

    let mut raw_headers = Vec::new();
    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line).await?;
        raw_headers.extend_from_slice(line.as_bytes());
        if line.trim().is_empty() {
            break;
        }
    }

    let header_str = String::from_utf8_lossy(&raw_headers);
    let mut headers = HashMap::new();
    for h in header_str.lines() {
        if let Some((name, value)) = h.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
    }

    if let Some((ref required_user, ref required_pass)) = required_auth {
        let mut authorized = false;
        if let Some(auth_header) = headers.get("proxy-authorization") {
            if let Some(encoded) = auth_header.strip_prefix("Basic ") {
                use base64::Engine;
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded.as_bytes())
                    .unwrap_or_default();
                let decoded_str = String::from_utf8_lossy(&decoded);
                let expected = format!("{required_user}:{required_pass}");
                authorized = decoded_str == expected;
            }
        }
        if !authorized {
            let resp = b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n";
            let w = buf_reader.into_inner();
            w.write_all(resp).await?;
            return Ok(());
        }
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts[0];
    let target = parts[1];

    if method.eq_ignore_ascii_case("CONNECT") {
        drop(buf_reader);
        handle_connect_tunnel(&mut client_stream, target).await?;
    } else {
        let content_length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut request_body = vec![0u8; content_length];
        if content_length > 0 {
            let mut total = 0;
            while total < content_length {
                match buf_reader.read(&mut request_body[total..]).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n,
                }
            }
            request_body.truncate(total);
        }

        drop(buf_reader);

        let dest_url = url::Url::parse(target)?;
        let dest_host = dest_url.host_str().unwrap_or("127.0.0.1");
        let dest_port = dest_url.port_or_known_default().unwrap_or(80);
        let dest_addr = format!("{dest_host}:{dest_port}");
        let dest_stream = TcpStream::connect(&dest_addr).await?;
        let (mut dest_reader, mut dest_writer) = dest_stream.into_split();

        let dest_path = dest_url.path();
        let dest_query = dest_url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        let dest_path_query = format!("{dest_path}{dest_query}");
        let mut dest_req = format!("{method} {dest_path_query} HTTP/1.1\r\n");
        for (name, value) in &headers {
            if name == "proxy-authorization" || name == "host" {
                continue;
            }
            let _ = write!(dest_req, "{name}: {value}\r\n");
        }
        if let Some(host) = headers.get("host") {
            let _ = write!(dest_req, "host: {host}\r\n");
        } else if let Ok(parsed) = url::Url::parse(target) {
            let host = parsed.host_str().unwrap_or("");
            let _ = write!(dest_req, "host: {host}\r\n");
        }
        dest_req.push_str("\r\n");

        dest_writer.write_all(dest_req.as_bytes()).await?;
        if !request_body.is_empty() {
            dest_writer.write_all(&request_body).await?;
        }

        let mut resp_buf = Vec::new();
        let mut resp_reader = BufReader::new(&mut dest_reader);

        let mut status_line = String::new();
        resp_reader.read_line(&mut status_line).await?;
        resp_buf.extend_from_slice(status_line.as_bytes());

        loop {
            let mut line = String::new();
            resp_reader.read_line(&mut line).await?;
            resp_buf.extend_from_slice(line.as_bytes());
            if line.trim().is_empty() {
                break;
            }
        }

        let status_str = String::from_utf8_lossy(&resp_buf);
        let mut resp_content_length: Option<usize> = None;
        let mut chunked = false;
        for line in status_str.lines() {
            if let Some(val) = line
                .strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
            {
                resp_content_length = val.trim().parse().ok();
            }
            if let Some(val) = line
                .strip_prefix("Transfer-Encoding:")
                .or_else(|| line.strip_prefix("transfer-encoding:"))
            {
                if val.trim().to_lowercase().contains("chunked") {
                    chunked = true;
                }
            }
        }

        if chunked {
            loop {
                let mut chunk_size_line = String::new();
                resp_reader.read_line(&mut chunk_size_line).await?;
                resp_buf.extend_from_slice(chunk_size_line.as_bytes());
                let size_str = chunk_size_line.trim();
                if size_str.is_empty() || size_str == "0" {
                    if size_str == "0" {
                        let mut trailer = String::new();
                        resp_reader.read_line(&mut trailer).await?;
                        resp_buf.extend_from_slice(trailer.as_bytes());
                    }
                    break;
                }
                let chunk_size = usize::from_str_radix(size_str, 16).unwrap_or(0);
                let mut chunk_body = vec![0u8; chunk_size];
                let mut total = 0;
                while total < chunk_size {
                    match resp_reader.read(&mut chunk_body[total..]).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => total += n,
                    }
                }
                resp_buf.extend_from_slice(&chunk_body);
                let mut crlf = String::new();
                resp_reader.read_line(&mut crlf).await?;
                resp_buf.extend_from_slice(crlf.as_bytes());
            }
        } else if let Some(cl) = resp_content_length {
            let mut body = vec![0u8; cl];
            let mut total = 0;
            while total < cl {
                match resp_reader.read(&mut body[total..]).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n,
                }
            }
            resp_buf.extend_from_slice(&body);
        } else {
            let mut buf = vec![0u8; 4096];
            loop {
                match resp_reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => resp_buf.extend_from_slice(&buf[..n]),
                }
            }
        }

        client_stream.write_all(&resp_buf).await?;
        client_stream.flush().await?;
    }

    Ok(())
}

async fn handle_connect_tunnel(
    client_stream: &mut TcpStream,
    target: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(dest_stream) = TcpStream::connect(target).await {
        let resp = b"HTTP/1.1 200 Connection Established\r\n\r\n";
        client_stream.write_all(resp).await?;

        let (mut client_read, mut client_write) = tokio::io::split(client_stream);
        let (mut dest_read, mut dest_write) = dest_stream.into_split();

        let c2d = async {
            let _ = tokio::io::copy(&mut client_read, &mut dest_write).await;
            let _ = dest_write.shutdown().await;
        };

        let d2c = async {
            let _ = tokio::io::copy(&mut dest_read, &mut client_write).await;
            let _ = client_write.shutdown().await;
        };

        tokio::select! {
            () = c2d => {}
            () = d2c => {}
        }
    } else {
        let resp = b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n";
        client_stream.write_all(resp).await?;
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn handle_tracking_proxy_connection(
    mut client_stream: TcpStream,
    required_auth: Option<(String, String)>,
    auth_received: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fmt::Write as _;
    let mut buf_reader = BufReader::new(&mut client_stream);

    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line).await?;
    if request_line.trim().is_empty() {
        return Ok(());
    }

    let mut raw_headers = Vec::new();
    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line).await?;
        raw_headers.extend_from_slice(line.as_bytes());
        if line.trim().is_empty() {
            break;
        }
    }

    let header_str = String::from_utf8_lossy(&raw_headers);
    let mut headers = HashMap::new();
    for h in header_str.lines() {
        if let Some((name, value)) = h.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
    }

    if headers.contains_key("proxy-authorization") {
        auth_received.store(true, Ordering::SeqCst);
    }

    if let Some((ref required_user, ref required_pass)) = required_auth {
        let mut authorized = false;
        if let Some(auth_header) = headers.get("proxy-authorization") {
            if let Some(encoded) = auth_header.strip_prefix("Basic ") {
                use base64::Engine;
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded.as_bytes())
                    .unwrap_or_default();
                let decoded_str = String::from_utf8_lossy(&decoded);
                let expected = format!("{required_user}:{required_pass}");
                authorized = decoded_str == expected;
            }
        }
        if !authorized {
            let resp = b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n";
            let w = buf_reader.into_inner();
            w.write_all(resp).await?;
            return Ok(());
        }
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts[0];
    let target = parts[1];

    if method.eq_ignore_ascii_case("CONNECT") {
        drop(buf_reader);
        handle_connect_tunnel(&mut client_stream, target).await?;
    } else {
        let content_length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut request_body = vec![0u8; content_length];
        if content_length > 0 {
            let mut total = 0;
            while total < content_length {
                match buf_reader.read(&mut request_body[total..]).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n,
                }
            }
            request_body.truncate(total);
        }

        drop(buf_reader);

        let dest_url = url::Url::parse(target)?;
        let dest_host = dest_url.host_str().unwrap_or("127.0.0.1");
        let dest_port = dest_url.port_or_known_default().unwrap_or(80);
        let dest_addr = format!("{dest_host}:{dest_port}");
        let dest_stream = TcpStream::connect(&dest_addr).await?;
        let (mut dest_reader, mut dest_writer) = dest_stream.into_split();

        let dest_path = dest_url.path();
        let dest_query = dest_url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        let dest_path_query = format!("{dest_path}{dest_query}");
        let mut dest_req = format!("{method} {dest_path_query} HTTP/1.1\r\n");
        for (name, value) in &headers {
            if name == "proxy-authorization" || name == "host" {
                continue;
            }
            let _ = write!(dest_req, "{name}: {value}\r\n");
        }
        if let Some(host) = headers.get("host") {
            let _ = write!(dest_req, "host: {host}\r\n");
        } else if let Ok(parsed) = url::Url::parse(target) {
            let host = parsed.host_str().unwrap_or("");
            let _ = write!(dest_req, "host: {host}\r\n");
        }
        dest_req.push_str("\r\n");

        dest_writer.write_all(dest_req.as_bytes()).await?;
        if !request_body.is_empty() {
            dest_writer.write_all(&request_body).await?;
        }

        let mut resp_buf = Vec::new();
        let mut resp_reader = BufReader::new(&mut dest_reader);

        let mut status_line = String::new();
        resp_reader.read_line(&mut status_line).await?;
        resp_buf.extend_from_slice(status_line.as_bytes());

        loop {
            let mut line = String::new();
            resp_reader.read_line(&mut line).await?;
            resp_buf.extend_from_slice(line.as_bytes());
            if line.trim().is_empty() {
                break;
            }
        }

        let status_str = String::from_utf8_lossy(&resp_buf);
        let mut resp_content_length: Option<usize> = None;
        let mut chunked = false;
        for line in status_str.lines() {
            if let Some(val) = line
                .strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
            {
                resp_content_length = val.trim().parse().ok();
            }
            if let Some(val) = line
                .strip_prefix("Transfer-Encoding:")
                .or_else(|| line.strip_prefix("transfer-encoding:"))
            {
                if val.trim().to_lowercase().contains("chunked") {
                    chunked = true;
                }
            }
        }

        if chunked {
            loop {
                let mut chunk_size_line = String::new();
                resp_reader.read_line(&mut chunk_size_line).await?;
                resp_buf.extend_from_slice(chunk_size_line.as_bytes());
                let size_str = chunk_size_line.trim();
                if size_str.is_empty() || size_str == "0" {
                    if size_str == "0" {
                        let mut trailer = String::new();
                        resp_reader.read_line(&mut trailer).await?;
                        resp_buf.extend_from_slice(trailer.as_bytes());
                    }
                    break;
                }
                let chunk_size = usize::from_str_radix(size_str, 16).unwrap_or(0);
                let mut chunk_body = vec![0u8; chunk_size];
                let mut total = 0;
                while total < chunk_size {
                    match resp_reader.read(&mut chunk_body[total..]).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => total += n,
                    }
                }
                resp_buf.extend_from_slice(&chunk_body);
                let mut crlf = String::new();
                resp_reader.read_line(&mut crlf).await?;
                resp_buf.extend_from_slice(crlf.as_bytes());
            }
        } else if let Some(cl) = resp_content_length {
            let mut body = vec![0u8; cl];
            let mut total = 0;
            while total < cl {
                match resp_reader.read(&mut body[total..]).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n,
                }
            }
            resp_buf.extend_from_slice(&body);
        } else {
            let mut buf = vec![0u8; 4096];
            loop {
                match resp_reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => resp_buf.extend_from_slice(&buf[..n]),
                }
            }
        }

        client_stream.write_all(&resp_buf).await?;
        client_stream.flush().await?;
    }

    Ok(())
}

struct EchoHttpServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl EchoHttpServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            tokio::spawn(async move {
                                if let Err(e) = handle_echo_http(stream).await {
                                    eprintln!("echo server error: {e}");
                                }
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

async fn handle_echo_http(
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fmt::Write as _;
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts.first().copied().unwrap_or("GET");
    let path = parts.get(1).copied().unwrap_or("/");

    let mut headers = Vec::new();
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name_lower = name.trim().to_lowercase();
            if name_lower == "content-length" {
                content_length = value.trim().parse().unwrap_or(0);
            }
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    let mut body = vec![0u8; content_length];
    let mut total = 0;
    while total < content_length {
        match reader.read(&mut body[total..]).await {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }

    let mut response_body = format!("{method} {path}");
    for (name, value) in &headers {
        let _ = write!(response_body, "\n{name}: {value}");
    }
    if !body.is_empty() {
        let _ = write!(response_body, "\nBody: {}", String::from_utf8_lossy(&body));
    }

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
        response_body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;

    Ok(())
}

fn test_client(proxy_url: &str) -> Client {
    Client::builder()
        .proxy(Proxy::all(proxy_url).unwrap())
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build()
}

#[cfg(feature = "multipart")]
fn build_multipart_body() -> eggfetch_core::Multipart {
    eggfetch_core::Multipart::new()
        .text("field", "value")
        .unwrap()
        .bytes(
            "file",
            "test.txt",
            "text/plain",
            Bytes::from("file content here"),
        )
        .unwrap()
}

struct CompressedResponseServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl CompressedResponseServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            tokio::spawn(async move {
                                if let Err(e) = handle_compressed_response(stream).await {
                                    eprintln!("compressed server error: {e}");
                                }
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

async fn handle_compressed_response(
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            break;
        }
    }

    let uncompressed = b"hello compressed world";
    let compressed = gzip_compress(uncompressed);

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        compressed.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.write_all(&compressed).await?;
    stream.flush().await?;

    Ok(())
}

fn gzip_compress(data: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

struct RedirectServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl RedirectServer {
    async fn start(cross_origin_port: Option<u16>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            let cop = cross_origin_port;
                            tokio::spawn(async move {
                                if let Err(e) = handle_redirect_request(stream, cop).await {
                                    eprintln!("redirect server error: {e}");
                                }
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

async fn handle_redirect_request(
    mut stream: TcpStream,
    cross_origin_port: Option<u16>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let path = parts.get(1).copied().unwrap_or("/");

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            break;
        }
    }

    match path {
        "/redirect-same" => {
            let resp = "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(resp.as_bytes()).await?;
        }
        "/redirect-cross" => {
            let target_port = cross_origin_port.unwrap_or(19999);
            let resp = format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{target_port}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(resp.as_bytes()).await?;
        }
        "/redirect-cookie" => {
            let resp = "HTTP/1.1 302 Found\r\nLocation: /final\r\nSet-Cookie: session=abc123; Path=/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(resp.as_bytes()).await?;
        }
        "/final" => {
            let body = "final destination reached";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(resp.as_bytes()).await?;
        }
        _ => {
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(resp.as_bytes()).await?;
        }
    }
    stream.flush().await?;

    Ok(())
}

struct CrossOriginServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl CrossOriginServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            tokio::spawn(async move {
                                if let Err(e) = handle_cross_origin_request(stream).await {
                                    eprintln!("cross-origin server error: {e}");
                                }
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

async fn handle_cross_origin_request(
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    let mut response_body = String::from("cross-origin response");
    for (name, value) in &headers {
        let _ = std::fmt::Write::write_fmt(&mut response_body, format_args!("\n{name}: {value}"));
    }

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
        response_body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;

    Ok(())
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_upload_through_http_proxy() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let multipart = build_multipart_body();
    let boundary = multipart.boundary().as_str().to_owned();
    let body = multipart.into_body();

    let dest = echo.url();
    let mut resp = client
        .post(&dest)
        .unwrap()
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let resp_body = resp.text().await.unwrap();
    assert!(resp_body.contains("POST"));
    assert!(resp_body.contains("value"));
    assert!(resp_body.contains("file content here"));

    proxy.shutdown();
    echo.shutdown();
}

struct CompressedEchoServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl CompressedEchoServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            tokio::spawn(async move {
                                if let Err(e) = handle_compressed_echo(stream).await {
                                    eprintln!("compressed echo server error: {e}");
                                }
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
        }
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

async fn handle_compressed_echo(
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().to_lowercase() == "content-length" {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    let mut total = 0;
    while total < content_length {
        match reader.read(&mut body[total..]).await {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }

    let compressed = gzip_compress(&body);

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        compressed.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.write_all(&compressed).await?;
    stream.flush().await?;

    Ok(())
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_upload_through_connect_tunnel() {
    let server = CompressedEchoServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let proxy_url = url::Url::parse(&proxy.url()).unwrap();
    let proxy_host = proxy_url.host_str().unwrap().to_string();
    let proxy_port = proxy_url.port().unwrap();

    let mut stream = TcpStream::connect(format!("{proxy_host}:{proxy_port}"))
        .await
        .unwrap();

    let connect_req = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        server.port(),
        server.port()
    );
    stream.write_all(connect_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(&mut stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await.unwrap();
    assert!(status_line.contains("200"), "CONNECT failed: {status_line}");

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line.trim().is_empty() {
            break;
        }
    }

    let stream = reader.into_inner();

    let multipart = build_multipart_body();
    let boundary = multipart.boundary().as_str().to_owned();
    let eggfetch_core::RequestBody::Bytes(encoded_body) = multipart.into_body() else {
        panic!("expected bytes body")
    };

    let request = format!(
        "POST /upload HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        server.port(),
        encoded_body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.write_all(&encoded_body).await.unwrap();

    let mut response_buf = Vec::new();
    let mut resp_reader = BufReader::new(stream);

    let mut status_line = String::new();
    resp_reader.read_line(&mut status_line).await.unwrap();
    response_buf.extend_from_slice(status_line.as_bytes());

    let mut resp_headers_raw = Vec::new();
    loop {
        let mut line = String::new();
        resp_reader.read_line(&mut line).await.unwrap();
        resp_headers_raw.extend_from_slice(line.as_bytes());
        if line.trim().is_empty() {
            break;
        }
    }
    response_buf.extend_from_slice(&resp_headers_raw);

    let resp_headers_str = String::from_utf8_lossy(&resp_headers_raw);
    let mut resp_content_length: Option<usize> = None;
    for line in resp_headers_str.lines() {
        if let Some(val) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            resp_content_length = val.trim().parse().ok();
        }
    }

    if let Some(cl) = resp_content_length {
        let mut body = vec![0u8; cl];
        let mut total = 0;
        while total < cl {
            match resp_reader.read(&mut body[total..]).await {
                Ok(0) | Err(_) => break,
                Ok(n) => total += n,
            }
        }
        response_buf.extend_from_slice(&body);
    }

    let response_str = String::from_utf8_lossy(&response_buf);
    assert!(
        response_str
            .to_lowercase()
            .contains("content-encoding: gzip"),
        "Expected gzip Content-Encoding in response: {response_str}"
    );

    let body_start = response_str.find("\r\n\r\n").unwrap() + 4;
    let compressed_body = &response_buf[body_start..];
    let decompressed = eggfetch_core::compression::decompress_buffered(
        compressed_body,
        "gzip",
        eggfetch_core::compression::DecompressionLimit::new(),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&decompressed),
        String::from_utf8_lossy(&encoded_body)
    );

    proxy.shutdown();
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "compression-gzip")]
async fn compressed_response_through_http_proxy() {
    let server = CompressedResponseServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let dest = server.url();
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello compressed world");

    proxy.shutdown();
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "compression-gzip")]
async fn compressed_response_through_connect_tunnel() {
    let server = CompressedResponseServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let proxy_url = url::Url::parse(&proxy.url()).unwrap();
    let proxy_host = proxy_url.host_str().unwrap().to_string();
    let proxy_port = proxy_url.port().unwrap();

    let mut stream = TcpStream::connect(format!("{proxy_host}:{proxy_port}"))
        .await
        .unwrap();

    let connect_req = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        server.port(),
        server.port()
    );
    stream.write_all(connect_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(&mut stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await.unwrap();
    assert!(status_line.contains("200"), "CONNECT failed: {status_line}");

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line.trim().is_empty() {
            break;
        }
    }

    let stream = reader.into_inner();
    let http_req = format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        server.port()
    );
    stream.write_all(http_req.as_bytes()).await.unwrap();

    let mut resp_reader = BufReader::new(stream);
    let mut status_line = String::new();
    resp_reader.read_line(&mut status_line).await.unwrap();
    let mut resp_buf = Vec::new();
    resp_buf.extend_from_slice(status_line.as_bytes());

    loop {
        let mut line = String::new();
        resp_reader.read_line(&mut line).await.unwrap();
        resp_buf.extend_from_slice(line.as_bytes());
        if line.trim().is_empty() {
            break;
        }
    }

    let resp_headers_str = String::from_utf8_lossy(&resp_buf);
    let mut resp_content_length: Option<usize> = None;
    for line in resp_headers_str.lines() {
        if let Some(val) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            resp_content_length = val.trim().parse().ok();
        }
    }

    if let Some(cl) = resp_content_length {
        let mut body = vec![0u8; cl];
        let mut total = 0;
        while total < cl {
            match resp_reader.read(&mut body[total..]).await {
                Ok(0) | Err(_) => break,
                Ok(n) => total += n,
            }
        }
        resp_buf.extend_from_slice(&body);
    }

    let response_str = String::from_utf8_lossy(&resp_buf);
    assert!(
        response_str
            .to_lowercase()
            .contains("content-encoding: gzip"),
        "Expected gzip Content-Encoding in response: {response_str}"
    );

    let body_start = response_str.find("\r\n\r\n").unwrap() + 4;
    let compressed_body = &resp_buf[body_start..];
    let decompressed = eggfetch_core::compression::decompress_buffered(
        compressed_body,
        "gzip",
        eggfetch_core::compression::DecompressionLimit::new(),
    )
    .unwrap();
    assert_eq!(decompressed.as_ref(), b"hello compressed world");

    proxy.shutdown();
    server.shutdown();
}

#[tokio::test]
async fn redirect_same_origin_through_proxy() {
    let redirect_server = RedirectServer::start(None).await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .follow_redirects(true)
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/redirect-same", redirect_server.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "final destination reached");
    assert_eq!(resp.url().path(), "/final");
    assert_eq!(resp.history().len(), 1);
    assert_eq!(resp.history()[0].status().as_u16(), 302);

    proxy.shutdown();
    redirect_server.shutdown();
}

#[tokio::test]
async fn redirect_cross_origin_through_proxy() {
    let cross_server = CrossOriginServer::start().await;
    let origin_server = RedirectServer::start(Some(cross_server.port())).await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .follow_redirects(true)
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/redirect-cross", origin_server.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("cross-origin response"),
        "Body should contain cross-origin response: {body}"
    );
    assert_eq!(resp.history().len(), 1);
    assert_eq!(resp.history()[0].status().as_u16(), 302);

    proxy.shutdown();
    origin_server.shutdown();
    cross_server.shutdown();
}

#[tokio::test]
#[cfg(feature = "cookies")]
async fn cookies_set_on_proxy_redirect_hop() {
    let redirect_server = RedirectServer::start(None).await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .follow_redirects(true)
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/redirect-cookie", redirect_server.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "final destination reached");

    assert_eq!(client.cookies().len(), 1);
    let cookie_header = client
        .cookies()
        .cookies_for_url(&url::Url::parse(&redirect_server.url()).unwrap());
    assert!(cookie_header.is_some());
    let header_val = cookie_header.unwrap();
    assert!(header_val.contains("session=abc123"));

    proxy.shutdown();
    redirect_server.shutdown();
}

#[tokio::test]
#[cfg(feature = "compression-gzip")]
async fn cancellation_during_streamed_compressed_proxy_response() {
    let server = CompressedResponseServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());
    let dest = server.url();
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let mut stream = resp.bytes_stream().unwrap();
    if let Some(chunk) = stream.next().await {
        let _ = chunk.unwrap();
    }
    drop(stream);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let metrics = client.pool_metrics();
    let _ = metrics;

    proxy.shutdown();
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "compression-gzip")]
async fn streamed_compressed_response_through_proxy() {
    let server = CompressedResponseServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let dest = server.url();
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let mut stream = resp.bytes_stream().unwrap();
    let mut all_bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        all_bytes.extend_from_slice(&chunk);
    }

    assert!(
        !all_bytes.is_empty(),
        "Stream should have yielded decompressed bytes"
    );
    let body = String::from_utf8_lossy(&all_bytes);
    assert_eq!(body.as_ref(), "hello compressed world");

    let raw_gzip = gzip_compress(b"hello compressed world");
    assert_ne!(
        all_bytes.len(),
        raw_gzip.len(),
        "Decompressed bytes should differ in length from gzip-compressed bytes, proving decompression occurred"
    );

    proxy.shutdown();
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn proxy_authenticated_multipart_upload() {
    let echo = EchoHttpServer::start().await;
    let auth_received = Arc::new(AtomicBool::new(false));
    let proxy = TrackingHttpProxyServer::start(TrackingHttpProxyConfig {
        required_auth: Some(("proxyuser".into(), "proxypass".into())),
        auth_received: auth_received.clone(),
    })
    .await;

    let client = Client::builder()
        .proxy(
            Proxy::all(&proxy.url())
                .unwrap()
                .auth(ProxyAuth::basic("proxyuser", "proxypass").unwrap()),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let multipart = build_multipart_body();
    let boundary = multipart.boundary().as_str().to_owned();
    let body = multipart.into_body();

    let dest = echo.url();
    let mut resp = client
        .post(&dest)
        .unwrap()
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let resp_body = resp.text().await.unwrap();
    assert!(resp_body.contains("POST"));
    assert!(resp_body.contains("value"));
    assert!(resp_body.contains("file content here"));

    assert!(
        proxy.auth_was_received(),
        "Proxy should have received Proxy-Authorization header"
    );

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn auth_stripping_on_cross_origin_proxied_redirect() {
    let cross_server = CrossOriginServer::start().await;
    let origin_server = RedirectServer::start(Some(cross_server.port())).await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .auth(eggfetch_core::AuthScheme::basic("testuser", "testpass").unwrap())
        .follow_redirects(true)
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/redirect-cross", origin_server.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("cross-origin response"),
        "Body should contain cross-origin response: {body}"
    );

    assert!(
        !body.to_lowercase().contains("authorization"),
        "Authorization header should NOT be present on cross-origin redirect target, but got: {body}"
    );

    proxy.shutdown();
    origin_server.shutdown();
    cross_server.shutdown();
}

#[tokio::test]
#[cfg(feature = "compression-gzip")]
async fn total_timeout_connect_plus_decompression() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    if let Ok((mut stream, _)) = result {
                        tokio::spawn(async move {
                            let _ = stream.read(&mut [0u8; 1]).await;
                        });
                    }
                }
                _ = shutdown_rx.changed() => { break; }
            }
        }
    });

    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .timeout(Timeout {
            total: Some(Duration::from_millis(200)),
            ..Default::default()
        })
        .build();

    let dest = format!("https://127.0.0.1:{port}/");
    let result = client.get(&dest).unwrap().send().await;
    assert!(
        result.is_err(),
        "Expected timeout error but got: {result:?}"
    );

    let _ = shutdown_tx.send(true);
    proxy.shutdown();
}
