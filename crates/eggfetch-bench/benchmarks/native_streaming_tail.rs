#![allow(
    missing_docs,
    clippy::cast_precision_loss,
    clippy::large_futures,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unwrap_used
)]

//! Manual native streaming tail investigation. Emits one JSON object per run.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use bytes::Bytes;
use eggfetch_core::{Client, HttpVersionPolicy, NativeRequestOptions};
use http::{Request, Response};
use http_body::{Body, Frame, SizeHint};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;

const RESPONSE: &[u8] = b"deterministic response";

#[derive(Clone, Copy)]
struct Geometry {
    label: &'static str,
    chunk: usize,
    count: usize,
}

impl Geometry {
    fn bytes(self) -> usize {
        self.chunk * self.count
    }
}

struct Frames {
    remaining: usize,
    queue: VecDeque<Bytes>,
    delay: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl Frames {
    fn new(geometry: Geometry, slow: bool) -> Self {
        let chunk = Bytes::from(vec![b'x'; geometry.chunk]);
        Self {
            remaining: geometry.bytes(),
            queue: (0..geometry.count).map(|_| chunk.clone()).collect(),
            delay: slow.then(|| Box::pin(tokio::time::sleep(Duration::from_millis(1)))),
        }
    }
}

impl Body for Frames {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        if !self.queue.is_empty() {
            if let Some(delay) = &mut self.delay {
                match delay.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        delay
                            .as_mut()
                            .reset(tokio::time::Instant::now() + Duration::from_millis(1));
                    }
                }
            }
        }
        let Some(bytes) = self.queue.pop_front() else {
            return Poll::Ready(None);
        };
        self.remaining = self.remaining.saturating_sub(bytes.len());
        Poll::Ready(Some(Ok(Frame::data(bytes))))
    }

    fn is_end_stream(&self) -> bool {
        self.queue.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.remaining as u64)
    }
}

#[derive(Default)]
struct ServerStats {
    connections: AtomicUsize,
    requests: AtomicUsize,
}

async fn start_server(h2: bool) -> (String, Arc<ServerStats>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stats = Arc::new(ServerStats::default());
    let server_stats = stats.clone();
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            server_stats.connections.fetch_add(1, Ordering::Relaxed);
            let request_stats = server_stats.clone();
            tokio::spawn(async move {
                let service = service_fn(move |request: Request<Incoming>| {
                    request_stats.requests.fetch_add(1, Ordering::Relaxed);
                    async move {
                        let _ = request.into_body().collect().await;
                        Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(RESPONSE))))
                    }
                });
                let io = TokioIo::new(stream);
                if h2 {
                    let builder = hyper::server::conn::http2::Builder::new(TokioExecutor::new());
                    let _ = builder.serve_connection(io, service).await;
                } else {
                    let mut builder = hyper::server::conn::http1::Builder::new();
                    builder.keep_alive(true);
                    let _ = builder.serve_connection(io, service).await;
                }
            });
        }
    });
    (format!("http://{address}/native-stream"), stats, task)
}

#[derive(Clone, Copy)]
enum Case {
    Direct,
    Default,
    GlobalAbove,
    OriginAbove,
    OriginExact,
    Constrained,
    HighLevel,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct_hyper",
            Self::Default => "native_default",
            Self::GlobalAbove => "native_global_above",
            Self::OriginAbove => "native_origin_above",
            Self::OriginExact => "native_origin_exact",
            Self::Constrained => "native_constrained",
            Self::HighLevel => "high_level_secondary",
        }
    }
}

struct Clients {
    direct: HyperClient<HttpConnector, Frames>,
    direct_h2: HyperClient<HttpConnector, Frames>,
    default: Client,
    global_above: Client,
    origin_above: Client,
    origin_exact: Client,
    constrained: Client,
    high_level: Client,
}

fn make_clients(concurrency: usize, h2: bool) -> Clients {
    let policy = if h2 {
        HttpVersionPolicy::Http2Only
    } else {
        HttpVersionPolicy::Http1Only
    };
    let make_native = |limit: Option<(bool, usize)>| {
        let mut builder = Client::builder().http_version_policy(policy);
        if let Some((global, limit)) = limit {
            builder = if global {
                builder.max_in_flight_requests(limit)
            } else {
                builder.max_in_flight_requests_per_origin(limit)
            };
        }
        builder.build()
    };
    let connector = || {
        let mut connector = HttpConnector::new();
        connector.enforce_http(true);
        connector
    };
    Clients {
        direct: HyperClient::builder(TokioExecutor::new())
            .retry_canceled_requests(true)
            .build(connector()),
        direct_h2: HyperClient::builder(TokioExecutor::new())
            .retry_canceled_requests(true)
            .http2_only(true)
            .build(connector()),
        default: make_native(None),
        global_above: make_native(Some((true, concurrency + 8))),
        origin_above: make_native(Some((false, concurrency + 8))),
        origin_exact: make_native(Some((false, concurrency))),
        constrained: make_native(Some((false, concurrency.saturating_sub(1).max(1)))),
        high_level: make_native(None),
    }
}

#[derive(Clone, Copy)]
struct Timing {
    construct: Duration,
    headers: Duration,
    drain: Duration,
    total: Duration,
}

async fn one(
    clients: &Clients,
    case: Case,
    uri: &str,
    geometry: Geometry,
    h2: bool,
    slow: bool,
) -> Result<Timing, Box<dyn std::error::Error + Send + Sync>> {
    let total_start = Instant::now();
    let construct_start = Instant::now();
    let request = Request::post(uri).body(Frames::new(geometry, slow))?;
    let construct = construct_start.elapsed();
    let headers_start = Instant::now();
    match case {
        Case::Direct if h2 => {
            let response = clients.direct_h2.request(request).await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::Direct => {
            let response = clients.direct.request(request).await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::Default => {
            let response = clients
                .default
                .execute_http_body(request, NativeRequestOptions::default())
                .await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::GlobalAbove => {
            let response = clients
                .global_above
                .execute_http_body(request, NativeRequestOptions::default())
                .await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::OriginAbove => {
            let response = clients
                .origin_above
                .execute_http_body(request, NativeRequestOptions::default())
                .await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::OriginExact => {
            let response = clients
                .origin_exact
                .execute_http_body(request, NativeRequestOptions::default())
                .await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::Constrained => {
            let response = clients
                .constrained
                .execute_http_body(request, NativeRequestOptions::default())
                .await?;
            drain(response, total_start, construct, headers_start).await
        }
        Case::HighLevel => {
            let mut response = clients
                .high_level
                .post(uri)
                .unwrap()
                .body(vec![b'x'; geometry.bytes()])
                .send()
                .await?;
            let headers = headers_start.elapsed();
            let drain_start = Instant::now();
            let _ = response.bytes().await?;
            Ok(Timing {
                construct,
                headers,
                drain: drain_start.elapsed(),
                total: total_start.elapsed(),
            })
        }
    }
}

async fn drain<B>(
    response: Response<B>,
    total_start: Instant,
    construct: Duration,
    headers_start: Instant,
) -> Result<Timing, Box<dyn std::error::Error + Send + Sync>>
where
    B: Body<Data = Bytes> + Send,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let headers = headers_start.elapsed();
    let drain_start = Instant::now();
    let mut body = Box::pin(response.into_body());
    while let Some(frame) = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        let _ = frame?.into_data();
    }
    Ok(Timing {
        construct,
        headers,
        drain: drain_start.elapsed(),
        total: total_start.elapsed(),
    })
}

fn percentiles(values: &[Duration]) -> (u128, u128, u128) {
    let mut values: Vec<_> = values.iter().map(Duration::as_nanos).collect();
    values.sort_unstable();
    let at = |n: usize| {
        values
            .get((values.len().saturating_sub(1) * n) / 100)
            .copied()
            .unwrap_or(0)
    };
    (at(50), at(95), at(99))
}

fn waits(clients: &Clients, case: Case) -> usize {
    let client = match case {
        Case::Direct => return 0,
        Case::Default => &clients.default,
        Case::GlobalAbove => &clients.global_above,
        Case::OriginAbove => &clients.origin_above,
        Case::OriginExact => &clients.origin_exact,
        Case::Constrained => &clients.constrained,
        Case::HighLevel => &clients.high_level,
    };
    client
        .pool_metrics()
        .acquisition_waits
        .load(Ordering::Relaxed)
}

#[allow(clippy::too_many_arguments)] // Each parameter is a benchmark dimension, kept explicit in call sites.
async fn block(
    clients: &Clients,
    case: Case,
    uri: &str,
    geometry: Geometry,
    concurrency: usize,
    count: usize,
    h2: bool,
    slow: bool,
) -> Vec<Result<Timing, Box<dyn std::error::Error + Send + Sync>>> {
    futures_util::future::join_all((0..concurrency).map(|_| async move {
        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            results.push(one(clients, case, uri, geometry, h2, slow).await);
        }
        results
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}

#[allow(clippy::too_many_arguments)] // Report fields are orthogonal measurements from one measured block.
fn log_run(
    label: &str,
    runtime: &str,
    workers: usize,
    concurrency: usize,
    repetition: usize,
    elapsed: Duration,
    outcomes: Vec<Result<Timing, Box<dyn std::error::Error + Send + Sync>>>,
    stats: &ServerStats,
    waits: usize,
) {
    let expected = outcomes.len();
    let (ok, failed): (Vec<_>, Vec<_>) = outcomes.into_iter().partition(Result::is_ok);
    let timings: Vec<_> = ok.into_iter().map(Result::unwrap).collect();
    let failures = failed.len();
    let p_total = percentiles(&timings.iter().map(|t| t.total).collect::<Vec<_>>());
    let p_construct = percentiles(&timings.iter().map(|t| t.construct).collect::<Vec<_>>());
    let p_headers = percentiles(&timings.iter().map(|t| t.headers).collect::<Vec<_>>());
    let p_drain = percentiles(&timings.iter().map(|t| t.drain).collect::<Vec<_>>());
    let connections = stats.connections.load(Ordering::Relaxed);
    let requests = stats.requests.load(Ordering::Relaxed).max(1);
    println!("{{\"record\":\"run\",\"case\":{:?},\"runtime\":{:?},\"workers\":{},\"concurrency\":{},\"repetition\":{},\"requests\":{},\"failures\":{},\"connections\":{},\"pool_waits\":{},\"throughput_req_s\":{:.3},\"total_p50_ns\":{},\"total_p95_ns\":{},\"total_p99_ns\":{},\"construct_p50_ns\":{},\"headers_p50_ns\":{},\"drain_p50_ns\":{},\"reuse_ratio\":{:.5}}}", label, runtime, workers, concurrency, repetition, timings.len(), failures + expected.saturating_sub(timings.len() + failures), connections, waits, timings.len() as f64 / elapsed.as_secs_f64(), p_total.0, p_total.1, p_total.2, p_construct.0, p_headers.0, p_drain.0, 1.0 - connections as f64 / requests as f64);
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = env::args().collect();
    let value = |name: &str, fallback: usize| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|arg| arg.parse().ok())
            .unwrap_or(fallback)
    };
    let reps = value("--reps", 5);
    let count = value("--requests", 2500);
    let h2_count = value("--h2-requests", 250);
    let max_concurrency = value("--max-concurrency", 16);
    let only_concurrency = value("--only-concurrency", 0);
    let workers = value("--workers", 0);
    let scenario = args
        .iter()
        .position(|arg| arg == "--scenario")
        .and_then(|i| args.get(i + 1))
        .map_or("primary", String::as_str);
    let h2_only = args.iter().any(|arg| arg == "--h2-only");
    let slow_only = scenario == "slow";
    let only_case = args
        .iter()
        .position(|arg| arg == "--only-case")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);
    let concurrency_values: Vec<_> = [1, 2, 4, 8, 16]
        .into_iter()
        .filter(|n| *n <= max_concurrency && (only_concurrency == 0 || *n == only_concurrency))
        .collect();
    let runtime_name = if workers == 0 {
        "tokio_default"
    } else {
        "controlled_workers"
    };
    let build_profile = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    let effective_workers = if workers == 0 {
        std::thread::available_parallelism().map_or(1, usize::from)
    } else {
        workers
    };
    let rustc = std::process::Command::new("rustc").arg("-V").output()?;
    let rustc = String::from_utf8_lossy(&rustc.stdout).trim().to_owned();
    let host = std::process::Command::new("rustc").arg("-vV").output()?;
    let host = String::from_utf8_lossy(&host.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or("unknown")
        .to_owned();
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    let runtime = if workers == 0 {
        builder.enable_all().build()?
    } else {
        builder.worker_threads(workers).enable_all().build()?
    };
    runtime.block_on(async move {
        println!("{{\"record\":\"metadata\",\"runtime\":{runtime_name:?},\"workers_configured\":{workers},\"workers_effective\":{effective_workers},\"reps\":{reps},\"requests_per_worker\":{count},\"rustc\":{rustc:?},\"host\":{host:?},\"profile\":{build_profile:?}}}");
        let geometries = if scenario == "sizes" || scenario == "all" {
            vec![
                Geometry { label: "1k_4x256", chunk: 256, count: 4 },
                Geometry { label: "64k_16x4096", chunk: 4096, count: 16 },
                Geometry { label: "1m_64x16384", chunk: 16384, count: 64 },
            ]
        } else {
            vec![Geometry { label: "64k_16x4096", chunk: 4096, count: 16 }]
        };
        let cases = [Case::Direct, Case::Default, Case::GlobalAbove, Case::OriginAbove, Case::OriginExact, Case::Constrained, Case::HighLevel];
        if !h2_only && !slow_only {
          for geometry in geometries {
            for &concurrency in &concurrency_values {
                for case in cases {
                    if only_case.is_some_and(|filter| filter != case.name()) {
                        continue;
                    }
                    let (uri, stats, server) = start_server(false).await;
                    let clients = make_clients(concurrency, false);
                    let _ = block(&clients, case, &uri, geometry, concurrency, 10, false, false).await;
                    let mut prior_waits = waits(&clients, case);
                    for repetition in 1..=reps {
                        let started = Instant::now();
                        let outcomes = block(&clients, case, &uri, geometry, concurrency, count, false, false).await;
                        let elapsed = started.elapsed();
                        let current_waits = waits(&clients, case);
                        log_run(&format!("{}_{}", geometry.label, case.name()), runtime_name, workers, concurrency, repetition, elapsed, outcomes, &stats, current_waits.saturating_sub(prior_waits));
                        prior_waits = current_waits;
                    }
                    server.abort();
                }
          }
        }
        }
        if scenario == "primary" || scenario == "all" || h2_only {
            let geometry = Geometry { label: "64k_16x4096_h2", chunk: 4096, count: 16 };
            for concurrency in concurrency_values {
                for case in [Case::Direct, Case::Default, Case::OriginAbove] {
                    if only_case.is_some_and(|filter| filter != case.name()) {
                        continue;
                    }
                    let (uri, stats, server) = start_server(true).await;
                    let clients = make_clients(concurrency, true);
                    let _ = block(&clients, case, &uri, geometry, concurrency, 10, true, false).await;
                    let mut prior_waits = waits(&clients, case);
                    for repetition in 1..=reps {
                        let started = Instant::now();
                        let outcomes = block(&clients, case, &uri, geometry, concurrency, h2_count, true, false).await;
                        let elapsed = started.elapsed();
                        let current_waits = waits(&clients, case);
                        log_run(&format!("h2_{}_{}", geometry.label, case.name()), runtime_name, workers, concurrency, repetition, elapsed, outcomes, &stats, current_waits.saturating_sub(prior_waits));
                        prior_waits = current_waits;
                    }
                    server.abort();
                }
            }
        }
        if scenario == "all" || slow_only {
            let geometry = Geometry { label: "64k_16x4096_slow1ms_frame", chunk: 4096, count: 16 };
            for case in [Case::Direct, Case::Default] {
                if only_case.is_some_and(|filter| filter != case.name()) {
                    continue;
                }
                let concurrency = 4;
                let (uri, stats, server) = start_server(false).await;
                let clients = make_clients(concurrency, false);
                let _ = block(&clients, case, &uri, geometry, concurrency, 2, false, true).await;
                for repetition in 1..=reps {
                    let started = Instant::now();
                    let outcomes = block(&clients, case, &uri, geometry, concurrency, count, false, true).await;
                    let elapsed = started.elapsed();
                    log_run(&format!("slow_{}", case.name()), runtime_name, workers, concurrency, repetition, elapsed, outcomes, &stats, waits(&clients, case));
                }
                server.abort();
            }
            let (uri, stats, server) = start_server(false).await;
            let client = Client::builder().http_version_policy(HttpVersionPolicy::Http1Only).build();
            let geometry = Geometry { label: "early_drop", chunk: 4096, count: 16 };
            let dropped = Request::post(&uri).body(Frames::new(geometry, false))?;
            drop(client.execute_http_body_default(dropped).await?);
            let recovery = Request::post(&uri).body(Frames::new(geometry, false))?;
            let response = client.execute_http_body_default(recovery).await?;
            let mut body = Box::pin(response.into_body());
            while let Some(frame) = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await { let _ = frame?; }
            println!("{{\"record\":\"lifecycle\",\"case\":\"early_drop_then_recovery\",\"requests\":{},\"connections\":{},\"recovery\":\"successfully drained\"}}", stats.requests.load(Ordering::Relaxed), stats.connections.load(Ordering::Relaxed));
            server.abort();
        }
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;
    Ok(())
}
