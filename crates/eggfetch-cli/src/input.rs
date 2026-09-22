//! Private request input parsing and body-source helpers.

use anyhow::{Context, Result};
use bytes::Bytes;
use tokio::io::{stdin, AsyncReadExt};

use super::Cli;

pub(crate) fn parse_header(s: &str) -> Result<(&str, &str)> {
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

pub(crate) fn parse_query(s: &str) -> Result<(&str, &str)> {
    let (key, value) = s
        .split_once('=')
        .context("query must be in NAME=VALUE format")?;
    Ok((key, value))
}

pub(crate) fn parse_form(s: &str) -> Result<(String, String)> {
    let (key, value) = s
        .split_once('=')
        .context("form field must be in NAME=VALUE format")?;
    Ok((key.to_owned(), value.to_owned()))
}

pub(crate) fn guess_mime(path: &str) -> &'static str {
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

pub(crate) fn percent_encode(s: &str) -> String {
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

pub(crate) fn parse_file_part(s: &str) -> Result<(String, String, Option<String>)> {
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

pub(crate) fn detect_method(cli: &Cli) -> &str {
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

pub(crate) async fn read_body_file(path: &str) -> Result<Bytes> {
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
