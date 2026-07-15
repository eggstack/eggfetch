//! Command-line interface for eggfetch.
//!
//! A full HTTP client CLI powered by [`eggfetch_core`]. Supports all
//! HTTP methods, headers, query parameters, request bodies, multipart
//! uploads, authentication, proxies, retries, and streaming.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use bytes::Bytes;
use clap::Parser;
use eggfetch_core::cookie::CookieJar;
use eggfetch_core::{
    AuthScheme, Client, HttpVersionPolicy, Method, Multipart, NoProxy, Proxy, RedirectPolicy,
    RetryPolicy, Timeout, TlsConfig,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::io::{stdin, AsyncReadExt, AsyncWriteExt};

/// Eggfetch: a fast, modern HTTP client.
#[derive(Parser, Debug)]
#[command(name = "eggfetch", version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Target URL.
    url: String,

    /// HTTP method (GET, POST, PUT, etc.).
    #[arg(short = 'X', long = "method")]
    method: Option<String>,

    /// Repeatable headers as NAME:VALUE.
    #[arg(short = 'H', long = "header", action = clap::ArgAction::Append)]
    header: Vec<String>,

    /// Repeatable query parameters as NAME=VALUE.
    #[arg(short = 'q', long = "query", action = clap::ArgAction::Append)]
    query: Vec<String>,

    /// Form fields as NAME=VALUE (application/x-www-form-urlencoded).
    #[arg(long = "form", action = clap::ArgAction::Append)]
    form: Vec<String>,

    /// Multipart file parts as NAME=@PATH[:FILENAME].
    #[arg(long = "file", action = clap::ArgAction::Append)]
    file: Vec<String>,

    /// Raw body string.
    #[arg(long = "body")]
    body: Option<String>,

    /// Read body from file (- for stdin).
    #[arg(long = "body-file")]
    body_file: Option<String>,

    /// JSON body string with auto Content-Type.
    #[arg(long = "json")]
    json: Option<String>,

    /// Write body to file instead of stdout.
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Derive filename from Content-Disposition or URL.
    #[arg(long = "download")]
    download: bool,

    /// Include response headers in output.
    #[arg(short = 'i', long = "include")]
    include: bool,

    /// Print headers only, no body.
    #[arg(long = "headers-only")]
    headers_only: bool,

    /// Suppress body output.
    #[arg(long = "no-body")]
    no_body: bool,

    /// Machine-readable JSON output.
    #[arg(long = "json-output")]
    json_output: bool,

    /// Newline-delimited JSON output.
    #[arg(long = "ndjson")]
    ndjson: bool,

    /// General timeout in seconds.
    #[arg(long = "timeout")]
    timeout: Option<u64>,

    /// Connect timeout in seconds.
    #[arg(long = "connect-timeout")]
    connect_timeout: Option<u64>,

    /// Total timeout in seconds.
    #[arg(long = "total-timeout")]
    total_timeout: Option<u64>,

    /// Read timeout in seconds.
    #[arg(long = "read-timeout")]
    read_timeout: Option<u64>,

    /// Follow redirects (default: no follow).
    #[arg(long = "follow")]
    follow: bool,

    /// Do not follow redirects.
    #[arg(long = "no-follow", default_value_t = true)]
    no_follow: bool,

    /// Maximum number of redirects.
    #[arg(long = "max-redirects", default_value = "20")]
    max_redirects: usize,

    /// Basic auth as USER:PASS.
    #[arg(long = "auth")]
    auth: Option<String>,

    /// Bearer token.
    #[arg(long = "bearer")]
    bearer: Option<String>,

    /// Cookies as NAME=VALUE (repeatable).
    #[arg(long = "cookie", action = clap::ArgAction::Append)]
    cookie: Vec<String>,

    /// Cookie jar file path.
    #[arg(long = "cookie-jar")]
    cookie_jar: Option<PathBuf>,

    /// Proxy URL.
    #[arg(long = "proxy")]
    proxy: Option<String>,

    /// Proxy auth as USER:PASS.
    #[arg(long = "proxy-auth")]
    proxy_auth: Option<String>,

    /// `NO_PROXY` bypass domains.
    #[arg(long = "no-proxy")]
    no_proxy: Option<String>,

    /// Verify TLS certificates (default: yes).
    #[arg(long = "verify", default_value_t = true)]
    verify: bool,

    /// Disable TLS certificate verification.
    #[arg(long = "no-verify")]
    no_verify: bool,

    /// Custom CA certificate file.
    #[arg(long = "cacert")]
    cacert: Option<PathBuf>,

    /// Client certificate file for mTLS.
    #[arg(long = "cert")]
    cert: Option<PathBuf>,

    /// Client private key file for mTLS.
    #[arg(long = "key")]
    key: Option<PathBuf>,

    /// Max retry attempts.
    #[arg(long = "retry")]
    retry: Option<usize>,

    /// Delay between retries in seconds.
    #[arg(long = "retry-delay")]
    retry_delay: Option<u64>,

    /// Force HTTP/1.1 only.
    #[arg(long = "http1")]
    http1: bool,

    /// Force HTTP/2 only.
    #[arg(long = "http2")]
    http2: bool,

    /// Force HTTP/3 only.
    #[arg(long = "http3")]
    http3: bool,

    /// Disable automatic response decompression.
    #[arg(long = "no-compress")]
    no_compress: bool,

    /// Check HTTP status for errors (exit 6 on 4xx/5xx).
    #[arg(long = "check-status")]
    check_status: bool,

    /// Print verbose request/response info.
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Exit codes.
const EXIT_SUCCESS: u8 = 0;
const EXIT_USAGE: u8 = 2;
const EXIT_CONNECT: u8 = 3;
const EXIT_TIMEOUT: u8 = 4;
const EXIT_PROTOCOL: u8 = 5;
const EXIT_STATUS: u8 = 6;
const EXIT_IO: u8 = 7;

fn map_error_to_exit_code(err: &eggfetch_core::Error) -> u8 {
    match err.kind() {
        "invalid_url"
        | "invalid_method"
        | "invalid_header_name"
        | "invalid_header_value"
        | "request_build"
        | "conflicting_auth" => EXIT_USAGE,
        "timeout_pool"
        | "timeout_connect"
        | "timeout_read"
        | "timeout_write"
        | "timeout_total"
        | "timeout_proxy_connect"
        | "timeout_proxy_tls" => EXIT_TIMEOUT,
        "protocol"
        | "decompression"
        | "decoded_body_too_large"
        | "decompression_ratio_exceeded"
        | "body"
        | "http2_go_away"
        | "http2_stream_reset"
        | "http2_flow_control"
        | "http2_protocol"
        | "h3_connect"
        | "h3_connection_closed"
        | "h3_stream"
        | "h3_protocol" => EXIT_PROTOCOL,
        _ => EXIT_CONNECT,
    }
}

fn parse_header(s: &str) -> Result<(&str, &str)> {
    let (name, value) = s
        .split_once(':')
        .context("header must be in NAME:VALUE format")?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        anyhow::bail!("header name must not be empty");
    }
    Ok((name, value))
}

fn parse_query(s: &str) -> Result<(&str, &str)> {
    let (key, value) = s
        .split_once('=')
        .context("query must be in NAME=VALUE format")?;
    Ok((key, value))
}

fn parse_form(s: &str) -> Result<(String, String)> {
    let (key, value) = s
        .split_once('=')
        .context("form field must be in NAME=VALUE format")?;
    Ok((key.to_owned(), value.to_owned()))
}

fn guess_mime(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webp" => "image/webp",
        "wasm" => "application/wasm",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "yaml" | "yml" => "text/yaml",
        "toml" => "application/toml",
        _ => "application/octet-stream",
    }
}

fn percent_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn parse_file_part(s: &str) -> Result<(String, String, Option<String>)> {
    let (name, rest) = s
        .split_once('=')
        .context("file part must be in NAME=@PATH[:FILENAME] format")?;
    let path = rest
        .strip_prefix('@')
        .context("file path must start with @")?;
    let (path, filename) = if let Some(colon_pos) = path.find(':') {
        let f = &path[colon_pos + 1..];
        (&path[..colon_pos], Some(f.to_owned()))
    } else {
        (path, None)
    };
    Ok((name.to_owned(), path.to_owned(), filename))
}

fn detect_method(cli: &Cli) -> &str {
    if let Some(ref method) = cli.method {
        return method;
    }
    if cli.body.is_some() || cli.body_file.is_some() || cli.json.is_some() || !cli.form.is_empty() {
        "POST"
    } else {
        "GET"
    }
}

async fn read_body_file(path: &str) -> Result<Bytes> {
    if path == "-" {
        let mut buf = Vec::new();
        stdin()
            .read_to_end(&mut buf)
            .await
            .context("failed to read body from stdin")?;
        Ok(Bytes::from(buf))
    } else {
        let buf = tokio::fs::read(path)
            .await
            .with_context(|| format!("failed to read body file: {path}"))?;
        Ok(Bytes::from(buf))
    }
}

fn version_string(v: http::Version) -> &'static str {
    match v {
        http::Version::HTTP_09 => "0.9",
        http::Version::HTTP_10 => "1.0",
        http::Version::HTTP_11 => "1.1",
        http::Version::HTTP_2 => "2",
        _ => "unknown",
    }
}

fn format_headers(headers: &http::HeaderMap, verbose: bool) -> String {
    let mut out = String::new();
    for (name, value) in headers {
        if !verbose && name.as_str() == "set-cookie" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            out.push_str(name.as_str());
            out.push_str(": ");
            out.push_str(v);
            out.push_str("\r\n");
        }
    }
    out
}

fn build_json_response(
    response: &eggfetch_core::Response,
    elapsed: Duration,
    body_len: Option<usize>,
) -> Value {
    let headers: Vec<Value> = response
        .headers()
        .iter()
        .map(|(name, value)| json!([name.as_str(), value.to_str().unwrap_or("<binary>")]))
        .collect();

    let history: Vec<Value> = response
        .history()
        .iter()
        .map(|entry| {
            json!({
                "status": entry.status().as_u16(),
                "url": entry.url().to_string(),
                "version": version_string(entry.version()),
            })
        })
        .collect();

    json!({
        "url": response.url().to_string(),
        "status": response.status().as_u16(),
        "version": version_string(response.version()),
        "headers": headers,
        "elapsed_ms": elapsed.as_millis(),
        "history": history,
        "body_length": body_len,
    })
}

fn derive_filename(response: &eggfetch_core::Response) -> Option<String> {
    if let Some(disp) = response.headers().get("content-disposition") {
        if let Ok(s) = disp.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(name) = part.strip_prefix("filename=") {
                    let name = name.trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        return Some(name.to_owned());
                    }
                }
            }
        }
    }
    let path = response.url().path();
    if let Some(last) = path.rsplit('/').next() {
        if !last.is_empty() {
            return Some(last.to_owned());
        }
    }
    None
}

#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
async fn run(cli: Cli) -> Result<()> {
    let method_str = detect_method(&cli);
    let method = Method::from_bytes(method_str.as_bytes())
        .with_context(|| format!("invalid HTTP method: {method_str}"))?;

    let follow = cli.follow || !cli.no_follow;
    let redirect_policy = RedirectPolicy::new(follow, cli.max_redirects);

    let mut client_builder = Client::builder().redirect_policy(redirect_policy);

    if let Some(t) = cli.timeout {
        client_builder = client_builder.timeout(Timeout::from_secs(t));
    }

    if let Some(ref auth_str) = cli.auth {
        let (user, pass) = auth_str
            .split_once(':')
            .context("auth must be in USER:PASS format")?;
        let scheme = AuthScheme::basic(user, pass)?;
        client_builder = client_builder.auth(scheme);
    }

    if let Some(ref token) = cli.bearer {
        let scheme = AuthScheme::bearer(token)?;
        client_builder = client_builder.auth(scheme);
    }

    if !cli.cookie.is_empty() {
        let jar = CookieJar::new();
        for c in &cli.cookie {
            let (name, value) = parse_query(c)?;
            jar.set_default_cookie(name.to_owned(), value.to_owned());
        }
        client_builder = client_builder.cookie_jar(jar);
    }

    if let Some(ref path) = cli.cookie_jar {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read cookie jar: {}", path.display()))?;
        let jar = CookieJar::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((name, value)) = line.split_once('=') {
                jar.set_default_cookie(name.trim().to_owned(), value.trim().to_owned());
            }
        }
        client_builder = client_builder.cookie_jar(jar);
    }

    if let Some(ref proxy_url) = cli.proxy {
        let mut proxy = Proxy::all(proxy_url)?;
        if let Some(ref proxy_auth_str) = cli.proxy_auth {
            let (user, pass) = proxy_auth_str
                .split_once(':')
                .context("proxy-auth must be in USER:PASS format")?;
            proxy = proxy.auth(eggfetch_core::ProxyAuth::basic(user, pass)?);
        }
        if let Some(ref no_proxy_str) = cli.no_proxy {
            let bypass = NoProxy::parse(no_proxy_str)?;
            proxy = proxy.no_proxy(bypass);
        }
        client_builder = client_builder.proxy(proxy);
    }

    let mut tls_builder = TlsConfig::builder();
    if cli.no_verify {
        tls_builder = tls_builder.danger_accept_invalid_certs(true);
    }
    if let Some(ref ca_path) = cli.cacert {
        tls_builder = tls_builder.ca_certificate_path(ca_path)?;
    }
    if let (Some(ref cert_path), Some(ref key_path)) = (&cli.cert, &cli.key) {
        tls_builder = tls_builder.client_cert_path(cert_path, key_path)?;
    }
    client_builder = client_builder.tls_config(tls_builder.build());

    let http_version_policy = if cli.http2 {
        HttpVersionPolicy::Http2Only
    } else if cli.http3 {
        HttpVersionPolicy::Http3Only
    } else if cli.http1 {
        HttpVersionPolicy::Http1Only
    } else {
        HttpVersionPolicy::Auto { allow_http3: false }
    };
    client_builder = client_builder.http_version_policy(http_version_policy);

    if cli.no_compress {
        client_builder = client_builder.automatic_decompression(false);
    }

    if let Some(attempts) = cli.retry {
        let mut retry_builder = RetryPolicy::builder().max_attempts(attempts);
        if let Some(delay) = cli.retry_delay {
            retry_builder = retry_builder
                .backoff_factor(delay as f64)
                .initial_delay(Duration::from_secs(delay));
        }
        client_builder = client_builder.retry(retry_builder.build());
    }

    let client = client_builder.build();

    let mut req_builder = client.request(method, &cli.url)?;

    for h in &cli.header {
        let (name, value) = parse_header(h)?;
        req_builder = req_builder.header(name, value);
    }

    for q in &cli.query {
        let (key, value) = parse_query(q)?;
        req_builder = req_builder.query(key, value);
    }

    if let Some(t) = cli.timeout {
        req_builder = req_builder.timeout(Timeout::from_secs(t));
    }
    if let Some(t) = cli.connect_timeout {
        let timeout = Timeout::builder().connect(Duration::from_secs(t)).build();
        req_builder = req_builder.timeout(timeout);
    }
    if let Some(t) = cli.total_timeout {
        let timeout = Timeout::builder().total(Duration::from_secs(t)).build();
        req_builder = req_builder.timeout(timeout);
    }
    if let Some(t) = cli.read_timeout {
        let timeout = Timeout::builder().read(Duration::from_secs(t)).build();
        req_builder = req_builder.timeout(timeout);
    }

    let has_form = !cli.form.is_empty();
    let has_files = !cli.file.is_empty();
    let has_body = cli.body.is_some();
    let has_body_file = cli.body_file.is_some();
    let has_json = cli.json.is_some();

    let body_count = [has_form, has_files, has_body, has_body_file, has_json]
        .iter()
        .filter(|&&x| x)
        .count();

    if body_count > 1 && !(has_form && has_files) {
        anyhow::bail!("body sources (body, body-file, json, form, file) are mutually exclusive");
    }

    if has_json {
        let json_str = cli.json.unwrap();
        let _value: Value = serde_json::from_str(&json_str).context("invalid JSON body string")?;
        req_builder = req_builder
            .header("content-type", "application/json")
            .body(json_str);
    } else if has_body {
        req_builder = req_builder.body(cli.body.unwrap());
    } else if has_body_file {
        let body_bytes = read_body_file(&cli.body_file.unwrap()).await?;
        req_builder = req_builder.body(body_bytes);
    } else if has_files && has_form {
        let mut multipart = Multipart::new();
        for f in &cli.form {
            let (name, value) = parse_form(f)?;
            multipart = multipart.text(&name, &value)?;
        }
        for f in &cli.file {
            let (name, path, filename) = parse_file_part(f)?;
            let data = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read file: {path}"))?;
            let fname = filename.unwrap_or_else(|| {
                std::path::Path::new(&path)
                    .file_name()
                    .map_or_else(|| name.clone(), |n| n.to_string_lossy().into_owned())
            });
            let mime = guess_mime(&path);
            multipart = multipart.bytes(&name, &fname, mime, Bytes::from(data))?;
        }
        req_builder = req_builder.body(multipart.into_body());
    } else if has_files {
        let mut multipart = Multipart::new();
        for f in &cli.file {
            let (name, path, filename) = parse_file_part(f)?;
            let data = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read file: {path}"))?;
            let fname = filename.unwrap_or_else(|| {
                std::path::Path::new(&path)
                    .file_name()
                    .map_or_else(|| name.clone(), |n| n.to_string_lossy().into_owned())
            });
            let mime = guess_mime(&path);
            multipart = multipart.bytes(&name, &fname, mime, Bytes::from(data))?;
        }
        req_builder = req_builder.body(multipart.into_body());
    } else if has_form {
        let mut params = Vec::new();
        for f in &cli.form {
            let (key, value) = parse_form(f)?;
            params.push(format!(
                "{}={}",
                percent_encode(&key),
                percent_encode(&value)
            ));
        }
        let form_body = params.join("&");
        req_builder = req_builder
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form_body);
    }

    if cli.no_compress {
        req_builder = req_builder.decompress(false);
    }

    let start = Instant::now();

    let send_result = req_builder.send().await;

    match send_result {
        Ok(mut response) => {
            let elapsed = start.elapsed();
            let status = response.status();
            let is_success = status.is_success();

            if cli.json_output || cli.ndjson {
                let body_bytes = response.bytes().await.ok();
                let body_len = body_bytes.as_ref().map(bytes::Bytes::len);
                let json_val = build_json_response(&response, elapsed, body_len);

                if cli.json_output {
                    let output = serde_json::to_string_pretty(&json_val)?;
                    if let Some(ref path) = cli.output {
                        tokio::fs::write(path, output.as_bytes())
                            .await
                            .context("failed to write output file")?;
                    } else {
                        println!("{output}");
                    }
                } else {
                    let mut lines: Vec<Value> = Vec::new();
                    lines.push(json_val);
                    for entry in response.history() {
                        lines.push(json!({
                            "type": "redirect",
                            "status": entry.status().as_u16(),
                            "url": entry.url().to_string(),
                            "version": version_string(entry.version()),
                        }));
                    }
                    if let Some(ref path) = cli.output {
                        let output: String = lines
                            .iter()
                            .map(|l| serde_json::to_string(l).unwrap_or_default())
                            .collect::<Vec<_>>()
                            .join("\n");
                        tokio::fs::write(path, format!("{output}\n").as_bytes())
                            .await
                            .context("failed to write output file")?;
                    } else {
                        for line in &lines {
                            println!("{}", serde_json::to_string(line)?);
                        }
                    }
                }
                if cli.check_status && !is_success {
                    return Err(anyhow::anyhow!("HTTP status {status}")).context("request failed");
                }
                return Ok(());
            }

            if cli.include || cli.verbose {
                if !response.history().is_empty() {
                    eprintln!(
                        "\n--- Redirect History ({} hops) ---",
                        response.history().len()
                    );
                    for (i, entry) in response.history().iter().enumerate() {
                        eprintln!(
                            "  Hop {}: {} {} {}",
                            i + 1,
                            entry.status(),
                            entry.url().path(),
                            version_string(entry.version()),
                        );
                        eprint!("{}", format_headers(entry.headers(), cli.verbose));
                    }
                    eprintln!();
                }

                eprint!(
                    "< HTTP/{} {} {}\r\n",
                    version_string(response.version()),
                    status.as_u16(),
                    status.canonical_reason().unwrap_or("")
                );
                eprint!("{}", format_headers(response.headers(), cli.verbose));
                eprintln!("\n--- Response time: {:.3}s ---\n", elapsed.as_secs_f64());
            }

            if cli.headers_only {
                if let Some(ref path) = cli.output {
                    let header_str = format!(
                        "HTTP/{} {} {}\r\n{}",
                        version_string(response.version()),
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        format_headers(response.headers(), false)
                    );
                    tokio::fs::write(path, header_str.as_bytes())
                        .await
                        .context("failed to write output file")?;
                } else {
                    print!(
                        "HTTP/{} {} {}\r\n{}",
                        version_string(response.version()),
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        format_headers(response.headers(), false)
                    );
                }
                return Ok(());
            }

            if cli.no_body {
                return Ok(());
            }

            let mut output_file: Option<tokio::fs::File> = None;
            let mut output_path: Option<PathBuf> = None;

            if let Some(ref path) = cli.output {
                output_path = Some(path.clone());
                let f = tokio::fs::File::create(path)
                    .await
                    .with_context(|| format!("failed to create output file: {}", path.display()))?;
                output_file = Some(f);
            } else if cli.download {
                let filename = derive_filename(&response).unwrap_or_else(|| "download".to_owned());
                let path = std::path::Path::new(&filename);
                if path.exists() {
                    let stem = path.file_stem().unwrap_or_default();
                    let ext = path
                        .extension()
                        .map(|e| format!(".{}", e.to_string_lossy()))
                        .unwrap_or_default();
                    let mut counter = 1u32;
                    let new_path;
                    loop {
                        let candidate = format!("{} ({}){}", stem.to_string_lossy(), counter, ext);
                        let candidate_path = std::path::Path::new(&candidate);
                        if !candidate_path.exists() {
                            new_path = PathBuf::from(candidate);
                            break;
                        }
                        counter += 1;
                    }
                    output_path = Some(new_path.clone());
                    let f = tokio::fs::File::create(&new_path)
                        .await
                        .with_context(|| format!("failed to create: {}", new_path.display()))?;
                    output_file = Some(f);
                } else {
                    let f = tokio::fs::File::create(path)
                        .await
                        .with_context(|| format!("failed to create: {}", path.display()))?;
                    output_file = Some(f);
                }
            }

            if let Some(ref path) = output_path {
                eprintln!("Saving to: {}", path.display());
            }

            let mut stream = response.bytes_stream()?;
            let mut total_bytes: usize = 0;

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                total_bytes += chunk.len();

                if let Some(ref mut f) = output_file {
                    f.write_all(&chunk).await?;
                } else {
                    let mut stdout = tokio::io::stdout();
                    stdout.write_all(&chunk).await?;
                    stdout.flush().await?;
                }
            }

            if let Some(mut f) = output_file {
                f.flush().await?;
            }

            if !cli.json_output && !cli.ndjson && !cli.include && !cli.verbose {
                eprintln!();
                eprintln!("Status: {}", status.as_u16());
                eprintln!("Body: {total_bytes} bytes");
                eprintln!("Time: {:.3}s", elapsed.as_secs_f64());
            }

            if cli.check_status && !is_success {
                return Err(anyhow::anyhow!("HTTP status {status}")).context("request failed");
            }
            Ok(())
        }
        Err(err) => {
            let exit_code = map_error_to_exit_code(&err);
            eprintln!("Error: {err}");
            std::process::exit(i32::from(exit_code));
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = tokio::select! {
        result = run(cli) => result,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nInterrupted");
            return ExitCode::from(130u8);
        }
    };

    match result {
        Ok(()) => ExitCode::from(EXIT_SUCCESS),
        Err(err) => {
            let msg = format!("{err:#}");
            if msg.contains("Interrupted") {
                return ExitCode::from(130);
            }
            if let Some(eggfetch_err) = err.downcast_ref::<eggfetch_core::Error>() {
                let code = map_error_to_exit_code(eggfetch_err);
                eprintln!("Error: {eggfetch_err}");
                ExitCode::from(code)
            } else if err.downcast_ref::<std::io::Error>().is_some() {
                eprintln!("I/O error: {err}");
                ExitCode::from(EXIT_IO)
            } else {
                eprintln!("Error: {err}");
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("usage") || msg_lower.contains("parse") {
                    ExitCode::from(EXIT_USAGE)
                } else if msg_lower.contains("status") {
                    ExitCode::from(EXIT_STATUS)
                } else {
                    EXIT_USAGE.into()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_valid() {
        let (name, value) = parse_header("Content-Type: application/json").unwrap();
        assert_eq!(name, "Content-Type");
        assert_eq!(value, "application/json");
    }

    #[test]
    fn parse_header_empty_value() {
        let (name, value) = parse_header("X-Custom:").unwrap();
        assert_eq!(name, "X-Custom");
        assert_eq!(value, "");
    }

    #[test]
    fn parse_header_no_colon() {
        assert!(parse_header("BadHeader").is_err());
    }

    #[test]
    fn parse_query_valid() {
        let (key, value) = parse_query("q=hello world").unwrap();
        assert_eq!(key, "q");
        assert_eq!(value, "hello world");
    }

    #[test]
    fn parse_query_no_equals() {
        assert!(parse_query("badquery").is_err());
    }

    #[test]
    fn parse_form_valid() {
        let (key, value) = parse_form("field=value").unwrap();
        assert_eq!(key, "field");
        assert_eq!(value, "value");
    }

    #[test]
    fn parse_file_part_basic() {
        let (name, path, filename) = parse_file_part("file=@/tmp/test.txt").unwrap();
        assert_eq!(name, "file");
        assert_eq!(path, "/tmp/test.txt");
        assert!(filename.is_none());
    }

    #[test]
    fn parse_file_part_with_filename() {
        let (name, path, filename) = parse_file_part("file=@/tmp/test.txt:custom.txt").unwrap();
        assert_eq!(name, "file");
        assert_eq!(path, "/tmp/test.txt");
        assert_eq!(filename.as_deref(), Some("custom.txt"));
    }

    #[test]
    fn parse_file_part_no_at() {
        assert!(parse_file_part("file=/tmp/test.txt").is_err());
    }

    #[test]
    fn detect_method_default_get() {
        let cli = Cli {
            method: None,
            url: "http://example.com".to_owned(),
            header: vec![],
            query: vec![],
            form: vec![],
            file: vec![],
            body: None,
            body_file: None,
            json: None,
            output: None,
            download: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: false,
            no_follow: true,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
            verify: true,
            no_verify: false,
            cacert: None,
            cert: None,
            key: None,
            retry: None,
            retry_delay: None,
            http1: false,
            http2: false,
            http3: false,
            no_compress: false,
            check_status: false,
            verbose: false,
        };
        assert_eq!(detect_method(&cli), "GET");
    }

    #[test]
    fn detect_method_body_implies_post() {
        let cli = Cli {
            method: None,
            url: "http://example.com".to_owned(),
            header: vec![],
            query: vec![],
            form: vec![],
            file: vec![],
            body: Some("data".to_owned()),
            body_file: None,
            json: None,
            output: None,
            download: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: false,
            no_follow: true,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
            verify: true,
            no_verify: false,
            cacert: None,
            cert: None,
            key: None,
            retry: None,
            retry_delay: None,
            http1: false,
            http2: false,
            http3: false,
            no_compress: false,
            check_status: false,
            verbose: false,
        };
        assert_eq!(detect_method(&cli), "POST");
    }

    #[test]
    fn detect_method_explicit() {
        let cli = Cli {
            method: Some("PUT".to_owned()),
            url: "http://example.com".to_owned(),
            header: vec![],
            query: vec![],
            form: vec![],
            file: vec![],
            body: None,
            body_file: None,
            json: None,
            output: None,
            download: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: false,
            no_follow: true,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
            verify: true,
            no_verify: false,
            cacert: None,
            cert: None,
            key: None,
            retry: None,
            retry_delay: None,
            http1: false,
            http2: false,
            http3: false,
            no_compress: false,
            check_status: false,
            verbose: false,
        };
        assert_eq!(detect_method(&cli), "PUT");
    }

    #[test]
    fn version_string_works() {
        assert_eq!(version_string(http::Version::HTTP_11), "1.1");
        assert_eq!(version_string(http::Version::HTTP_10), "1.0");
        assert_eq!(version_string(http::Version::HTTP_2), "2");
    }

    #[test]
    fn exit_code_mapping() {
        use eggfetch_core::Error;

        assert_eq!(
            map_error_to_exit_code(&Error::InvalidUrl("test".into())),
            EXIT_USAGE
        );
        assert_eq!(
            map_error_to_exit_code(&Error::Connect("test".into())),
            EXIT_CONNECT
        );
        assert_eq!(
            map_error_to_exit_code(&Error::Timeout {
                phase: eggfetch_core::TimeoutPhase::Read,
                elapsed: Duration::from_secs(5),
            }),
            EXIT_TIMEOUT
        );
        assert_eq!(
            map_error_to_exit_code(&Error::Protocol("test".into())),
            EXIT_PROTOCOL
        );
    }
}
