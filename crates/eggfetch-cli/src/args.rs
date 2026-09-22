//! Private clap schema and CLI-only argument types.

use clap::Parser;
use std::path::PathBuf;

/// Eggfetch: a fast, modern HTTP client.
#[derive(Parser, Debug)]
#[command(name = "eggfetch", version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Cli {
    /// Target URL.
    pub(crate) url: String,

    /// HTTP method (GET, POST, PUT, etc.).
    #[arg(short = 'X', long = "method")]
    pub(crate) method: Option<String>,

    /// Repeatable headers as NAME:VALUE.
    #[arg(short = 'H', long = "header", action = clap::ArgAction::Append)]
    pub(crate) header: Vec<String>,

    /// Repeatable query parameters as NAME=VALUE.
    #[arg(short = 'q', long = "query", action = clap::ArgAction::Append)]
    pub(crate) query: Vec<String>,

    /// Form fields as NAME=VALUE (application/x-www-form-urlencoded).
    #[arg(long = "form", action = clap::ArgAction::Append)]
    pub(crate) form: Vec<String>,

    /// Multipart file parts as `NAME=@PATH[:FILENAME]`.
    #[arg(long = "file", action = clap::ArgAction::Append)]
    pub(crate) file: Vec<String>,

    /// Raw body string.
    #[arg(long = "body")]
    pub(crate) body: Option<String>,

    /// Read body from file (- for stdin).
    #[arg(long = "body-file")]
    pub(crate) body_file: Option<String>,

    /// JSON body string with auto Content-Type.
    #[arg(long = "json")]
    pub(crate) json: Option<String>,

    /// Write body to file instead of stdout.
    #[arg(short = 'o', long = "output")]
    pub(crate) output: Option<PathBuf>,

    /// Derive filename from Content-Disposition or URL.
    #[arg(long = "download")]
    pub(crate) download: bool,

    /// Fail if output file already exists (no overwrite).
    #[arg(long = "no-clobber")]
    pub(crate) no_clobber: bool,

    /// Include response headers in output.
    #[arg(short = 'i', long = "include")]
    pub(crate) include: bool,

    /// Print headers only, no body.
    #[arg(long = "headers-only")]
    pub(crate) headers_only: bool,

    /// Suppress body output.
    #[arg(long = "no-body")]
    pub(crate) no_body: bool,

    /// Machine-readable JSON output.
    #[arg(long = "json-output")]
    pub(crate) json_output: bool,

    /// Newline-delimited JSON output.
    #[arg(long = "ndjson", conflicts_with = "json_output")]
    pub(crate) ndjson: bool,

    /// Encode binary body as base64 in JSON output.
    #[arg(long = "base64")]
    pub(crate) base64: bool,

    /// General timeout in seconds.
    #[arg(long = "timeout")]
    pub(crate) timeout: Option<u64>,

    /// Connect timeout in seconds.
    #[arg(long = "connect-timeout")]
    pub(crate) connect_timeout: Option<u64>,

    /// Total timeout in seconds.
    #[arg(long = "total-timeout")]
    pub(crate) total_timeout: Option<u64>,

    /// Read timeout in seconds.
    #[arg(long = "read-timeout")]
    pub(crate) read_timeout: Option<u64>,

    /// Follow redirects (default: on).
    #[arg(long = "follow", default_value_t = true)]
    pub(crate) follow: bool,

    /// Do not follow redirects (conflicts with `--follow`).
    #[arg(long = "no-follow", conflicts_with = "follow")]
    pub(crate) no_follow: bool,

    /// Maximum number of redirects.
    #[arg(long = "max-redirects", default_value = "20")]
    pub(crate) max_redirects: usize,

    /// Basic auth as USER:PASS (env: `EGGFETCH_AUTH`).
    #[arg(long = "auth", env = "EGGFETCH_AUTH", conflicts_with = "bearer")]
    pub(crate) auth: Option<String>,

    /// Bearer token (env: `EGGFETCH_BEARER`).
    #[arg(long = "bearer", env = "EGGFETCH_BEARER", conflicts_with = "auth")]
    pub(crate) bearer: Option<String>,

    /// Cookies as NAME=VALUE (repeatable).
    #[arg(long = "cookie", action = clap::ArgAction::Append)]
    pub(crate) cookie: Vec<String>,

    /// Cookie jar file path.
    #[arg(long = "cookie-jar")]
    pub(crate) cookie_jar: Option<PathBuf>,

    /// Proxy URL (env: `EGGFETCH_PROXY`).
    #[arg(long = "proxy", env = "EGGFETCH_PROXY")]
    pub(crate) proxy: Option<String>,

    /// Proxy auth as USER:PASS (env: `EGGFETCH_PROXY_AUTH`).
    #[arg(long = "proxy-auth", env = "EGGFETCH_PROXY_AUTH")]
    pub(crate) proxy_auth: Option<String>,

    /// `NO_PROXY` bypass domains.
    #[arg(long = "no-proxy")]
    pub(crate) no_proxy: Option<String>,

    /// Disable TLS certificate verification.
    #[arg(long = "no-verify")]
    pub(crate) no_verify: bool,

    /// Custom CA certificate file.
    #[arg(long = "cacert")]
    pub(crate) cacert: Option<PathBuf>,

    /// Client certificate file for mTLS.
    #[arg(long = "cert")]
    pub(crate) cert: Option<PathBuf>,

    /// Client private key file for mTLS.
    #[arg(long = "key")]
    pub(crate) key: Option<PathBuf>,

    /// Max retry attempts.
    #[arg(long = "retry")]
    pub(crate) retry: Option<usize>,

    /// Delay between retries in seconds.
    #[arg(long = "retry-delay")]
    pub(crate) retry_delay: Option<u64>,

    /// Force HTTP/1.1 only.
    #[arg(long = "http1")]
    pub(crate) http1: bool,

    /// Force HTTP/2 only.
    #[arg(long = "http2")]
    pub(crate) http2: bool,

    /// Force HTTP/3 only.
    #[arg(long = "http3")]
    pub(crate) http3: bool,

    /// Disable automatic response decompression.
    #[arg(long = "no-compress")]
    pub(crate) no_compress: bool,

    /// Maximum decoded body size in bytes.
    #[arg(long = "max-body-size")]
    pub(crate) max_body_size: Option<usize>,

    /// Maximum decompression ratio.
    #[arg(long = "max-decompression-ratio")]
    pub(crate) max_decompression_ratio: Option<f64>,

    /// Check HTTP status for errors (exit 6 on 4xx/5xx).
    #[arg(long = "check-status")]
    pub(crate) check_status: bool,

    /// Print verbose request/response info.
    #[arg(short = 'v', long = "verbose")]
    pub(crate) verbose: bool,

    /// Generate shell completions and exit.
    #[arg(long = "generate-completion", value_enum)]
    pub(crate) generate_completion: Option<Shell>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
#[allow(clippy::enum_variant_names)] // PowerShell is the canonical name; renaming breaks clap completion scripts
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}
