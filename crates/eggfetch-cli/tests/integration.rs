#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use flate2::write::GzEncoder;
use flate2::Compression;

struct TestServer {
    port: u16,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    request_count: Arc<AtomicUsize>,
    captured_requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

#[derive(Clone)]
struct CapturedRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl TestServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        listener
            .set_nonblocking(true)
            .expect("failed to set nonblocking");
        let port = listener.local_addr().unwrap().port();
        let shutdown = Arc::new(AtomicBool::new(false));
        let request_count = Arc::new(AtomicUsize::new(0));
        let captured = Arc::new(Mutex::new(Vec::new()));

        let sd = shutdown.clone();
        let rc = request_count.clone();
        let cap = captured.clone();

        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let sd = sd.clone();
                        let rc = rc.clone();
                        let cap = cap.clone();
                        thread::spawn(move || {
                            handle_client(stream, &sd, &rc, &cap);
                        });
                    }
                    Err(_) if sd.load(Ordering::Relaxed) => break,
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });

        Self {
            port,
            shutdown,
            handle: Some(handle),
            request_count,
            captured_requests: captured,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }

    fn captured_requests(&self) -> Vec<CapturedRequest> {
        self.captured_requests.lock().unwrap().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn handle_client(
    mut stream: TcpStream,
    shutdown: &AtomicBool,
    request_count: &AtomicUsize,
    captured: &Mutex<Vec<CapturedRequest>>,
) {
    stream.set_nonblocking(false).ok();
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut reader = BufReader::new(stream.try_clone().unwrap());

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.trim().is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0].to_owned();
    let full_path = parts[1].to_owned();

    let mut headers: HashMap<String, String> = HashMap::new();
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let k = key.trim().to_lowercase();
            let v = value.trim().to_owned();
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
            headers.insert(k, v);
        }
    }

    let mut body = vec![0u8; content_length];
    let mut total = 0;
    while total < content_length {
        match reader.read(&mut body[total..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }

    request_count.fetch_add(1, Ordering::SeqCst);
    captured.lock().unwrap().push(CapturedRequest {
        method: method.clone(),
        path: full_path.clone(),
        headers: headers.clone(),
        body,
    });

    if shutdown.load(Ordering::Relaxed) {
        return;
    }

    let (path, query) = if let Some(pos) = full_path.find('?') {
        (&full_path[..pos], Some(&full_path[pos + 1..]))
    } else {
        (&full_path[..], None)
    };

    let mut response_headers = Vec::new();

    match path {
        "/get" => {
            let body_text = format!("method={method}");
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                body_text.as_bytes(),
            );
        }
        "/echo" => {
            let body_text = format!("method={method}");
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                body_text.as_bytes(),
            );
        }
        "/headers" => {
            let mut sorted: Vec<_> = headers.iter().collect();
            sorted.sort();
            let body_text: String = sorted
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                body_text.as_bytes(),
            );
        }
        "/json" => {
            response_headers.push(("Content-Type", "application/json"));
            let body_text = r#"{"message":"hello","count":42}"#;
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                body_text.as_bytes(),
            );
        }
        "/binary" => {
            response_headers.push(("Content-Type", "application/octet-stream"));
            let body_bytes: Vec<u8> = (0..=255).cycle().take(512).collect();
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                &body_bytes,
            );
        }
        "/post-echo" | "/post-body" => {
            let captured = captured.lock().unwrap();
            if let Some(last) = captured.last() {
                send_response(
                    &mut reader.get_mut(),
                    200,
                    "OK",
                    &response_headers,
                    &last.body,
                );
            } else {
                send_response(&mut reader.get_mut(), 200, "OK", &response_headers, b"");
            }
        }
        "/status/404" => {
            send_response(
                &mut reader.get_mut(),
                404,
                "Not Found",
                &response_headers,
                b"Not Found",
            );
        }
        "/status/500" => {
            send_response(
                &mut reader.get_mut(),
                500,
                "Internal Server Error",
                &response_headers,
                b"Server Error",
            );
        }
        "/status/301" => {
            let location = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("to=")))
                .map(|p| p[3..].to_owned())
                .unwrap_or_else(|| {
                    format!(
                        "{}/get",
                        reader
                            .get_ref()
                            .peer_addr()
                            .map(|a| format!("http://{a}"))
                            .unwrap_or_default()
                    )
                });
            response_headers.push(("Location", &location));
            send_response(
                &mut reader.get_mut(),
                301,
                "Moved Permanently",
                &response_headers,
                b"",
            );
        }
        "/redirect-to" => {
            let location = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("to=")))
                .map(|p| p[3..].to_owned())
                .unwrap_or_else(|| "/get".to_owned());
            response_headers.push(("Location", &location));
            send_response(&mut reader.get_mut(), 302, "Found", &response_headers, b"");
        }
        "/auth/basic" => {
            if let Some(auth) = headers.get("authorization") {
                if auth.starts_with("Basic ") {
                    send_response(
                        &mut reader.get_mut(),
                        200,
                        "OK",
                        &response_headers,
                        b"authenticated",
                    );
                } else {
                    send_response(
                        &mut reader.get_mut(),
                        401,
                        "Unauthorized",
                        &response_headers,
                        b"bad auth scheme",
                    );
                }
            } else {
                response_headers.push(("WWW-Authenticate", "Basic realm=\"test\""));
                send_response(
                    &mut reader.get_mut(),
                    401,
                    "Unauthorized",
                    &response_headers,
                    b"need auth",
                );
            }
        }
        "/auth/bearer" => {
            if let Some(auth) = headers.get("authorization") {
                if auth.starts_with("Bearer ") {
                    send_response(
                        &mut reader.get_mut(),
                        200,
                        "OK",
                        &response_headers,
                        b"authenticated",
                    );
                } else {
                    send_response(
                        &mut reader.get_mut(),
                        401,
                        "Unauthorized",
                        &response_headers,
                        b"bad auth scheme",
                    );
                }
            } else {
                response_headers.push(("WWW-Authenticate", "Bearer realm=\"test\""));
                send_response(
                    &mut reader.get_mut(),
                    401,
                    "Unauthorized",
                    &response_headers,
                    b"need auth",
                );
            }
        }
        "/cookie/set" => {
            let cookie_val = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("value=")))
                .map(|p| p[6..].to_owned())
                .unwrap_or_else(|| "test123".to_owned());
            let cookie_header = format!("session={cookie_val}; Path=/");
            response_headers.push(("Set-Cookie", &cookie_header));
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                b"cookie set",
            );
        }
        "/cookie/check" => {
            if let Some(cookie) = headers.get("cookie") {
                send_response(
                    &mut reader.get_mut(),
                    200,
                    "OK",
                    &response_headers,
                    cookie.as_bytes(),
                );
            } else {
                send_response(
                    &mut reader.get_mut(),
                    404,
                    "No Cookie",
                    &response_headers,
                    b"no cookie",
                );
            }
        }
        "/large" => {
            let size: usize = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("size=")))
                .and_then(|p| p[5..].parse().ok())
                .unwrap_or(1024);
            let body: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
            response_headers.push(("Content-Type", "text/plain"));
            send_response(&mut reader.get_mut(), 200, "OK", &response_headers, &body);
        }
        "/chunked" => {
            let stream_ref = reader.get_mut();
            let header =
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n";
            let _ = stream_ref.write_all(header.as_bytes());
            for i in 0..5 {
                let chunk = format!("chunk-{i}");
                let _ = stream_ref.write_all(format!("{:x}\r\n", chunk.len()).as_bytes());
                let _ = stream_ref.write_all(chunk.as_bytes());
                let _ = stream_ref.write_all(b"\r\n");
                thread::sleep(Duration::from_millis(10));
            }
            let _ = stream_ref.write_all(b"0\r\n\r\n");
            let _ = stream_ref.flush();
        }
        "/slow" => {
            let delay: u64 = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("ms=")))
                .and_then(|p| p[3..].parse().ok())
                .unwrap_or(5000);
            thread::sleep(Duration::from_millis(delay));
            send_response(&mut reader.get_mut(), 200, "OK", &response_headers, b"done");
        }
        "/redirect-chain" => {
            response_headers.push(("Location", "/redirect-to?to=/get"));
            send_response(&mut reader.get_mut(), 302, "Found", &response_headers, b"");
        }
        "/large-gzipped" => {
            let size: usize = query
                .and_then(|q| q.split('&').find(|p| p.starts_with("size=")))
                .and_then(|p| p[5..].parse().ok())
                .unwrap_or(1024);
            let plain: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&plain).unwrap();
            let compressed = encoder.finish().unwrap();
            response_headers.push(("Content-Encoding", "gzip"));
            response_headers.push(("Content-Type", "application/octet-stream"));
            send_response(
                &mut reader.get_mut(),
                200,
                "OK",
                &response_headers,
                &compressed,
            );
        }
        _ => {
            send_response(
                &mut reader.get_mut(),
                404,
                "Not Found",
                &response_headers,
                b"not found",
            );
        }
    }
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) {
    let status_text = match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => reason,
    };

    let mut header = format!("HTTP/1.1 {status} {status_text}\r\n");
    header.push_str(&format!("Content-Length: {}\r\n", body.len()));
    for (k, v) in extra_headers {
        header.push_str(&format!("{k}: {v}\r\n"));
    }
    header.push_str("Connection: close\r\n");
    header.push_str("\r\n");

    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("failed to get test binary path")
        .parent()
        .expect("failed to get test binary dir")
        .parent()
        .expect("failed to get target dir")
        .to_path_buf();
    path.push("eggfetch");
    #[cfg(windows)]
    {
        path.set_extension("exe");
    }
    path
}

fn run_cli(args: &[&str]) -> (String, String, Option<i32>) {
    run_cli_inner(args, false)
}

fn run_cli_no_clobber(args: &[&str]) -> (String, String, Option<i32>) {
    run_cli_inner(args, true)
}

fn run_cli_inner(args: &[&str], add_no_clobber: bool) -> (String, String, Option<i32>) {
    let bin = binary_path();
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(args)
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH");
    if add_no_clobber {
        cmd.arg("--no-clobber");
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {bin:?}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    (stdout, stderr, code)
}

fn run_cli_clean(args: &[&str]) -> (String, String, Option<i32>) {
    let bin = binary_path();
    let output = std::process::Command::new(&bin)
        .args(args)
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {bin:?}: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    (stdout, stderr, code)
}

#[test]
fn test_help_output() {
    let (stdout, _stderr, code) = run_cli_clean(&["--help"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("eggfetch"));
    assert!(stdout.contains("--method"));
    assert!(stdout.contains("--header"));
    assert!(stdout.contains("--json-output"));
}

#[test]
fn test_version_output() {
    let (stdout, _stderr, code) = run_cli_clean(&["--version"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("eggfetch"));
}

#[test]
fn test_basic_get() {
    let server = TestServer::start();
    let (stdout, _stderr, code) = run_cli(&[&format!("{}/get", server.url())]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("method=GET"));
}

#[test]
fn test_explicit_method() {
    let server = TestServer::start();
    let (stdout, _stderr, code) = run_cli(&["-X", "DELETE", &format!("{}/echo", server.url())]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("method=DELETE"));
}

#[test]
fn test_post_body() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--body", "hello world"]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "hello world");
}

#[test]
fn test_json_body() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--json", r#"{"key":"value"}"#]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout, r#"{"key":"value"}"#);
}

#[test]
fn test_query_params() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "-q", "foo=bar", "-q", "baz=42"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("method=GET"));
}

#[test]
fn test_headers_echo() {
    let server = TestServer::start();
    let url = format!("{}/headers", server.url());
    let (stdout, _stderr, code) =
        run_cli(&[&url, "-H", "X-Custom: test-value", "-H", "X-Other: 123"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("x-custom: test-value"));
    assert!(stdout.contains("x-other: 123"));
}

#[test]
fn test_json_output() {
    let server = TestServer::start();
    let url = format!("{}/json", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--json-output"]);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["url"], url);
    assert!(parsed["headers"].is_array());
    assert!(parsed["elapsed_ms"].is_number());
}

#[test]
fn test_ndjson_output() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/json", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--ndjson", "--follow"]);
    assert_eq!(code, Some(0));
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(lines.len() >= 1, "should have at least one NDJSON line");
    for line in &lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect(&format!("invalid NDJSON line: {line}"));
        assert!(parsed.is_object());
    }
}

#[test]
fn test_json_output_and_ndjson_conflict() {
    // The two output formats are mutually exclusive; passing both must
    // fail fast at argument parsing instead of silently dropping
    // `--ndjson`.
    let (_stdout, stderr, code) =
        run_cli_clean(&["http://127.0.0.1:1/x", "--json-output", "--ndjson"]);
    assert_ne!(code, Some(0), "conflicting flags must be rejected");
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "expected a clap conflict diagnostic, got: {stderr}"
    );
}

#[test]
fn test_ndjson_redirect_records_are_chronological() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/json", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--ndjson", "--follow"]);
    assert_eq!(code, Some(0));
    let lines: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid NDJSON line"))
        .collect();
    // Records must be oldest-first: any redirect hop precedes the final
    // response record.
    let final_idx = lines
        .iter()
        .position(|l| l.get("type") != Some(&serde_json::Value::String("redirect".into())));
    if let Some(final_idx) = final_idx {
        assert!(
            lines[..final_idx]
                .iter()
                .all(|l| l.get("type") == Some(&serde_json::Value::String("redirect".into()))),
            "redirect records must precede the final response"
        );
    }
}

#[test]
fn test_json_output_base64() {
    let server = TestServer::start();
    let url = format!("{}/binary", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--json-output", "--base64"]);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(parsed["body_base64"].is_string());
    let b64 = parsed["body_base64"].as_str().unwrap();
    assert!(!b64.is_empty());
}

#[test]
fn test_include_headers() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (_stdout, stderr, code) = run_cli(&[&url, "--include"]);
    assert_eq!(code, Some(0));
    assert!(stderr.contains("HTTP/"));
    assert!(stderr.contains("200"));
}

#[test]
fn test_headers_only() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--headers-only"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("HTTP/"));
    assert!(stdout.contains("200"));
}

#[test]
fn test_no_body() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--no-body"]);
    assert_eq!(code, Some(0));
    assert!(stdout.is_empty());
}

#[test]
fn test_output_file() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let (_stdout, _stderr, code) = run_cli(&[&url, "-o", path]);
    assert_eq!(code, Some(0));
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("method=GET"));
}

#[test]
fn test_no_clobber() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    std::fs::write(path, "existing").unwrap();

    let (_stdout, _stderr, code) = run_cli(&[&url, "-o", path, "--no-clobber"]);
    assert_eq!(code, Some(2));
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn test_download_filename() {
    let server = TestServer::start();
    let url = format!("{}/json", server.url());
    let dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let (_stdout, _stderr, code) = run_cli(&[&url, "--download"]);
    std::env::set_current_dir(&original_dir).unwrap();
    assert_eq!(code, Some(0));
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(!entries.is_empty(), "download should create a file");
}

#[test]
fn test_status_404() {
    let server = TestServer::start();
    let url = format!("{}/status/404", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url]);
    assert_eq!(code, Some(0));
}

#[test]
fn test_check_status_404() {
    let server = TestServer::start();
    let url = format!("{}/status/404", server.url());
    let (_stdout, stderr, code) = run_cli(&[&url, "--check-status"]);
    assert_eq!(code, Some(6));
    assert!(!stderr.is_empty(), "stderr should contain error info");
}

#[test]
fn test_check_status_500() {
    let server = TestServer::start();
    let url = format!("{}/status/500", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--check-status"]);
    assert_eq!(code, Some(6));
}

#[test]
fn test_basic_auth() {
    let server = TestServer::start();
    let url = format!("{}/auth/basic", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--auth", "user:pass123"]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "authenticated");
}

#[test]
fn test_basic_auth_required() {
    let server = TestServer::start();
    let url = format!("{}/auth/basic", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url]);
    assert_eq!(code, Some(0));
}

#[test]
fn test_bearer_auth() {
    let server = TestServer::start();
    let url = format!("{}/auth/bearer", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--bearer", "mytoken123"]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "authenticated");
}

#[test]
fn test_redirect_follow() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--follow"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("method=GET"));
}

#[test]
fn test_redirect_no_follow() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/get", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--no-follow"]);
    assert_eq!(code, Some(0));
}

#[test]
fn test_connect_timeout() {
    let (_stdout, _stderr, code) = run_cli_clean(&["http://127.0.0.1:1", "--connect-timeout", "1"]);
    assert!(
        code == Some(3) || code == Some(7),
        "expected connect or I/O error, got {code:?}"
    );
}

#[test]
fn test_invalid_url() {
    let (_stdout, _stderr, code) = run_cli_clean(&["not-a-url"]);
    assert_eq!(code, Some(2));
}

#[test]
fn test_invalid_method() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (_stdout, _stderr, code) = run_cli(&["-X", "", &url]);
    assert_eq!(code, Some(2));
}

#[test]
fn test_invalid_header() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "-H", "BadHeader"]);
    assert_eq!(code, Some(2));
}

#[test]
fn test_env_var_auth() {
    let server = TestServer::start();
    let url = format!("{}/auth/basic", server.url());
    let bin = binary_path();
    let output = std::process::Command::new(&bin)
        .args([&url])
        .env("EGGFETCH_AUTH", "user:pass123")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .output()
        .expect("failed to run CLI");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success());
    assert_eq!(stdout.trim(), "authenticated");
}

#[test]
fn test_body_file_stdin() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let bin = binary_path();
    let mut child = std::process::Command::new(&bin)
        .args(["-X", "POST", &url, "--body-file", "-"])
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn CLI");

    {
        let stdin = child.stdin.as_mut().unwrap();
        use std::io::Write;
        stdin.write_all(b"from stdin!").ok();
    }
    let result = child.wait_with_output().expect("failed to wait");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    assert!(result.status.success());
    assert_eq!(stdout, "from stdin!");
}

#[test]
fn test_body_file_path() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "from file!").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--body-file", path]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "from file!");
}

#[test]
fn test_mutual_exclusive_body() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--body", "a", "--json", r#"{"b":1}"#]);
    assert_eq!(code, Some(2));
}

#[test]
fn test_max_redirects_exceeded() {
    let server = TestServer::start();
    let url = format!("{}/redirect-chain", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--follow", "--max-redirects", "1"]);
    assert!(code == Some(5) || code == Some(3));
}

#[test]
fn test_large_download_to_file() {
    let server = TestServer::start();
    let url = format!("{}/large?size=10000", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let (_stdout, _stderr, code) = run_cli(&[&url, "-o", path]);
    assert_eq!(code, Some(0));
    let content = std::fs::read(path).unwrap();
    assert_eq!(content.len(), 10000);
}

#[test]
fn test_chunked_response() {
    let server = TestServer::start();
    let url = format!("{}/chunked", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("chunk-0"));
    assert!(stdout.contains("chunk-4"));
}

#[test]
fn test_json_output_to_file() {
    let server = TestServer::start();
    let url = format!("{}/json", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let (_stdout, _stderr, code) = run_cli(&[&url, "--json-output", "-o", path]);
    assert_eq!(code, Some(0));
    let content = std::fs::read_to_string(path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("invalid JSON in file");
    assert_eq!(parsed["status"], 200);
}

#[test]
fn test_ndjson_output_to_file() {
    let server = TestServer::start();
    let url = format!("{}/json", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let (_stdout, _stderr, code) = run_cli(&[&url, "--ndjson", "-o", path]);
    assert_eq!(code, Some(0));
    let content = std::fs::read_to_string(path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(content.lines().next().unwrap()).expect("invalid NDJSON");
    assert_eq!(parsed["status"], 200);
}

#[test]
fn test_headers_only_to_file() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_str().unwrap();
    let (_stdout, _stderr, code) = run_cli(&[&url, "--headers-only", "-o", path]);
    assert_eq!(code, Some(0));
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("HTTP/"));
    assert!(content.contains("200"));
}

#[test]
fn test_json_output_with_base64() {
    let server = TestServer::start();
    let url = format!("{}/binary", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--json-output", "--base64"]);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["body_base64"].is_string());
    assert_eq!(parsed["body_length"], 512);
}

#[test]
fn test_check_status_with_json_output() {
    let server = TestServer::start();
    let url = format!("{}/status/404", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--json-output", "--check-status"]);
    assert!(
        code == Some(6) || code == Some(7),
        "expected status or I/O error, got {code:?}"
    );
}

#[test]
fn test_invalid_json_body() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (_stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--json", "not json!"]);
    assert_eq!(code, Some(2));
}

#[test]
fn test_multiple_headers() {
    let server = TestServer::start();
    let url = format!("{}/headers", server.url());
    let (stdout, _stderr, code) = run_cli(&[
        &url,
        "-H",
        "X-First: one",
        "-H",
        "X-Second: two",
        "-H",
        "X-Third: three",
    ]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("x-first: one"));
    assert!(stdout.contains("x-second: two"));
    assert!(stdout.contains("x-third: three"));
}

#[test]
fn test_body_file_not_found() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (_stdout, stderr, code) =
        run_cli(&["-X", "POST", &url, "--body-file", "/nonexistent/file"]);
    assert!(
        code == Some(2) || code == Some(7),
        "expected usage or I/O error, got {code:?}"
    );
    assert!(!stderr.is_empty());
}

#[test]
fn test_form_urlencoded() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let (stdout, _stderr, code) =
        run_cli(&[&url, "--form", "field1=value1", "--form", "field2=value2"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("field1=value1"));
    assert!(stdout.contains("field2=value2"));
    let reqs = server.captured_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(
        reqs[0].headers.get("content-type").unwrap(),
        "application/x-www-form-urlencoded"
    );
}

#[test]
fn test_file_upload() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("upload.txt");
    std::fs::write(&file_path, "file contents here").unwrap();
    let path = file_path.to_str().unwrap();
    let file_arg = format!("upload=@{path}");
    let (stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--file", &file_arg]);
    assert_eq!(code, Some(0));
    let reqs = server.captured_requests();
    assert_eq!(reqs.len(), 1);
    let body_str = String::from_utf8_lossy(&reqs[0].body);
    assert!(
        body_str.contains("--"),
        "multipart body should contain boundary markers"
    );
    assert!(
        body_str.contains("file contents here"),
        "multipart body should contain file contents"
    );
    assert!(stdout.contains("file contents here"));
}

#[test]
fn test_cookie_flag() {
    let server = TestServer::start();
    let url = format!("{}/cookie/check", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--cookie", "session=abc123"]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "session=abc123");
    let reqs = server.captured_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].headers.get("cookie").unwrap(), "session=abc123");
}

#[test]
fn test_verbose_output() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (_stdout, stderr, code) = run_cli(&[&url, "--verbose"]);
    assert_eq!(code, Some(0));
    assert!(
        stderr.contains("HTTP/"),
        "stderr should contain HTTP status line"
    );
    assert!(stderr.contains("200"), "stderr should contain status 200");
    assert!(
        stderr.contains("Response time"),
        "stderr should contain response time"
    );
}

#[test]
fn test_include_headers_with_body() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (stdout, stderr, code) = run_cli(&[&url, "--include"]);
    assert_eq!(code, Some(0));
    assert!(stderr.contains("HTTP/"), "stderr should have status line");
    assert!(stderr.contains("200"), "stderr should have status 200");
    assert!(stdout.contains("method=GET"), "stdout should have the body");
}

#[test]
fn test_json_output_errors_field() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--json-output"]);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(
        parsed.get("errors").is_some(),
        "JSON should have an errors field"
    );
    assert!(
        parsed["errors"].as_array().unwrap().is_empty(),
        "errors field should be an empty array"
    );
}

#[test]
fn test_max_body_size() {
    let server = TestServer::start();
    let url = format!("{}/large-gzipped?size=1000", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--max-body-size", "100"]);
    assert_eq!(
        code,
        Some(5),
        "expected exit code 5 (protocol), got {code:?}"
    );
}

#[test]
fn test_check_status_200() {
    let server = TestServer::start();
    let url = format!("{}/get", server.url());
    let (_stdout, _stderr, code) = run_cli(&[&url, "--check-status"]);
    assert_eq!(code, Some(0), "check-status on 200 should exit 0");
}

#[test]
fn test_download_mode() {
    let server = TestServer::start();
    let url = format!("{}/json", server.url());
    let dir = tempfile::tempdir().unwrap();

    let bin = binary_path();
    let output = std::process::Command::new(&bin)
        .args([&url, "--download", "--no-clobber"])
        .current_dir(dir.path())
        .env_remove("EGGFETCH_AUTH")
        .env_remove("EGGFETCH_BEARER")
        .env_remove("EGGFETCH_PROXY")
        .env_remove("EGGFETCH_PROXY_AUTH")
        .output()
        .unwrap_or_else(|e| panic!("failed to run CLI {bin:?}: {e}"));
    assert_eq!(
        output.status.code(),
        Some(0),
        "download mode should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(
        !entries.is_empty(),
        "download mode should create a file in current directory"
    );
}

#[test]
fn test_body_from_file_flag() {
    let server = TestServer::start();
    let url = format!("{}/post-body", server.url());
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "hello from file").unwrap();
    let path = tmp.path().to_str().unwrap();

    let (stdout, _stderr, code) = run_cli(&["-X", "POST", &url, "--body-file", path]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "hello from file");
}

#[test]
fn test_json_output_history() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/json", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--json-output", "--follow"]);
    assert_eq!(code, Some(0));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(
        parsed.get("history").is_some(),
        "JSON should have a history field"
    );
    let history = parsed["history"].as_array().unwrap();
    assert_eq!(history.len(), 1, "history should have 1 redirect hop");
    assert_eq!(history[0]["status"], 302);
    assert_eq!(parsed["status"], 200);
}

#[test]
fn test_no_follow_returns_302() {
    let server = TestServer::start();
    let url = format!("{}/redirect-to?to=/get", server.url());
    let (stdout, _stderr, code) = run_cli(&[&url, "--no-follow"]);
    assert_eq!(code, Some(0));
    assert!(
        stdout.is_empty(),
        "302 response has no body, stdout should be empty"
    );
}
