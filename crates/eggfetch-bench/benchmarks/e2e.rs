#![allow(missing_docs)]

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

criterion_group!(
    benches,
    bench_one_shot,
    bench_warm_client,
    bench_concurrent_requests,
    bench_body_sizes,
    bench_streaming_body,
    bench_large_upload,
    bench_http2_handshake,
);
criterion_main!(benches);
