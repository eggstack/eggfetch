//! Private output filename and file-creation policy.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Reduce a server-supplied filename to a safe single path component.
///
/// Strips any directory components (`/` and `\\`, including Windows
/// separators), then rejects empty names, `.`, and `..` so a hostile
/// `Content-Disposition` cannot traverse out of the working directory.
pub(crate) fn sanitize_filename(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('\0')
        || is_windows_reserved_name(name)
    {
        return None;
    }
    Some(name.to_owned())
}

pub(super) fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.']);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub(crate) fn derive_filename(response: &eggfetch_core::Response) -> Option<String> {
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

pub(crate) async fn create_output_file(
    path: &PathBuf,
    no_clobber: bool,
) -> Result<tokio::fs::File> {
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
