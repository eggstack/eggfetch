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

// The native and HTTPX parsers intentionally share only these low-level
// mechanics; their entry classification remains explicit in parse_entry.
pub(super) fn parse_rules(s: &str, exact_localhost: bool) -> Result<Vec<NoProxyRule>> {
    let mut rules = Vec::new();
    for entry in s.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        rules.push(parse_entry(entry, exact_localhost)?);
    }
    Ok(rules)
}

fn parse_entry(entry: &str, exact_localhost: bool) -> Result<NoProxyRule> {
    if entry == "*" {
        return Ok(NoProxyRule::Wildcard);
    }
    if entry.eq_ignore_ascii_case("localhost") {
        return Ok(if exact_localhost {
            NoProxyRule::LocalhostExact
        } else {
            NoProxyRule::Localhost
        });
    }

    // HTTPX treats scheme-qualified NO_PROXY values as URL patterns,
    // rather than native CIDR or host rules.
    if exact_localhost && entry.contains("://") {
        let pattern = url::Url::parse(entry).map_err(|_| {
            Error::InvalidProxyUrl(format!("invalid URL in NO_PROXY entry: {entry}"))
        })?;
        let host = pattern
            .host_str()
            .ok_or_else(|| Error::InvalidProxyUrl(format!("NO_PROXY URL has no host: {entry}")))?;
        return Ok(NoProxyRule::SchemeHostPort {
            scheme: pattern.scheme().to_ascii_lowercase(),
            host: host.to_ascii_lowercase(),
            port: pattern.port(),
        });
    }

    if exact_localhost {
        if let Some(rule) = parse_httpx_ip_entry(entry)? {
            return Ok(rule);
        }
    }

    if let Ok(address) = entry.parse::<std::net::Ipv6Addr>() {
        return Ok(NoProxyRule::Host(address.to_string()));
    }

    if let Some((network, prefix)) = entry.split_once('/') {
        let network = network.parse::<std::net::IpAddr>().map_err(|_| {
            Error::InvalidProxyUrl(format!("invalid IP network in NO_PROXY entry: {entry}"))
        })?;
        let prefix = prefix.parse::<u8>().map_err(|_| {
            Error::InvalidProxyUrl(format!("invalid CIDR prefix in NO_PROXY entry: {entry}"))
        })?;
        let max_prefix = match network {
            std::net::IpAddr::V4(_) => 32,
            std::net::IpAddr::V6(_) => 128,
        };
        if prefix > max_prefix {
            return Err(Error::InvalidProxyUrl(format!(
                "CIDR prefix exceeds address width in NO_PROXY entry: {entry}"
            )));
        }
        return Ok(NoProxyRule::IpNetwork(network, prefix));
    }

    // IPv6 literal: [::1] or [::1]:8080
    if let Some(rest) = entry.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let ipv6 = &rest[..close];
            let remainder = &rest[close + 1..];
            if remainder.is_empty() {
                // bare IPv6 literal — treat as host
                return Ok(if exact_localhost {
                    NoProxyRule::HostExact(ipv6.to_ascii_lowercase())
                } else {
                    NoProxyRule::Host(entry.to_owned())
                });
            }
            if let Some(port_str) = remainder.strip_prefix(':') {
                let port = port_str.parse::<u16>().map_err(|_| {
                    Error::InvalidProxyUrl(format!("invalid port in NO_PROXY entry: {entry}"))
                })?;
                return Ok(if exact_localhost {
                    NoProxyRule::HostPortExact(format!("[{ipv6}]"), port)
                } else {
                    NoProxyRule::HostPort(format!("[{ipv6}]"), port)
                });
            }
        }
    }

    // host:port. HTTPX builds an `all://*host:port` URL pattern for a
    // non-scheme-qualified entry, so the host keeps bare-domain and
    // subdomain matching while the port remains an explicit match. In
    // particular, an entry such as `example.com:80` does not match an
    // HTTP URL whose normalized port is omitted.
    if let Some(colon_pos) = entry.rfind(':') {
        let host = &entry[..colon_pos];
        let port_str = &entry[colon_pos + 1..];
        if host.is_empty() || entry.matches(':').count() > 1 {
            return Err(Error::InvalidProxyUrl(format!(
                "invalid NO_PROXY host/port entry: {entry}"
            )));
        }
        let port = port_str.parse::<u16>().map_err(|_| {
            Error::InvalidProxyUrl(format!("invalid port in NO_PROXY entry: {entry}"))
        })?;
        return Ok(if exact_localhost {
            NoProxyRule::HostPortHttpx(host.to_owned(), port)
        } else {
            NoProxyRule::HostPort(host.to_owned(), port)
        });
    }

    // Domain suffix: .example.com
    if let Some(suffix) = entry.strip_prefix('.') {
        if suffix.is_empty() {
            return Err(Error::InvalidProxyUrl(
                "NO_PROXY entry cannot be just a dot".into(),
            ));
        }
        return Ok(NoProxyRule::DomainSuffix(entry.to_owned()));
    }

    // HTTPX builds an `all://*host` pattern for ordinary domains, which
    // matches the bare domain and subdomains at a label boundary. Keep
    // localhost and IP literals on their exact-host paths above.
    Ok(NoProxyRule::Host(entry.to_owned()))
}

pub(super) fn should_bypass_components(
    rules: &[NoProxyRule],
    scheme: &str,
    host: &str,
    port: Option<u16>,
) -> bool {
    for rule in rules {
        match rule {
            NoProxyRule::Wildcard => return true,
            NoProxyRule::Localhost => {
                let ip_host = host
                    .strip_prefix('[')
                    .and_then(|value| value.strip_suffix(']'))
                    .unwrap_or(host);
                if host.eq_ignore_ascii_case("localhost")
                    || ip_host.parse::<std::net::IpAddr>().is_ok_and(|ip| {
                        ip == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                            || ip == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
                    })
                {
                    return true;
                }
            }
            NoProxyRule::LocalhostExact => {
                if host.eq_ignore_ascii_case("localhost") {
                    return true;
                }
            }
            NoProxyRule::Host(h) => {
                if matches_host_rule(host, h) {
                    return true;
                }
            }
            NoProxyRule::HostExact(h) => {
                if matches_exact_host(host, h) {
                    return true;
                }
            }
            NoProxyRule::DomainSuffix(suffix) => {
                if matches_domain_suffix(host, suffix) {
                    return true;
                }
            }
            NoProxyRule::HostPort(h, p) => {
                let port_matches = match port {
                    Some(pu) => pu == *p,
                    None => default_port_for_scheme(scheme) == *p,
                };
                let host_matches = if h.starts_with('.') {
                    matches_domain_suffix(host, h)
                } else {
                    matches_host_rule(host, h)
                };
                if port_matches && host_matches {
                    return true;
                }
            }
            NoProxyRule::HostPortExact(h, p) => {
                let port_matches = match port {
                    Some(pu) => pu == *p,
                    None => default_port_for_scheme(scheme) == *p,
                };
                if port_matches && matches_exact_host(host, h) {
                    return true;
                }
            }
            NoProxyRule::HostPortHttpx(h, p) => {
                if port == Some(*p) && matches_host_rule(host, h) {
                    return true;
                }
            }
            NoProxyRule::IpNetwork(network, prefix) => {
                if host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|candidate| ip_in_network(candidate, *network, *prefix))
                {
                    return true;
                }
            }
            NoProxyRule::SchemeHostPort {
                scheme: rule_scheme,
                host: rule_host,
                port: rule_port,
            } => {
                if scheme.eq_ignore_ascii_case(rule_scheme)
                    && host.eq_ignore_ascii_case(rule_host)
                    && rule_port.is_none_or(|rule_port| {
                        port == Some(rule_port)
                            || (port.is_none() && default_port_for_scheme(scheme) == rule_port)
                    })
                {
                    return true;
                }
            }
        }
    }
    false
}

fn matches_domain_suffix(host: &str, suffix: &str) -> bool {
    let host_lower = host.to_ascii_lowercase();
    let suffix_lower = suffix.trim_start_matches('.').to_ascii_lowercase();
    host_lower.len() > suffix_lower.len()
        && host_lower.as_bytes()[host_lower.len() - suffix_lower.len() - 1] == b'.'
        && host_lower.ends_with(&suffix_lower)
}

fn matches_host_rule(host: &str, rule: &str) -> bool {
    let host_lower = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    let rule_lower = rule
        .trim_start_matches('.')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    host_lower == rule_lower
        || (host_lower.len() > rule_lower.len()
            && host_lower.as_bytes()[host_lower.len() - rule_lower.len() - 1] == b'.'
            && host_lower.ends_with(&rule_lower))
}

fn matches_exact_host(host: &str, rule: &str) -> bool {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .eq_ignore_ascii_case(rule.trim_start_matches('[').trim_end_matches(']'))
}

fn ip_in_network(candidate: std::net::IpAddr, network: std::net::IpAddr, prefix: u8) -> bool {
    match (candidate, network) {
        (std::net::IpAddr::V4(candidate), std::net::IpAddr::V4(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - u32::from(prefix))
            };
            u32::from(candidate) & mask == u32::from(network) & mask
        }
        (std::net::IpAddr::V6(candidate), std::net::IpAddr::V6(network)) => {
            let candidate = u128::from(candidate);
            let network = u128::from(network);
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - u32::from(prefix))
            };
            candidate & mask == network & mask
        }
        _ => false,
    }
}

fn default_port_for_scheme(scheme: &str) -> u16 {
    match scheme {
        "https" => 443,
        _ => 80,
    }
}
