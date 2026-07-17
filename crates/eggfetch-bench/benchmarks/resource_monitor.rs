//! Process-level resource regression harness for eggfetch.
//!
//! Measures peak RSS across several workloads to detect unbounded memory growth.
//! Runs outside production crates; uses OS-level process metrics (no unsafe).
//!
//! Usage:
//!     cargo run --bin `resource_monitor` --release
//!
//! Output is JSON to stdout for CI consumption.

#![allow(
    missing_docs,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::print_stdout,
    clippy::use_debug
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eggfetch_core::{Client, HttpVersionPolicy};
use futures_util::StreamExt;

// ---------------------------------------------------------------------------
// RSS measurement (cross-platform, no unsafe)
// ---------------------------------------------------------------------------

/// Current process RSS in bytes, or None if unavailable.
fn current_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        current_rss_linux()
    }
    #[cfg(target_os = "macos")]
    {
        current_rss_macos()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn current_rss_linux() -> Option<u64> {
    let file = std::fs::File::open("/proc/self/status").ok()?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(kb) = line.strip_prefix("VmRSS:") {
            let kb = kb.trim().strip_suffix(" kB")?.trim();
            let kb: u64 = kb.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn current_rss_macos() -> Option<u64> {
    let pid = std::process::id();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let kb: u64 = text.trim().parse().ok()?;
    Some(kb * 1024)
}

/// Convert bytes to megabytes for display. Precision loss is acceptable for RSS reporting.
#[allow(clippy::cast_precision_loss)]
fn rss_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Compute delta RSS (peak - baseline) as i64. In practice RSS values fit in i64.
#[allow(clippy::cast_possible_wrap)]
fn delta_rss(peak: u64, baseline: u64) -> i64 {
    peak as i64 - baseline as i64
}

// ---------------------------------------------------------------------------
// Minimal HTTP server (blocking, single-purpose)
// ---------------------------------------------------------------------------

struct ResourceServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
}

impl ResourceServer {
    fn start(body_size: usize, consume_body: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(false).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shut = shutdown.clone();

        thread::spawn(move || {
            let body: Vec<u8> = vec![b'x'; body_size];
            for stream in listener.incoming() {
                if shut.load(Ordering::Relaxed) {
                    break;
                }
                match stream {
                    Ok(stream) => {
                        let body = body.clone();
                        thread::spawn(move || {
                            Self::handle(stream, &body, consume_body);
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        for _ in 0..200 {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        Self { port, shutdown }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    fn handle(mut stream: TcpStream, body: &[u8], consume_body: bool) {
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut content_length: usize = 0;
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) if line.trim().is_empty() => break,
                Ok(_) => {
                    if consume_body {
                        let lower = line.to_ascii_lowercase();
                        if let Some(val) = lower.strip_prefix("content-length:") {
                            content_length = val.trim().parse().unwrap_or(0);
                        }
                    }
                }
            }
        }

        if consume_body && content_length > 0 {
            let mut remaining = content_length;
            let mut buf = [0u8; 8192];
            while remaining > 0 {
                let to_read = remaining.min(buf.len());
                match reader.read(&mut buf[..to_read]) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => remaining -= n,
                }
            }
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/octet-stream\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            body.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return;
        }
        let _ = stream.write_all(body);
    }
}

// ---------------------------------------------------------------------------
// Workloads
// ---------------------------------------------------------------------------

struct WorkloadResult {
    name: String,
    peak_rss_bytes: u64,
    delta_rss_bytes: i64,
}

fn make_client() -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .build()
}

/// Workload 1: Buffered download of a large body.
fn workload_buffered_download(_url: &str, size: usize) -> (String, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = ResourceServer::start(size, false);
    let url = server.url();
    let mut peak = current_rss_bytes().unwrap_or(0);

    rt.block_on(async {
        for _ in 0..5 {
            let client = make_client();
            let mut resp = client.get(&url).unwrap().send().await.unwrap();
            let _ = resp.bytes().await;
            if let Some(rss) = current_rss_bytes() {
                peak = peak.max(rss);
            }
        }
    });

    server.shutdown();
    ("buffered_download".to_string(), peak)
}

/// Workload 2: Streaming download of a large body.
fn workload_streaming_download(_url: &str, size: usize) -> (String, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = ResourceServer::start(size, false);
    let url = server.url();
    let mut peak = current_rss_bytes().unwrap_or(0);

    rt.block_on(async {
        for _ in 0..5 {
            let client = make_client();
            let mut resp = client.get(&url).unwrap().send().await.unwrap();
            let mut stream = resp.bytes_stream().unwrap();
            while let Some(result) = stream.next().await {
                let _ = result.unwrap();
                if let Some(rss) = current_rss_bytes() {
                    peak = peak.max(rss);
                }
            }
        }
    });

    server.shutdown();
    ("streaming_download".to_string(), peak)
}

/// Workload 3: Repeated connection reuse (100 small requests, one client).
fn workload_connection_reuse(_url: &str) -> (String, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = ResourceServer::start(1024, false);
    let url = server.url();
    let mut peak = current_rss_bytes().unwrap_or(0);

    rt.block_on(async {
        let client = make_client();
        for _ in 0..100 {
            let mut resp = client.get(&url).unwrap().send().await.unwrap();
            let _ = resp.bytes().await;
            if let Some(rss) = current_rss_bytes() {
                peak = peak.max(rss);
            }
        }
    });

    server.shutdown();
    ("connection_reuse".to_string(), peak)
}

/// Workload 4: Repeated cancelled requests.
fn workload_cancelled_requests(_url: &str) -> (String, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = ResourceServer::start(1024 * 1024, false);
    let url = server.url();
    let mut peak = current_rss_bytes().unwrap_or(0);

    rt.block_on(async {
        for _ in 0..20 {
            let client = make_client();
            let url = url.clone();
            let handle = tokio::spawn(async move {
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let _ = resp.bytes().await;
            });
            // Drop the handle immediately to simulate cancellation.
            drop(handle);
            if let Some(rss) = current_rss_bytes() {
                peak = peak.max(rss);
            }
        }
    });

    server.shutdown();
    ("cancelled_requests".to_string(), peak)
}

/// Workload 5: Concurrent requests with streaming bodies.
fn workload_concurrent_streaming(_url: &str, size: usize) -> (String, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = ResourceServer::start(size, false);
    let url = server.url();
    let mut peak = current_rss_bytes().unwrap_or(0);

    rt.block_on(async {
        let mut handles = Vec::new();
        for _ in 0..10 {
            let client = make_client();
            let url = url.clone();
            handles.push(tokio::spawn(async move {
                let mut resp = client.get(&url).unwrap().send().await.unwrap();
                let mut stream = resp.bytes_stream().unwrap();
                while let Some(result) = stream.next().await {
                    let _ = result.unwrap();
                }
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        if let Some(rss) = current_rss_bytes() {
            peak = peak.max(rss);
        }
    });

    server.shutdown();
    ("concurrent_streaming".to_string(), peak)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // Warm up: get baseline RSS after process startup.
    let baseline = current_rss_bytes().unwrap_or(0);

    eprintln!(
        "Resource monitor starting. Baseline RSS: {:.2} MB",
        rss_mb(baseline)
    );

    let mut results = Vec::new();

    let (name, peak) = workload_buffered_download("", 4 * 1024 * 1024);
    results.push(WorkloadResult {
        name,
        peak_rss_bytes: peak,
        delta_rss_bytes: delta_rss(peak, baseline),
    });

    let (name, peak) = workload_streaming_download("", 4 * 1024 * 1024);
    results.push(WorkloadResult {
        name,
        peak_rss_bytes: peak,
        delta_rss_bytes: delta_rss(peak, baseline),
    });

    let (name, peak) = workload_connection_reuse("");
    results.push(WorkloadResult {
        name,
        peak_rss_bytes: peak,
        delta_rss_bytes: delta_rss(peak, baseline),
    });

    let (name, peak) = workload_cancelled_requests("");
    results.push(WorkloadResult {
        name,
        peak_rss_bytes: peak,
        delta_rss_bytes: delta_rss(peak, baseline),
    });

    let (name, peak) = workload_concurrent_streaming("", 1024 * 1024);
    results.push(WorkloadResult {
        name,
        peak_rss_bytes: peak,
        delta_rss_bytes: delta_rss(peak, baseline),
    });

    // Output JSON report.
    println!("{{");
    println!("  \"baseline_rss_bytes\": {baseline},");
    println!("  \"baseline_rss_mb\": {:.2},", rss_mb(baseline));
    println!("  \"workloads\": [");
    for (i, r) in results.iter().enumerate() {
        let comma = if i + 1 < results.len() { "," } else { "" };
        let name = &r.name;
        let peak = r.peak_rss_bytes;
        let delta = r.delta_rss_bytes;
        println!("    {{");
        println!("      \"name\": \"{name}\",");
        println!("      \"peak_rss_bytes\": {peak},");
        println!("      \"peak_rss_mb\": {:.2},", rss_mb(peak));
        println!("      \"delta_rss_bytes\": {delta},");
        println!(
            "      \"delta_rss_mb\": {:.2}",
            rss_mb(delta.unsigned_abs())
        );
        println!("    }}{comma}");
    }
    println!("  ]");
    println!("}}");
}
