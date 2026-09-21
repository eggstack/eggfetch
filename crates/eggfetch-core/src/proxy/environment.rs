//! Private proxy-environment normalization helpers.

pub(super) fn normalize_proxy_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("http://{trimmed}")
    }
}
