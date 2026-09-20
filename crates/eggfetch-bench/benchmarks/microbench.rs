//! Microbenchmarks for eggfetch-core internals.

#![allow(missing_docs, clippy::too_many_lines, clippy::large_futures)]

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

    for count in [8_usize, 50, 200] {
        group.bench_function(format!("response_header_clone/{count}"), |b| {
            let mut headers = http::HeaderMap::new();
            for index in 0..count {
                headers.insert(
                    http::header::HeaderName::from_bytes(format!("x-bench-{index}").as_bytes())
                        .unwrap(),
                    HeaderValue::from_static("value"),
                );
            }
            b.iter(|| black_box(headers.clone()));
        });
    }

    group.finish();
}

fn bench_request_ownership(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_ownership");
    for count in [8_usize, 50, 200] {
        let mut headers = Headers::new();
        for index in 0..count {
            headers
                .append(&format!("x-bench-{index}"), "value")
                .unwrap();
        }
        headers.append("x-duplicate", "first").unwrap();
        headers.append("x-duplicate", "second").unwrap();

        group.bench_function(format!("high_level_rebuild/{count}"), |b| {
            b.iter(|| {
                let mut builder = http::Request::builder()
                    .method(http::Method::GET)
                    .uri("http://example.com/")
                    .version(http::Version::HTTP_11);
                for (name, value) in headers.iter() {
                    builder = builder.header(name, value);
                }
                black_box(builder.body(()).unwrap());
            });
        });

        group.bench_function(format!("high_level_owned/{count}"), |b| {
            b.iter_batched(
                || headers.clone(),
                |headers| {
                    let request = http::Request::builder()
                        .method(http::Method::GET)
                        .uri("http://example.com/")
                        .version(http::Version::HTTP_11)
                        .body(())
                        .unwrap();
                    let mut request = request;
                    *request.headers_mut() = headers.into_inner();
                    assert_eq!(request.headers().get_all("x-duplicate").iter().count(), 2);
                    black_box(request);
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_function(format!("native_rebuild/{count}"), |b| {
            b.iter(|| {
                let request = http::Request::builder()
                    .method(http::Method::POST)
                    .uri("http://example.com/upload")
                    .version(http::Version::HTTP_11)
                    .body(bytes::Bytes::from_static(b"body"))
                    .unwrap();
                let mut request = request;
                for (name, value) in headers.iter() {
                    request.headers_mut().append(name, value.clone());
                }
                black_box(request);
            });
        });

        group.bench_function(format!("native_owned/{count}"), |b| {
            b.iter_batched(
                || headers.clone(),
                |headers| {
                    let request = http::Request::builder()
                        .method(http::Method::POST)
                        .uri("http://example.com/upload")
                        .version(http::Version::HTTP_11)
                        .body(bytes::Bytes::from_static(b"body"))
                        .unwrap();
                    let mut request = request;
                    *request.headers_mut() = headers.into_inner();
                    assert_eq!(request.headers().get_all("x-duplicate").iter().count(), 2);
                    black_box(request);
                },
                BatchSize::SmallInput,
            );
        });
    }
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

        for count in [10_usize, 1_000] {
            group.bench_function(format!("lookup_no_expiry/{count}"), |b| {
                let url = url::Url::parse("http://example.com/path").unwrap();
                let jar = CookieJar::new();
                for index in 0..count {
                    jar.set_default_cookie(format!("cookie_{index}"), "value".to_owned())
                        .unwrap();
                }
                b.iter(|| black_box(jar.cookies_for_url(&url)));
            });
        }

        for count in [10_usize, 1_000, 10_000] {
            group.bench_function(format!("mutate_large_jar/{count}"), |b| {
                let url = url::Url::parse("http://example.com/path").unwrap();
                b.iter_batched(
                    || {
                        let jar = CookieJar::new();
                        for index in 0..count {
                            jar.set_default_cookie(format!("cookie_{index}"), "value".to_owned())
                                .unwrap();
                        }
                        jar
                    },
                    |jar| {
                        jar.set_default_cookie("replacement".to_owned(), "value".to_owned())
                            .unwrap();
                        jar.delete("replacement", "", "/");
                        black_box(jar.cookies_for_url(&url));
                    },
                    BatchSize::SmallInput,
                );
            });
        }

        // Isolate the single-cookie mutation paths that the incremental
        // expiry watermark is intended to optimize. Parsing and seed-jar
        // construction are batch setup; the measured operation is one
        // CookieJar mutation. Run the same cases on the pre-watermark
        // revision when evaluating the retain/revert gate.
        for count in [10_usize, 1_000, 10_000] {
            let url = url::Url::parse("http://example.com/path").unwrap();
            let cookie = |name: &str, max_age: Option<u64>| {
                let suffix = max_age
                    .map(|age| format!("; Max-Age={age}"))
                    .unwrap_or_default();
                parse_set_cookie_headers(&url, &[format!("{name}=value; Path=/{suffix}")])
                    .into_iter()
                    .next()
                    .unwrap()
            };
            let seed = |minimum: bool| {
                let jar = CookieJar::new();
                for index in 0..count {
                    jar.set(cookie(&format!("cookie_{index}"), Some(3_600)))
                        .unwrap();
                }
                if minimum {
                    jar.set(cookie("minimum", Some(1))).unwrap();
                }
                jar
            };

            group.bench_function(format!("mutate_single/{count}/insert_later"), |b| {
                b.iter_batched(
                    || seed(false),
                    |jar| {
                        jar.set(cookie("new-later", Some(7_200))).unwrap();
                        black_box(jar.len());
                    },
                    BatchSize::SmallInput,
                );
            });

            group.bench_function(format!("mutate_single/{count}/replace_non_minimum"), |b| {
                b.iter_batched(
                    || seed(false),
                    |jar| {
                        jar.set(cookie("cookie_0", Some(7_200))).unwrap();
                        black_box(jar.len());
                    },
                    BatchSize::SmallInput,
                );
            });

            group.bench_function(format!("mutate_single/{count}/insert_earlier"), |b| {
                b.iter_batched(
                    || seed(false),
                    |jar| {
                        jar.set(cookie("new-earlier", Some(1))).unwrap();
                        black_box(jar.len());
                    },
                    BatchSize::SmallInput,
                );
            });

            group.bench_function(
                format!("mutate_single/{count}/replace_unique_minimum"),
                |b| {
                    b.iter_batched(
                        || seed(true),
                        |jar| {
                            jar.set(cookie("minimum", Some(7_200))).unwrap();
                            black_box(jar.len());
                        },
                        BatchSize::SmallInput,
                    );
                },
            );

            group.bench_function(
                format!("mutate_single/{count}/delete_unique_minimum"),
                |b| {
                    b.iter_batched(
                        || seed(true),
                        |jar| {
                            jar.delete("minimum", "example.com", "/");
                            black_box(jar.len());
                        },
                        BatchSize::SmallInput,
                    );
                },
            );

            group.bench_function(
                format!("mutate_single/{count}/replace_equal_minimum"),
                |b| {
                    b.iter_batched(
                        || {
                            let jar = CookieJar::new();
                            for index in 0..count {
                                jar.set(cookie(&format!("cookie_{index}"), Some(3_600)))
                                    .unwrap();
                            }
                            let equal_headers = [
                                "equal_a=value; Path=/; Expires=Wed, 01 Jan 2030 00:00:00 GMT",
                                "equal_b=value; Path=/; Expires=Wed, 01 Jan 2030 00:00:00 GMT",
                            ]
                            .into_iter()
                            .map(str::to_owned)
                            .collect::<Vec<_>>();
                            for equal in parse_set_cookie_headers(&url, &equal_headers) {
                                jar.set(equal).unwrap();
                            }
                            jar
                        },
                        |jar| {
                            jar.set(cookie("equal_a", Some(7_200))).unwrap();
                            black_box(jar.len());
                        },
                        BatchSize::SmallInput,
                    );
                },
            );

            group.bench_function(format!("mutate_single/{count}/insert_session"), |b| {
                b.iter_batched(
                    || seed(false),
                    |jar| {
                        jar.set(cookie("session", None)).unwrap();
                        black_box(jar.len());
                    },
                    BatchSize::SmallInput,
                );
            });
        }

        // Repeated replacement on a prebuilt jar removes seed construction
        // from the timing and exposes the O(1) versus full-map-scan delta.
        for count in [10_usize, 1_000, 10_000] {
            let url = url::Url::parse("http://example.com/path").unwrap();
            let cookie = |name: &str| {
                parse_set_cookie_headers(&url, &[format!("{name}=value; Path=/; Max-Age=7200")])
                    .into_iter()
                    .next()
                    .unwrap()
            };
            let jar = CookieJar::new();
            for index in 0..count {
                jar.set(cookie(&format!("cookie_{index}"))).unwrap();
            }
            let replacement = cookie("cookie_0");
            group.bench_function(format!("mutate_hot/{count}/replace_non_minimum"), |b| {
                b.iter(|| {
                    jar.set(replacement.clone()).unwrap();
                    black_box(jar.len());
                });
            });
        }

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
    bench_request_ownership,
    bench_cookie_matching,
    bench_auth_application,
    bench_multipart_encoding,
    bench_decompression,
    bench_retry_decision,
    bench_request_building,
);
criterion_main!(benches);
