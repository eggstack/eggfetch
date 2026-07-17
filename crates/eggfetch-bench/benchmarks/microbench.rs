//! Microbenchmarks for eggfetch-core internals.

#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use eggfetch_core::{BasicAuth, BearerAuth, Client, Headers, RetryPolicy};
use http::header::HeaderValue;
use http::Method;

fn bench_url_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("url_construction");

    group.bench_function("parse_url", |b| {
        b.iter(|| {
            black_box(url::Url::parse("https://example.com/path?query=value&foo=bar").unwrap())
        });
    });

    group.bench_function("build_with_query", |b| {
        let client = Client::new();
        b.iter(|| {
            black_box(
                client
                    .get("https://example.com/path")
                    .unwrap()
                    .query("query", "value")
                    .query("foo", "bar")
                    .build()
                    .unwrap(),
            );
        });
    });

    group.finish();
}

fn bench_header_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("header_operations");

    group.bench_function("insert_http_headermap", |b| {
        b.iter_batched(
            http::HeaderMap::new,
            |mut map| {
                map.insert("content-type", HeaderValue::from_static("application/json"));
                map.insert("accept", HeaderValue::from_static("text/html"));
                map.insert("authorization", HeaderValue::from_static("Bearer token123"));
                map.insert("user-agent", HeaderValue::from_static("eggfetch/0.1"));
                map.insert("accept-encoding", HeaderValue::from_static("gzip, br"));
                map.insert("cache-control", HeaderValue::from_static("no-cache"));
                map.insert("connection", HeaderValue::from_static("keep-alive"));
                map.insert("host", HeaderValue::from_static("example.com"));
                map.insert("referer", HeaderValue::from_static("https://example.com"));
                map.insert("x-request-id", HeaderValue::from_static("abc-123"));
                black_box(map);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("insert_eggfetch_headers", |b| {
        b.iter_batched(
            Headers::new,
            |mut headers| {
                let _ = headers.insert("content-type", "application/json");
                let _ = headers.insert("accept", "text/html");
                let _ = headers.insert("authorization", "Bearer token123");
                let _ = headers.insert("user-agent", "eggfetch/0.1");
                let _ = headers.insert("accept-encoding", "gzip, br");
                let _ = headers.insert("cache-control", "no-cache");
                let _ = headers.insert("connection", "keep-alive");
                let _ = headers.insert("host", "example.com");
                let _ = headers.insert("referer", "https://example.com");
                let _ = headers.insert("x-request-id", "abc-123");
                black_box(headers);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("lookup_header", |b| {
        let mut headers = Headers::new();
        headers.insert("x-custom-header", "custom-value").unwrap();
        b.iter(|| {
            black_box(headers.get("x-custom-header"));
        });
    });

    group.finish();
}

fn bench_cookie_matching(c: &mut Criterion) {
    #[cfg(feature = "cookies")]
    {
        use eggfetch_core::cookie::{parse_set_cookie_headers, CookieJar};

        let mut group = c.benchmark_group("cookie_matching");

        group.bench_function("set_cookie", |b| {
            let url = url::Url::parse("http://example.com/path").unwrap();
            b.iter_batched(
                CookieJar::new,
                |jar| {
                    let headers = vec!["session=abc123".to_owned()];
                    jar.update_from_response(&url, &headers);
                    black_box(&jar);
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_function("lookup_cookies", |b| {
            let url = url::Url::parse("http://example.com/path").unwrap();
            let jar = CookieJar::new();
            let headers = vec![
                "session=abc123".to_owned(),
                "user=john".to_owned(),
                "theme=dark".to_owned(),
            ];
            jar.update_from_response(&url, &headers);
            b.iter(|| {
                black_box(jar.cookies_for_url(&url));
            });
        });

        group.bench_function("parse_set_cookie", |b| {
            let url = url::Url::parse("http://example.com/path").unwrap();
            let header_values = vec!["session=abc123; Path=/; HttpOnly".to_owned()];
            b.iter(|| {
                black_box(parse_set_cookie_headers(&url, &header_values));
            });
        });

        group.finish();
    }
}

fn bench_auth_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("auth_application");

    group.bench_function("basic_auth_construction", |b| {
        b.iter(|| {
            black_box(BasicAuth::new("user", "password").unwrap());
        });
    });

    group.bench_function("bearer_auth_construction", |b| {
        b.iter(|| {
            black_box(BearerAuth::new("my-secret-token-12345").unwrap());
        });
    });

    group.finish();
}

fn bench_multipart_encoding(c: &mut Criterion) {
    #[cfg(feature = "multipart")]
    {
        use bytes::Bytes;
        use eggfetch_core::Multipart;

        let mut group = c.benchmark_group("multipart_encoding");

        group.bench_function("build_multipart", |b| {
            b.iter(|| {
                let mp = Multipart::new()
                    .text("field1", "value1")
                    .unwrap()
                    .text("field2", "value2")
                    .unwrap()
                    .text("field3", "value3")
                    .unwrap()
                    .text("field4", "value4")
                    .unwrap()
                    .text("field5", "value5")
                    .unwrap()
                    .bytes(
                        "file1",
                        "file1.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 1024]),
                    )
                    .unwrap()
                    .bytes(
                        "file2",
                        "file2.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 2048]),
                    )
                    .unwrap()
                    .bytes(
                        "file3",
                        "file3.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 4096]),
                    )
                    .unwrap();
                black_box(mp);
            });
        });

        group.bench_function("content_length", |b| {
            let mp = Multipart::new()
                .text("field1", "value1")
                .unwrap()
                .text("field2", "value2")
                .unwrap()
                .text("field3", "value3")
                .unwrap()
                .text("field4", "value4")
                .unwrap()
                .text("field5", "value5")
                .unwrap()
                .bytes(
                    "file1",
                    "file1.bin",
                    "application/octet-stream",
                    Bytes::from(vec![0u8; 1024]),
                )
                .unwrap()
                .bytes(
                    "file2",
                    "file2.bin",
                    "application/octet-stream",
                    Bytes::from(vec![0u8; 2048]),
                )
                .unwrap()
                .bytes(
                    "file3",
                    "file3.bin",
                    "application/octet-stream",
                    Bytes::from(vec![0u8; 4096]),
                )
                .unwrap();
            b.iter(|| {
                black_box(mp.content_length());
            });
        });

        group.bench_function("encode_body", |b| {
            b.iter(|| {
                let mp = Multipart::new()
                    .text("field1", "value1")
                    .unwrap()
                    .text("field2", "value2")
                    .unwrap()
                    .text("field3", "value3")
                    .unwrap()
                    .text("field4", "value4")
                    .unwrap()
                    .text("field5", "value5")
                    .unwrap()
                    .bytes(
                        "file1",
                        "file1.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 1024]),
                    )
                    .unwrap()
                    .bytes(
                        "file2",
                        "file2.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 2048]),
                    )
                    .unwrap()
                    .bytes(
                        "file3",
                        "file3.bin",
                        "application/octet-stream",
                        Bytes::from(vec![0u8; 4096]),
                    )
                    .unwrap();
                black_box(mp.into_body());
            });
        });

        group.finish();
    }
}

fn bench_decompression(c: &mut Criterion) {
    #[cfg(feature = "compression-gzip")]
    {
        use eggfetch_core::compression::{decompress_buffered, DecompressionLimit};
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut group = c.benchmark_group("decompression");

        let original: Vec<u8> = (0_usize..1_048_576)
            .map(|i| u8::try_from(i % 256).expect("mod 256 fits in u8"))
            .collect();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();

        group.throughput(criterion::Throughput::Bytes(compressed.len() as u64));
        group.bench_function("gzip_decompress_1mb", |b| {
            b.iter(|| {
                let result =
                    decompress_buffered(black_box(&compressed), "gzip", DecompressionLimit::new())
                        .unwrap();
                black_box(result);
            });
        });

        group.finish();
    }
}

fn bench_retry_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("retry_decision");

    group.bench_function("create_policy", |b| {
        b.iter(|| {
            black_box(
                RetryPolicy::builder()
                    .max_attempts(3)
                    .backoff_factor(0.5)
                    .retry_status(429)
                    .retry_status(503)
                    .build(),
            );
        });
    });

    let policy = RetryPolicy::builder()
        .max_attempts(5)
        .backoff_factor(0.5)
        .build();

    group.bench_function("check_method_retryable", |b| {
        b.iter(|| {
            black_box(policy.is_method_retryable(black_box(&Method::GET)));
        });
    });

    group.bench_function("check_status_retryable", |b| {
        b.iter(|| {
            black_box(policy.is_status_retryable(black_box(503)));
        });
    });

    group.bench_function("compute_backoff", |b| {
        b.iter_batched(
            || {
                RetryPolicy::builder()
                    .max_attempts(10)
                    .backoff_factor(0.5)
                    .build()
            },
            |policy| {
                for attempt in 1..=10 {
                    black_box(policy.backoff_delay(attempt));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_request_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_building");

    group.bench_function("build_get_request", |b| {
        let client = Client::new();
        b.iter(|| {
            black_box(
                client
                    .get("https://example.com/api/resource")
                    .unwrap()
                    .header("accept", "application/json")
                    .header("user-agent", "eggfetch-bench")
                    .query("page", "1")
                    .query("limit", "100")
                    .build()
                    .unwrap(),
            );
        });
    });

    group.bench_function("build_post_json_request", |b| {
        let client = Client::new();
        let json_body = br#"{"name":"test","value":"benchmark"}"#;
        b.iter(|| {
            black_box(
                client
                    .post("https://example.com/api/data")
                    .unwrap()
                    .header("content-type", "application/json")
                    .header("accept", "application/json")
                    .bytes(bytes::Bytes::from_static(json_body))
                    .build()
                    .unwrap(),
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_url_construction,
    bench_header_operations,
    bench_cookie_matching,
    bench_auth_application,
    bench_multipart_encoding,
    bench_decompression,
    bench_retry_decision,
    bench_request_building,
);
criterion_main!(benches);
