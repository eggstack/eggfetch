#![allow(missing_docs, clippy::needless_return)]

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use criterion::{criterion_group, criterion_main, Criterion};
use eggfetch_bench::{BenchServer, BenchServerConfig};
use eggfetch_core::{Client, HttpVersionPolicy};
use futures_util::StreamExt;

fn make_client() -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .build()
}

fn bench_buffered_vs_streaming(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024 * 1024,
        ..Default::default()
    });
    let url = server.url();

    let mut group = c.benchmark_group("buffered_vs_streaming");

    group.bench_function("buffered_1mb", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            }
        });
    });

    group.bench_function("streaming_1mb", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let mut stream = resp.bytes_stream().unwrap();
                while let Some(result) = stream.next().await {
                    let _ = result.unwrap();
                }
            }
        });
    });

    group.finish();
    server.shutdown();
}

fn bench_long_lived_client(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    c.bench_function("long_lived_client_100_requests", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                for _ in 0..100 {
                    let mut resp = client.get(&url).unwrap().send().await.unwrap();
                    let _ = resp.bytes().await;
                }
            }
        });
    });

    server.shutdown();
}

fn bench_connection_pool_saturation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 1024,
        ..Default::default()
    });
    let url = server.url();

    c.bench_function("pool_saturation_20_concurrent", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let mut handles = Vec::with_capacity(20);
                for _ in 0..20 {
                    let client = client.clone();
                    let url = url.clone();
                    handles.push(tokio::spawn(async move {
                        let mut resp = client.get(&url).unwrap().send().await.unwrap();
                        let _ = resp.bytes().await;
                    }));
                }
                for handle in handles {
                    handle.await.unwrap();
                }
            }
        });
    });

    server.shutdown();
}

fn make_many_headers_server(num_headers: usize) -> (String, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(false).unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .ok();
                    stream
                        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
                        .ok();

                    thread::spawn(move || {
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut line = String::new();
                        loop {
                            line.clear();
                            match reader.read_line(&mut line) {
                                Ok(0) | Err(_) => return,
                                Ok(_) if line.trim().is_empty() => break,
                                Ok(_) => {}
                            }
                        }

                        let mut response = String::from("HTTP/1.1 200 OK\r\n");
                        for i in 0..num_headers {
                            let _ = write!(response, "X-Custom-{i}: value-{i}\r\n");
                        }
                        response.push_str("Content-Length: 0\r\nConnection: close\r\n\r\n");
                        let _ = stream.write_all(response.as_bytes());
                    });
                }
                Err(_) => break,
            }
        }
    });

    // Wait for the server to be ready.
    for _ in 0..200 {
        if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    (format!("http://127.0.0.1:{port}"), shutdown)
}

fn bench_header_large_set(c: &mut Criterion) {
    let (url, shutdown) = make_many_headers_server(50);
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("parse_50_response_headers", |b| {
        b.to_async(&rt).iter(|| {
            let url = url.clone();
            async move {
                let client = make_client();
                let resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.headers().len();
            }
        });
    });

    shutdown.store(true, Ordering::Relaxed);
}

fn bench_redirect_chain(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = BenchServer::start(BenchServerConfig {
        body_size: 256,
        ..Default::default()
    });
    let url = server.url();

    c.bench_function("simple_request_no_redirect", |b| {
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

criterion_group!(
    benches,
    bench_buffered_vs_streaming,
    bench_long_lived_client,
    bench_connection_pool_saturation,
    bench_header_large_set,
    bench_redirect_chain,
);
criterion_main!(benches);
