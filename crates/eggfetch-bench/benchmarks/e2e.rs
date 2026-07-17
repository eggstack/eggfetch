#![allow(warnings)]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use eggfetch_bench::{BenchServer, BenchServerConfig};
use eggfetch_core::{Client, HttpVersionPolicy};
use futures_util::StreamExt;

fn make_client() -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .build()
}

fn bench_one_shot(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    c.bench_function("one_shot_get_1k", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });
    server.shutdown();
}

fn bench_warm_client(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    let mut group = c.benchmark_group("warm_client");
    for n in [1, 5, 10] {
        group.bench_function(format!("sequential_{n}_requests"), |b| {
            b.to_async(&rt).iter(|| {
                let url = url.clone();
                async move {
                    let client = make_client();
                    for _ in 0..n {
                        let mut resp = client.get(&url).unwrap().send().await.unwrap();
                        let _ = resp.bytes().await;
                    }
                }
            });
        });
    }
    group.finish();
    server.shutdown();
}

fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    c.bench_function("concurrent_10_get", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let futs: Vec<_> = (0..10)
                    .map(|_| {
                        let url = url.clone();
                        async move {
                            let client = make_client();
                            let mut resp = client.get(&url).unwrap().send().await.unwrap();
                            let _ = resp.bytes().await;
                        }
                    })
                    .collect();
                futures_util::future::join_all(futs).await;
            }
        });
    });
    server.shutdown();
}

fn bench_body_sizes(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("body_sizes");
    for &(label, size) in &[
        ("256b", 256),
        ("1k", 1024),
        ("64k", 65536),
        ("1m", 1_048_576),
    ] {
        let server = BenchServer::start(BenchServerConfig {
            body_size: size,
            ..Default::default()
        });
        let url = server.url();

        group.throughput(criterion::Throughput::Bytes(size as u64));
        group.bench_function(label, |b| {
            b.to_async(&rt).iter(|| {
                let url = url.clone();
                async move {
                    let client = make_client();
                    let mut resp = client.get(&url).unwrap().send().await.unwrap();
                    let _ = resp.bytes().await;
                }
            });
        });
        server.shutdown();
    }
    group.finish();
}

fn bench_streaming_body(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 65536,
        chunk_size: 4096,
        ..Default::default()
    });
    let url = server.url();

    let mut group = c.benchmark_group("streaming_body");

    group.bench_function("time_to_first_byte", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let mut stream = resp.bytes_stream().unwrap();
                let _first = stream.next().await;
            }
        });
    });

    group.bench_function("full_body_stream", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let mut stream = resp.bytes_stream().unwrap();
                while let Some(chunk) = stream.next().await {
                    let _ = chunk.unwrap();
                }
            }
        });
    });

    group.finish();
    server.shutdown();
}

fn bench_large_upload(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 0,
        consume_request_body: true,
        ..Default::default()
    });
    let url = server.url();
    let body = vec![b'x'; 256 * 1024];

    c.bench_function("upload_256k", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            let body = body.clone();
            async move {
                let client = make_client();
                let mut resp = client.post(&url).unwrap().body(body).send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });
    server.shutdown();
}

fn bench_http2_handshake(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("http_version");

    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    group.bench_function("http1_only", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = Client::builder()
                    .http_version_policy(eggfetch_core::HttpVersionPolicy::Http1Only)
                    .build();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });

    group.bench_function("auto_negotiate", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = Client::builder()
                    .http_version_policy(eggfetch_core::HttpVersionPolicy::Auto {
                        allow_http3: false,
                    })
                    .build();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });

    group.finish();
    server.shutdown();
}

/// Minimal HTTP proxy that supports CONNECT tunneling.
#[allow(dead_code)]
struct BenchProxy {
    port: u16,
    connections_served: Arc<AtomicUsize>,
}

impl BenchProxy {
    /// Start a proxy on a random available port.
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(false).unwrap();

        let connections_served = Arc::new(AtomicUsize::new(0));
        let cs = connections_served.clone();

        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        cs.fetch_add(1, Ordering::Relaxed);
                        thread::spawn(move || Self::handle_proxy_connection(stream));
                    }
                    Err(_) => break,
                }
            }
        });

        let proxy = Self {
            port,
            connections_served,
        };
        // Wait for the proxy thread to start accepting connections.
        for _ in 0..200 {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        proxy
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    #[allow(dead_code)]
    fn connections_served(&self) -> usize {
        self.connections_served.load(Ordering::Relaxed)
    }

    fn shutdown(&self) {}

    fn handle_proxy_connection(mut client_stream: TcpStream) {
        client_stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .ok();
        client_stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .ok();

        let mut reader = BufReader::new(client_stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).ok();
        let request_line = request_line.trim().to_string();

        // Read headers, looking for Host and collecting until blank line.
        let mut host: Option<String> = None;
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) if line.trim().is_empty() => break,
                Ok(_) => {
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("host:") {
                        host = Some(line[5..].trim().to_string());
                    }
                }
            }
        }

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 3 {
            let _ = client_stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            return;
        }

        let method = parts[0];
        let target = parts[1];

        if method.eq_ignore_ascii_case("CONNECT") {
            // CONNECT host:port -> tunnel to target
            let target_addr = if let Some(h) = host {
                h
            } else {
                target.to_string()
            };
            let target_addr = if !target_addr.contains(':') {
                format!("{target_addr}:443")
            } else {
                target_addr
            };

            match TcpStream::connect(&target_addr) {
                Ok(target_stream) => {
                    let _ = client_stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
                    target_stream
                        .set_read_timeout(Some(Duration::from_secs(30)))
                        .ok();
                    target_stream
                        .set_write_timeout(Some(Duration::from_secs(30)))
                        .ok();

                    // Bidirectional copy: client <-> target
                    // Each thread needs its own clone of both streams.
                    let cs_to_target = client_stream.try_clone().unwrap();
                    let cs_to_client = client_stream.try_clone().unwrap();
                    let ts_to_client = target_stream.try_clone().unwrap();
                    let ts_to_target = target_stream.try_clone().unwrap();
                    let t1 = thread::spawn(move || {
                        let mut cs = cs_to_target;
                        let mut ts = ts_to_client;
                        let _ = std::io::copy(&mut cs, &mut ts);
                        let _ = ts.shutdown(std::net::Shutdown::Write);
                    });
                    let t2 = thread::spawn(move || {
                        let mut ts = ts_to_target;
                        let mut cs = cs_to_client;
                        let _ = std::io::copy(&mut ts, &mut cs);
                        let _ = cs.shutdown(std::net::Shutdown::Write);
                    });
                    t1.join().ok();
                    t2.join().ok();
                    let _ = client_stream.shutdown(std::net::Shutdown::Both);
                }
                Err(_) => {
                    let _ = client_stream
                        .write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n");
                }
            }
        } else {
            // Forward HTTP requests to the target directly
            let target_url = if target.starts_with("http://") {
                target.to_string()
            } else if let Some(h) = host {
                format!("http://{h}{target}")
            } else {
                format!("http://127.0.0.1:{target}")
            };

            if let Ok(mut target_stream) = TcpStream::connect(
                target_url
                    .strip_prefix("http://")
                    .unwrap_or(&target_url)
                    .split('/')
                    .next()
                    .unwrap_or("127.0.0.1:80"),
            ) {
                // Reconstruct and forward the request
                let new_request = format!("{} {} HTTP/1.1\r\n", method, target);
                let _ = target_stream.write_all(new_request.as_bytes());
                // Forward remaining headers
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if line.trim().is_empty() => {
                            let _ = target_stream.write_all(b"\r\n");
                            break;
                        }
                        Ok(_) => {
                            let lower = line.to_ascii_lowercase();
                            if !lower.starts_with("proxy-") {
                                let _ = target_stream.write_all(line.as_bytes());
                            }
                        }
                    }
                }
                // Pipe target response back to client
                let _ = std::io::copy(&mut target_stream, &mut client_stream);
            } else {
                let _ = client_stream
                    .write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n");
            }
        }
    }
}

fn bench_proxy_direct_comparison(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let proxy = BenchProxy::start();
    let target_url = server.url();
    let proxy_url = proxy.url();

    let mut group = c.benchmark_group("proxy_overhead");

    // Direct request (no proxy)
    group.bench_function("direct_get_1k", |b| {
        let url = target_url.clone();
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });

    // Request through HTTP proxy (CONNECT tunnel to an HTTP server)
    group.bench_function("proxied_get_1k", |b| {
        let url = target_url.clone();
        let proxy_url = proxy_url.clone();
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            let proxy_url = proxy_url.clone();
            async move {
                let proxy = eggfetch_core::Proxy::http(&proxy_url).unwrap();
                let client = Client::builder()
                    .http_version_policy(HttpVersionPolicy::Http1Only)
                    .proxy(proxy)
                    .build();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });

    group.finish();
    server.shutdown();
    proxy.shutdown();
}

criterion_group!(
    benches,
    bench_one_shot,
    bench_warm_client,
    bench_concurrent_requests,
    bench_body_sizes,
    bench_streaming_body,
    bench_large_upload,
    bench_http2_handshake,
    bench_proxy_direct_comparison,
);
criterion_main!(benches);
