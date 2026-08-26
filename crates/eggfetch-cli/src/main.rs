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
use std::io::IsTerminal;
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

    /// Multipart file parts as `NAME=@PATH[:FILENAME]`.
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

    /// Fail if output file already exists (no overwrite).
    #[arg(long = "no-clobber")]
    no_clobber: bool,

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
    #[arg(long = "ndjson", conflicts_with = "json_output")]
    ndjson: bool,

    /// Encode binary body as base64 in JSON output.
    #[arg(long = "base64")]
    base64: bool,

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

    /// Follow redirects (default: on).
    #[arg(long = "follow", default_value_t = true)]
    follow: bool,

    /// Do not follow redirects (conflicts with `--follow`).
    #[arg(long = "no-follow", conflicts_with = "follow")]
    no_follow: bool,

    /// Maximum number of redirects.
    #[arg(long = "max-redirects", default_value = "20")]
    max_redirects: usize,

    /// Basic auth as USER:PASS (env: `EGGFETCH_AUTH`).
    #[arg(long = "auth", env = "EGGFETCH_AUTH")]
    auth: Option<String>,

    /// Bearer token (env: `EGGFETCH_BEARER`).
    #[arg(long = "bearer", env = "EGGFETCH_BEARER")]
    bearer: Option<String>,

    /// Cookies as NAME=VALUE (repeatable).
    #[arg(long = "cookie", action = clap::ArgAction::Append)]
    cookie: Vec<String>,

    /// Cookie jar file path.
    #[arg(long = "cookie-jar")]
    cookie_jar: Option<PathBuf>,

    /// Proxy URL (env: `EGGFETCH_PROXY`).
    #[arg(long = "proxy", env = "EGGFETCH_PROXY")]
    proxy: Option<String>,

    /// Proxy auth as USER:PASS (env: `EGGFETCH_PROXY_AUTH`).
    #[arg(long = "proxy-auth", env = "EGGFETCH_PROXY_AUTH")]
    proxy_auth: Option<String>,

    /// `NO_PROXY` bypass domains.
    #[arg(long = "no-proxy")]
    no_proxy: Option<String>,

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

    /// Maximum decoded body size in bytes.
    #[arg(long = "max-body-size")]
    max_body_size: Option<usize>,

    /// Maximum decompression ratio.
    #[arg(long = "max-decompression-ratio")]
    max_decompression_ratio: Option<f64>,

    /// Check HTTP status for errors (exit 6 on 4xx/5xx).
    #[arg(long = "check-status")]
    check_status: bool,

    /// Print verbose request/response info.
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Generate shell completions and exit.
    #[arg(long = "generate-completion", value_enum)]
    generate_completion: Option<Shell>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
#[allow(clippy::enum_variant_names)] // PowerShell is the canonical name; renaming breaks clap completion scripts
enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

/// Exit codes.
const EXIT_SUCCESS: u8 = 0;
const EXIT_USAGE: u8 = 2;
const EXIT_CONNECT: u8 = 3;
const EXIT_TIMEOUT: u8 = 4;
const EXIT_PROTOCOL: u8 = 5;
const EXIT_STATUS: u8 = 6;
const EXIT_IO: u8 = 7;

#[derive(Debug)]
struct StatusError(u16);

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP status {}", self.0)
    }
}

impl std::error::Error for StatusError {}

fn map_error_to_exit_code(err: &eggfetch_core::Error) -> u8 {
    use eggfetch_core::Error;
    match err {
        // Usage / configuration errors
        Error::InvalidUrl(_)
        | Error::InvalidMethod(_)
        | Error::InvalidHeaderName(_)
        | Error::InvalidHeaderValue(_)
        | Error::RequestBuild(_)
        | Error::ConflictingAuth(_)
        | Error::InvalidProxyUrl(_)
        | Error::TlsConfig(_)
        | Error::CaBundle(_)
        | Error::ClientCert(_)
        | Error::PrivateKey(_)
        | Error::CertificateVerification(_)
        | Error::HostnameVerification(_) => EXIT_USAGE,

        // Timeout errors
        Error::Timeout { .. } => EXIT_TIMEOUT,

        // Protocol / decompression errors
        Error::Protocol(_)
        | Error::Decompression(_)
        | Error::UnsupportedContentEncoding(_)
        | Error::DecodedBodyTooLarge
        | Error::DecompressionRatioExceeded
        | Error::Body(_)
        | Error::Unsupported(_)
        | Error::InvalidRedirectLocation(_)
        | Error::InvalidAuthHeader(_)
        | Error::BodyNotReplayableForRedirect
        | Error::TooManyRedirects { .. }
        | Error::BodyNotReplayableForRetry
        | Error::RetryBudgetExhausted { .. }
        | Error::RetryNotConfigured
        | Error::Http2GoAway { .. }
        | Error::Http2StreamReset { .. }
        | Error::Http2FlowControl(_)
        | Error::Http2Protocol(_)
        | Error::H3Connect(_)
        | Error::H3ConnectionClosed(_)
        | Error::H3Stream(_)
        | Error::H3Protocol(_)
        | Error::TraceCallbackAborted => EXIT_PROTOCOL,

        // Connect / TLS / proxy errors
        Error::Connect(_)
        | Error::Tls(_)
        | Error::Pool(_)
        | Error::ProxyConnect(_)
        | Error::ProxyAuthRequired
        | Error::ProxyConnectRejected { .. }
        | Error::MalformedProxyResponse(_) => EXIT_CONNECT,

        // I/O errors
        Error::Hyper(_) | Error::HyperClient(_) | Error::Io(_) => EXIT_IO,
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
        // On Windows, "C:\..." is a drive letter, not a filename separator.
        let is_drive_letter = colon_pos == 1
            && path
                .as_bytes()
                .get(2)
                .is_some_and(|&b| b == b'\\' || b == b'/');
        if is_drive_letter {
            // Skip drive letter, look for filename separator after it.
            if let Some(rel) = path.get(3..) {
                if let Some(filename_pos) = rel.find(':') {
                    let real_pos = 3 + filename_pos;
                    (&path[..real_pos], Some(path[real_pos + 1..].to_owned()))
                } else {
                    (path, None)
                }
            } else {
                (path, None)
            }
        } else {
            (&path[..colon_pos], Some(path[colon_pos + 1..].to_owned()))
        }
    } else {
        (path, None)
    };
    Ok((name.to_owned(), path.to_owned(), filename))
}

fn detect_method(cli: &Cli) -> &str {
    if let Some(ref method) = cli.method {
        return method;
    }
    // Any body-carrying option implies POST (curl's -d/-F semantics).
    if cli.body.is_some()
        || cli.body_file.is_some()
        || cli.json.is_some()
        || !cli.form.is_empty()
        || !cli.file.is_empty()
    {
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

const SECRET_HEADER_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

fn is_secret_header(name: &str) -> bool {
    SECRET_HEADER_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

fn is_binary_content_type(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.contains("octet-stream")
        || ct.contains("image/")
        || ct.contains("audio/")
        || ct.contains("video/")
        || ct.contains("application/pdf")
        || ct.contains("application/zip")
        || ct.contains("application/gzip")
}

fn format_header_value(name: &str, value: &str, redact: bool) -> String {
    if redact && is_secret_header(name) {
        if name.eq_ignore_ascii_case("authorization")
            || name.eq_ignore_ascii_case("proxy-authorization")
        {
            if let Some(space_pos) = value.find(' ') {
                let scheme = &value[..space_pos];
                format!("{scheme} <redacted>")
            } else {
                "<redacted>".to_owned()
            }
        } else {
            "<redacted>".to_owned()
        }
    } else {
        value.to_owned()
    }
}

fn format_headers(headers: &http::HeaderMap, verbose: bool, redact_secrets: bool) -> String {
    let mut out = String::new();
    for (name, value) in headers {
        if !verbose && name.as_str() == "set-cookie" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            out.push_str(name.as_str());
            out.push_str(": ");
            out.push_str(&format_header_value(name.as_str(), v, redact_secrets));
            out.push_str("\r\n");
        }
    }
    out
}

fn format_headers_machine(headers: &http::HeaderMap) -> Vec<Value> {
    headers
        .iter()
        .map(|(name, value)| json!([name.as_str(), value.to_str().unwrap_or("<binary>")]))
        .collect()
}

fn build_json_response(
    response: &eggfetch_core::Response,
    elapsed: Duration,
    body_len: Option<usize>,
    include_body_b64: bool,
    body_b64: Option<&str>,
    errors: &[String],
) -> Value {
    let headers = format_headers_machine(response.headers());

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

    let mut obj = json!({
        "url": response.url().to_string(),
        "status": response.status().as_u16(),
        "version": version_string(response.version()),
        "headers": headers,
        "elapsed_ms": elapsed.as_millis(),
        "history": history,
        "body_length": body_len,
        "errors": errors,
    });

    if include_body_b64 {
        if let Some(b64) = body_b64 {
            obj["body_base64"] = json!(b64);
        }
    }

    obj
}

/// Reduce a server-supplied filename to a safe single path component.
///
/// Strips any directory components (`/` and `\`, including Windows
/// separators), then rejects empty names, `.`, and `..` so a hostile
/// `Content-Disposition` cannot traverse out of the working directory.
fn sanitize_filename(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    Some(name.to_owned())
}

fn derive_filename(response: &eggfetch_core::Response) -> Option<String> {
    if let Some(disp) = response.headers().get("content-disposition") {
        if let Ok(s) = disp.to_str() {
            for part in s.split(';') {
                let part = part.trim();
                if let Some(name) = part.strip_prefix("filename=") {
                    let name = name.trim_matches('"').trim_matches('\'');
                    if let Some(safe) = sanitize_filename(name) {
                        return Some(safe);
                    }
                }
            }
        }
    }
    let path = response.url().path();
    sanitize_filename(path.rsplit('/').next().unwrap_or(path))
}

async fn create_output_file(path: &PathBuf, no_clobber: bool) -> Result<tokio::fs::File> {
    if no_clobber {
        // create_new maps to O_CREAT|O_EXCL so the check-and-create is
        // atomic; two concurrent invocations cannot both win.
        return tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    anyhow::Error::msg(format!(
                        "output file already exists: {} (remove --no-clobber to allow overwrite)",
                        path.display()
                    ))
                } else {
                    anyhow::Error::new(e)
                        .context(format!("failed to create output file: {}", path.display()))
                }
            });
    }
    tokio::fs::File::create(path)
        .await
        .with_context(|| format!("failed to create output file: {}", path.display()))
}

#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
async fn run(cli: Cli) -> Result<()> {
    if let Some(shell) = cli.generate_completion {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        let bin_name = "eggfetch";
        let writer = std::io::stdout();
        match shell {
            Shell::Bash => clap_complete::generate(
                clap_complete::shells::Bash,
                &mut cmd,
                bin_name,
                &mut writer.lock(),
            ),
            Shell::Zsh => clap_complete::generate(
                clap_complete::shells::Zsh,
                &mut cmd,
                bin_name,
                &mut writer.lock(),
            ),
            Shell::Fish => clap_complete::generate(
                clap_complete::shells::Fish,
                &mut cmd,
                bin_name,
                &mut writer.lock(),
            ),
            Shell::PowerShell => clap_complete::generate(
                clap_complete::shells::PowerShell,
                &mut cmd,
                bin_name,
                &mut writer.lock(),
            ),
            Shell::Elvish => clap_complete::generate(
                clap_complete::shells::Elvish,
                &mut cmd,
                bin_name,
                &mut writer.lock(),
            ),
        }
        return Ok(());
    }

    let method_str = detect_method(&cli);
    let method = Method::from_bytes(method_str.as_bytes())
        .with_context(|| format!("invalid HTTP method: {method_str}"))?;

    let follow = cli.follow && !cli.no_follow;
    let redirect_policy = RedirectPolicy::new(follow, cli.max_redirects);

    let mut client_builder = Client::builder().redirect_policy(redirect_policy);

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
            } else if !line.is_empty() {
                eprintln!("Warning: skipping unrecognized cookie jar line: {line}");
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

    let protocol_count = [cli.http1, cli.http2, cli.http3]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if protocol_count > 1 {
        anyhow::bail!("--http1, --http2, and --http3 are mutually exclusive");
    }

    // mTLS requires both halves; one-sided configuration would silently
    // produce a client without client-auth.
    if cli.cert.is_some() != cli.key.is_some() {
        anyhow::bail!("--cert and --key must be provided together for mTLS");
    }

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

    if let Some(max_size) = cli.max_body_size {
        client_builder = client_builder.max_decoded_body_size(max_size);
    }

    if let Some(ratio) = cli.max_decompression_ratio {
        client_builder = client_builder.max_decompression_ratio(ratio);
    }

    if let Some(attempts) = cli.retry {
        let mut retry_builder = RetryPolicy::builder().max_attempts(attempts);
        if let Some(delay) = cli.retry_delay {
            // Plain inter-retry delay: pin the backoff factor to 1
            // explicitly so each retry waits `delay` seconds rather than
            // growing geometrically (`delay^n`).
            retry_builder = retry_builder
                .initial_delay(Duration::from_secs(delay))
                .backoff_factor(1.0);
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

    let mut request_timeout = cli.timeout.map(Timeout::from_secs).unwrap_or_default();
    if let Some(t) = cli.connect_timeout {
        request_timeout.connect = Some(Duration::from_secs(t));
    }
    if let Some(t) = cli.total_timeout {
        request_timeout.total = Some(Duration::from_secs(t));
    }
    if let Some(t) = cli.read_timeout {
        request_timeout.read = Some(Duration::from_secs(t));
    }
    if cli.timeout.is_some()
        || cli.connect_timeout.is_some()
        || cli.total_timeout.is_some()
        || cli.read_timeout.is_some()
    {
        req_builder = req_builder.timeout(request_timeout);
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

    if body_count > 1 && !(has_form && has_files && body_count == 2) {
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

    #[allow(clippy::large_futures)]
    // eggfetch-core RequestBuilder::send future is large due to hyper/h2 internals
    let send_result = req_builder.send().await;

    match send_result {
        Ok(mut response) => {
            let elapsed = start.elapsed();
            let status = response.status();
            let is_success = status.is_success();

            if cli.json_output || cli.ndjson {
                let body_result = response.bytes().await;
                let body_len = body_result.as_ref().ok().map(bytes::Bytes::len);
                let mut json_errors: Vec<String> = Vec::new();
                if let Err(ref e) = body_result {
                    json_errors.push(e.to_string());
                }

                let body_b64 = if cli.base64 {
                    body_result.as_ref().ok().map(|b| base64_encode(b))
                } else {
                    None
                };

                let mut json_val = build_json_response(
                    &response,
                    elapsed,
                    body_len,
                    cli.base64,
                    body_b64.as_deref(),
                    &json_errors,
                );

                if !cli.json_output {
                    // NDJSON mode: redirect hops are emitted as their own
                    // chronological records below, so embedding them again
                    // in the final record's `history` would double-report
                    // each hop.
                    if let Some(obj) = json_val.as_object_mut() {
                        obj.remove("history");
                    }
                }

                if cli.json_output {
                    let output = serde_json::to_string_pretty(&json_val)?;
                    if let Some(ref path) = cli.output {
                        let mut f = create_output_file(path, cli.no_clobber).await?;
                        tokio::io::AsyncWriteExt::write_all(&mut f, output.as_bytes()).await?;
                    } else {
                        println!("{output}");
                    }
                } else {
                    // NDJSON records are chronological: redirect hops
                    // oldest-first, then the final response.
                    let mut lines: Vec<Value> = Vec::new();
                    for entry in response.history() {
                        lines.push(json!({
                            "type": "redirect",
                            "status": entry.status().as_u16(),
                            "url": entry.url().to_string(),
                            "version": version_string(entry.version()),
                        }));
                    }
                    lines.push(json_val);
                    if let Some(ref path) = cli.output {
                        let output: String = lines
                            .iter()
                            .map(|l| serde_json::to_string(l).unwrap_or_default())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let mut f = create_output_file(path, cli.no_clobber).await?;
                        tokio::io::AsyncWriteExt::write_all(
                            &mut f,
                            format!("{output}\n").as_bytes(),
                        )
                        .await?;
                    } else {
                        for line in &lines {
                            println!("{}", serde_json::to_string(line)?);
                        }
                    }
                }
                // A body read failure is a real error: the JSON record has
                // been emitted (with the message in `errors`), but the
                // process must still exit non-zero with the mapped code.
                if let Err(e) = body_result {
                    return Err(e.into());
                }
                if cli.check_status && !is_success {
                    return Err(StatusError(status.as_u16()).into());
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
                        eprint!(
                            "{}",
                            format_headers(entry.headers(), cli.verbose, cli.verbose)
                        );
                    }
                    eprintln!();
                }

                eprint!(
                    "< HTTP/{} {} {}\r\n",
                    version_string(response.version()),
                    status.as_u16(),
                    status.canonical_reason().unwrap_or("")
                );
                eprint!(
                    "{}",
                    format_headers(response.headers(), cli.verbose, cli.verbose)
                );
                eprintln!("\n--- Response time: {:.3}s ---\n", elapsed.as_secs_f64());
            }

            if cli.headers_only {
                let header_str = format!(
                    "HTTP/{} {} {}\r\n{}",
                    version_string(response.version()),
                    status.as_u16(),
                    status.canonical_reason().unwrap_or(""),
                    format_headers(response.headers(), false, false)
                );
                if let Some(ref path) = cli.output {
                    let mut f = create_output_file(path, cli.no_clobber).await?;
                    tokio::io::AsyncWriteExt::write_all(&mut f, header_str.as_bytes()).await?;
                } else {
                    print!("{header_str}");
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
                let f = create_output_file(path, cli.no_clobber).await?;
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
                    // A u64 range iterator terminates instead of wrapping,
                    // so a pathological filesystem cannot loop forever.
                    let mut new_path: Option<PathBuf> = None;
                    for counter in 1u64.. {
                        let candidate = format!("{} ({}){}", stem.to_string_lossy(), counter, ext);
                        let candidate_path = std::path::Path::new(&candidate);
                        if !candidate_path.exists() {
                            new_path = Some(PathBuf::from(candidate));
                            break;
                        }
                    }
                    match new_path {
                        Some(new_path) => {
                            output_path = Some(new_path.clone());
                            let f =
                                tokio::fs::File::create(&new_path).await.with_context(|| {
                                    format!("failed to create: {}", new_path.display())
                                })?;
                            output_file = Some(f);
                        }
                        None => anyhow::bail!(
                            "could not find an unused filename for download after u64::MAX attempts"
                        ),
                    }
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
            // Warn if writing binary body to a terminal
            if std::io::stdout().is_terminal() {
                if let Some(ct) = response.headers().get("content-type") {
                    if let Ok(ct_str) = ct.to_str() {
                        if is_binary_content_type(ct_str) {
                            eprintln!("Warning: binary content type detected; writing to terminal may produce garbled output");
                        }
                    }
                }
            }
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

            if cli.verbose {
                eprintln!();
                eprintln!("Status: {}", status.as_u16());
                eprintln!("Body: {total_bytes} bytes");
                eprintln!("Time: {:.3}s", elapsed.as_secs_f64());
            }

            if cli.check_status && !is_success {
                return Err(StatusError(status.as_u16()).into());
            }
            Ok(())
        }
        // Return the error so main() maps it to an exit code. Using
        // std::process::exit here would skip destructors (pool guards,
        // buffered writers) on the error path.
        Err(err) => Err(err.into()),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = if chunk.len() > 1 {
            u32::from(chunk[1])
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            u32::from(chunk[2])
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
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
    fn parse_file_part_windows_path() {
        let (name, path, filename) = parse_file_part("file=@C:\\Users\\runner\\file.txt").unwrap();
        assert_eq!(name, "file");
        assert_eq!(path, "C:\\Users\\runner\\file.txt");
        assert!(filename.is_none());
    }

    #[test]
    fn parse_file_part_windows_path_with_filename() {
        let (name, path, filename) =
            parse_file_part("file=@C:\\Users\\runner\\file.txt:custom.txt").unwrap();
        assert_eq!(name, "file");
        assert_eq!(path, "C:\\Users\\runner\\file.txt");
        assert_eq!(filename.as_deref(), Some("custom.txt"));
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
            no_clobber: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            base64: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: true,
            no_follow: false,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
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
            max_body_size: None,
            max_decompression_ratio: None,
            check_status: false,
            verbose: false,
            generate_completion: None,
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
            no_clobber: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            base64: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: true,
            no_follow: false,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
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
            max_body_size: None,
            max_decompression_ratio: None,
            check_status: false,
            verbose: false,
            generate_completion: None,
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
            no_clobber: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            base64: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: true,
            no_follow: false,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
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
            max_body_size: None,
            max_decompression_ratio: None,
            check_status: false,
            verbose: false,
            generate_completion: None,
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
        assert_eq!(
            map_error_to_exit_code(&Error::Tls("test".into())),
            EXIT_CONNECT
        );
        assert_eq!(
            map_error_to_exit_code(&Error::Io(std::sync::Arc::new(std::io::Error::other(
                "test"
            )))),
            EXIT_IO
        );
        assert_eq!(
            map_error_to_exit_code(&Error::DecodedBodyTooLarge),
            EXIT_PROTOCOL
        );
        assert_eq!(
            map_error_to_exit_code(&Error::InvalidProxyUrl("test".into())),
            EXIT_USAGE
        );
        assert_eq!(
            map_error_to_exit_code(&Error::ProxyConnect("test".into())),
            EXIT_CONNECT
        );
    }

    #[test]
    fn is_secret_header_detection() {
        assert!(is_secret_header("authorization"));
        assert!(is_secret_header("Authorization"));
        assert!(is_secret_header("proxy-authorization"));
        assert!(is_secret_header("cookie"));
        assert!(is_secret_header("Cookie"));
        assert!(is_secret_header("set-cookie"));
        assert!(!is_secret_header("content-type"));
        assert!(!is_secret_header("host"));
    }

    #[test]
    fn format_header_value_redaction() {
        assert_eq!(
            format_header_value("authorization", "Bearer secret123", true),
            "Bearer <redacted>"
        );
        assert_eq!(
            format_header_value("authorization", "Basic dXNlcjpwYXNz", true),
            "Basic <redacted>"
        );
        assert_eq!(
            format_header_value("cookie", "session=abc123", true),
            "<redacted>"
        );
        assert_eq!(
            format_header_value("content-type", "application/json", true),
            "application/json"
        );
        assert_eq!(
            format_header_value("authorization", "Bearer secret123", false),
            "Bearer secret123"
        );
    }

    #[test]
    fn base64_encode_works() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn follow_default_behavior() {
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
            no_clobber: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            base64: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: true,
            no_follow: false,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
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
            max_body_size: None,
            max_decompression_ratio: None,
            check_status: false,
            verbose: false,
            generate_completion: None,
        };
        let follow = cli.follow && !cli.no_follow;
        assert!(follow, "default should follow redirects");
    }

    #[test]
    fn no_follow_overrides_follow() {
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
            no_clobber: false,
            include: false,
            headers_only: false,
            no_body: false,
            json_output: false,
            ndjson: false,
            base64: false,
            timeout: None,
            connect_timeout: None,
            total_timeout: None,
            read_timeout: None,
            follow: true,
            no_follow: true,
            max_redirects: 20,
            auth: None,
            bearer: None,
            cookie: vec![],
            cookie_jar: None,
            proxy: None,
            proxy_auth: None,
            no_proxy: None,
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
            max_body_size: None,
            max_decompression_ratio: None,
            check_status: false,
            verbose: false,
            generate_completion: None,
        };
        let follow = cli.follow && !cli.no_follow;
        assert!(!follow, "--no-follow should override --follow");
    }

    #[test]
    fn sanitize_filename_strips_path_components() {
        // A hostile Content-Disposition must not escape the output dir.
        assert_eq!(
            sanitize_filename("../../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(
            sanitize_filename("..\\..\\win\\secret").as_deref(),
            Some("secret")
        );
        assert_eq!(
            sanitize_filename("report.pdf").as_deref(),
            Some("report.pdf")
        );
    }

    #[test]
    fn sanitize_filename_rejects_dot_names() {
        assert_eq!(sanitize_filename(""), None);
        assert_eq!(sanitize_filename("."), None);
        assert_eq!(sanitize_filename(".."), None);
    }
}
