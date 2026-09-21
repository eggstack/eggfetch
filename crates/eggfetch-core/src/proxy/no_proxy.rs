//! Private HTTPX-specific `NO_PROXY` parsing primitives.

use super::NoProxyRule;
use crate::error::{Error, Result};

/// Parse IP-shaped entries using HTTPX's intentionally narrower rules.
pub(super) fn parse_httpx_ip_entry(entry: &str) -> Result<Option<NoProxyRule>> {
    fn invalid_ipv6_entry(entry: &str) -> Error {
        const MAX_ENTRY_CHARS: usize = 256;
        let mut chars = entry.chars();
        let mut display: String = chars.by_ref().take(MAX_ENTRY_CHARS).collect();
        if chars.next().is_some() {
            display.push_str("...");
        }
        Error::InvalidProxyUrl(format!("invalid IPv6 NO_PROXY entry: {display}"))
    }

    // HTTPX checks IPv4/IPv6 hostnames before URL-pattern construction.
    // IPv4 CIDR-looking values become exact host patterns, while IPv6
    // prefix-looking values are rejected by its URL-pattern parser.
    if let Some((address, _prefix)) = entry.split_once('/') {
        if address.parse::<std::net::Ipv4Addr>().is_ok() {
            return Ok(Some(NoProxyRule::HostExact(address.to_ascii_lowercase())));
        }
        if address.parse::<std::net::Ipv6Addr>().is_ok() {
            return Err(invalid_ipv6_entry(entry));
        }
    }
    if entry.starts_with('[') {
        return Err(invalid_ipv6_entry(entry));
    }
    if let Ok(address) = entry.parse::<std::net::Ipv4Addr>() {
        return Ok(Some(NoProxyRule::HostExact(address.to_string())));
    }
    if let Ok(address) = entry.parse::<std::net::Ipv6Addr>() {
        return Ok(Some(NoProxyRule::Host(address.to_string())));
    }
    if entry.matches(':').count() > 1 {
        return Err(invalid_ipv6_entry(entry));
    }
    Ok(None)
}
