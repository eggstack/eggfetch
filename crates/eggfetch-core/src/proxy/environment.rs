//! Private proxy-environment normalization helpers.

pub(super) fn normalize_proxy_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("http://{trimmed}")
    }
}
use super::{NoProxy, ProxyRule};
use crate::error::Result;

pub(super) fn prefer_lowercase(lower: Option<String>, upper: Option<String>) -> Option<String> {
    lower.or(upper)
}

pub(super) fn parse_no_proxy(raw: Option<&str>) -> Result<Option<NoProxy>> {
    raw.map(NoProxy::parse).transpose()
}

pub(super) fn select_proxy<'a>(
    scheme: &str,
    proxy_for_http: Option<&'a str>,
    proxy_for_https: Option<&'a str>,
    all_proxy: Option<&'a str>,
) -> Option<(&'a str, ProxyRule, bool)> {
    match scheme {
        "https" => proxy_for_https
            .or(all_proxy)
            .map(|raw| (raw, ProxyRule::Https, proxy_for_https.is_none())),
        "http" => proxy_for_http
            .or(all_proxy)
            .map(|raw| (raw, ProxyRule::Http, proxy_for_http.is_none())),
        _ => None,
    }
}
