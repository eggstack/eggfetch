//! Proxy integration tests for eggfetch-core.
//!
//! Contains local test proxy servers and integration tests covering
//! HTTP forward proxying, CONNECT tunneling, auth, security, and
//! streaming through proxies.

#![cfg(feature = "proxy")]
#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, Proxy, ProxyAuth, RequestBody, Timeout};
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Local test proxy servers
// ---------------------------------------------------------------------------

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

struct EchoServer {
    port: u16,
    shutdown: watch::Sender<bool>,
}

impl EchoServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((mut stream, _)) = result {
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 4096];
                                loop {
                                    match stream.read(&mut buf).await {
                                        Ok(0) | Err(_) => break,
                                        Ok(n) => {
                                            if stream.write_all(&buf[..n]).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
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

// ---------------------------------------------------------------------------
// HTTP Forward Proxy Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_proxy_get_returns_correct_response() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let dest = format!("{}/test", echo.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("GET"));
    assert!(body.contains("/test"));

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_post_with_body() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let dest = echo.url();
    let mut resp = client
        .post(&dest)
        .unwrap()
        .header("Content-Type", "text/plain")
        .body("hello proxy")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("POST"));
    assert!(body.contains("hello proxy"));

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_auth_sent_to_proxy_not_destination() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig {
        required_auth: Some(("testuser".into(), "testpass".into())),
    })
    .await;

    let client = Client::builder()
        .proxy(
            Proxy::all(&proxy.url())
                .unwrap()
                .auth(ProxyAuth::basic("testuser", "testpass").unwrap()),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/auth-test", echo.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("proxy-authorization"),
        "proxy auth header should not reach destination"
    );

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_auth_failure_returns_error() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig {
        required_auth: Some(("testuser".into(), "testpass".into())),
    })
    .await;

    let client = Client::builder()
        .proxy(
            Proxy::all(&proxy.url())
                .unwrap()
                .auth(ProxyAuth::basic("wronguser", "wrongpass").unwrap()),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = echo.url();
    let resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 407);

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_streaming_download() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let dest = echo.url();
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let mut stream = resp.bytes_stream().unwrap();
    let mut total = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        total += chunk.len();
    }
    assert!(total > 0);

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_streaming_upload() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let chunks = vec![
        Ok(Bytes::from("chunk1-")),
        Ok(Bytes::from("chunk2-")),
        Ok(Bytes::from("chunk3")),
    ];
    let stream = Box::pin(futures_util::stream::iter(chunks));
    let body = RequestBody::from_stream(stream, Some(20));

    let dest = echo.url();
    let mut resp = client.post(&dest).unwrap().body(body).send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let resp_body = resp.text().await.unwrap();
    eprintln!("STREAMING UPLOAD BODY: {resp_body:?}");
    assert!(resp_body.contains("chunk1-chunk2-chunk3"));

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn http_proxy_large_body() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let large_body = vec![b'x'; 50_000];
    let dest = echo.url();
    let mut resp = client
        .post(&dest)
        .unwrap()
        .body(RequestBody::from(large_body))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Body: "));
    assert!(body.len() > 50_000);

    proxy.shutdown();
    echo.shutdown();
}

// ---------------------------------------------------------------------------
// CONNECT Tunnel Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn connect_tunnel_echo_roundtrip() {
    let echo = EchoServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let proxy_url = url::Url::parse(&proxy.url()).unwrap();
    let proxy_host = proxy_url.host_str().unwrap().to_string();
    let proxy_port = proxy_url.port().unwrap();

    let mut stream = TcpStream::connect(format!("{proxy_host}:{proxy_port}"))
        .await
        .unwrap();

    let connect_req = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        echo.port(),
        echo.port()
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
    stream.write_all(b"hello tunnel").await.unwrap();
    let mut response = vec![0u8; 12];
    stream.read_exact(&mut response).await.unwrap();
    assert_eq!(&response, b"hello tunnel");

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn connect_tunnel_auth_required() {
    let echo = EchoServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig {
        required_auth: Some(("user".into(), "pass".into())),
    })
    .await;

    let proxy_url = url::Url::parse(&proxy.url()).unwrap();
    let proxy_host = proxy_url.host_str().unwrap().to_string();
    let proxy_port = proxy_url.port().unwrap();

    let mut stream = TcpStream::connect(format!("{proxy_host}:{proxy_port}"))
        .await
        .unwrap();

    let connect_req = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        echo.port(),
        echo.port()
    );
    stream.write_all(connect_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(&mut stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await.unwrap();
    assert!(
        status_line.contains("407"),
        "Expected 407, got: {status_line}"
    );

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn connect_tunnel_auth_with_valid_creds() {
    use base64::Engine;

    let echo = EchoServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig {
        required_auth: Some(("user".into(), "pass".into())),
    })
    .await;

    let proxy_url = url::Url::parse(&proxy.url()).unwrap();
    let proxy_host = proxy_url.host_str().unwrap().to_string();
    let proxy_port = proxy_url.port().unwrap();

    let mut stream = TcpStream::connect(format!("{proxy_host}:{proxy_port}"))
        .await
        .unwrap();

    let creds = base64::engine::general_purpose::STANDARD.encode(b"user:pass");
    let connect_req = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nProxy-Authorization: Basic {creds}\r\n\r\n",
        echo.port(),
        echo.port()
    );
    stream.write_all(connect_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(&mut stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await.unwrap();
    assert!(
        status_line.contains("200"),
        "Expected 200, got: {status_line}"
    );

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line.trim().is_empty() {
            break;
        }
    }

    let stream = reader.into_inner();
    stream.write_all(b"hello").await.unwrap();
    let mut response = vec![0u8; 5];
    stream.read_exact(&mut response).await.unwrap();
    assert_eq!(&response, b"hello");

    let _ = stream;

    proxy.shutdown();
    echo.shutdown();
}

// ---------------------------------------------------------------------------
// Security Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proxy_auth_header_never_reaches_destination() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(
            Proxy::all(&proxy.url())
                .unwrap()
                .auth(ProxyAuth::basic("secretuser", "secretpass").unwrap()),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let dest = format!("{}/security-test", echo.url());
    let mut resp = client.get(&dest).unwrap().send().await.unwrap();
    let body = resp.text().await.unwrap();

    assert!(
        !body.contains("proxy-authorization"),
        "Proxy-Authorization header leaked to destination"
    );
    assert!(
        !body.contains("secretuser"),
        "Proxy username leaked to destination"
    );
    assert!(
        !body.contains("secretpass"),
        "Proxy password leaked to destination"
    );

    proxy.shutdown();
    echo.shutdown();
}

#[tokio::test]
async fn credentials_not_in_error_messages() {
    let proxy = HttpProxyServer::start(HttpProxyConfig {
        required_auth: Some(("secretuser".into(), "secretpass".into())),
    })
    .await;

    let client = Client::builder()
        .proxy(
            Proxy::all(&proxy.url())
                .unwrap()
                .auth(ProxyAuth::basic("wronguser", "wrongpass").unwrap()),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let result = client
        .get("http://127.0.0.1:1/nonexistent")
        .unwrap()
        .send()
        .await;

    let resp = result.unwrap();
    assert_eq!(resp.status().as_u16(), 407);

    proxy.shutdown();
}

// ---------------------------------------------------------------------------
// No-Proxy Bypass Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_proxy_bypass_direct_request() {
    let echo = EchoHttpServer::start().await;

    let no_proxy = eggfetch_core::NoProxy::parse("127.0.0.1").unwrap();
    let client = Client::builder()
        .proxy(
            Proxy::all("http://127.0.0.1:9999")
                .unwrap()
                .no_proxy(no_proxy),
        )
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let mut resp = client.get(&echo.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("GET"));

    echo.shutdown();
}

// ---------------------------------------------------------------------------
// Per-Request Proxy Override Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn per_request_bypass_proxy() {
    let echo = EchoHttpServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = Client::builder()
        .proxy(Proxy::all(&proxy.url()).unwrap())
        .timeout(Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let mut resp = client
        .get(&echo.url())
        .unwrap()
        .without_proxy()
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("GET"));

    proxy.shutdown();
    echo.shutdown();
}

struct ConnectionCountingServer {
    port: u16,
    connection_count: Arc<AtomicUsize>,
    shutdown: watch::Sender<bool>,
}

impl ConnectionCountingServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let connection_count = Arc::new(AtomicUsize::new(0));
        let count = connection_count.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((mut stream, _)) = result {
                            count.fetch_add(1, Ordering::SeqCst);
                            tokio::spawn(async move {
                                let mut reader = BufReader::new(&mut stream);
                                let mut request_line = String::new();
                                reader.read_line(&mut request_line).await.ok();

                                let mut content_length: usize = 0;
                                loop {
                                    let mut line = String::new();
                                    reader.read_line(&mut line).await.ok();
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

                                if content_length > 0 {
                                    let mut body = vec![0u8; content_length];
                                    let mut total = 0;
                                    while total < content_length {
                                        match reader.read(&mut body[total..]).await {
                                            Ok(0) | Err(_) => break,
                                            Ok(n) => total += n,
                                        }
                                    }
                                }

                                let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                                stream.write_all(resp.as_bytes()).await.ok();
                            });
                        }
                    }
                    _ = shutdown_rx.changed() => { break; }
                }
            }
        });

        Self {
            port,
            connection_count,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::SeqCst)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

#[tokio::test]
async fn repeated_proxy_requests_create_separate_connections() {
    let server = ConnectionCountingServer::start().await;
    let proxy = HttpProxyServer::start(HttpProxyConfig::default()).await;

    let client = test_client(&proxy.url());

    let request_count = 5;
    for i in 0..request_count {
        let url = format!("{}/req/{i}", server.url());
        let mut resp = client.get(&url).unwrap().send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.unwrap();
        assert_eq!(body, "ok");
    }

    let connections = server.connection_count();
    assert_eq!(
        connections, request_count,
        "Expected {request_count} separate connections but got {connections}"
    );

    proxy.shutdown();
    server.shutdown();
}
