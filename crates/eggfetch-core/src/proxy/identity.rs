//! Private proxy connection identity encoding.

use super::ProxyConfig;

pub(super) fn connection_identity(config: &ProxyConfig) -> Vec<u8> {
    let mut identity = Vec::new();
    identity.extend_from_slice(config.uri.as_str().as_bytes());
    identity.push(0);
    if let Some(auth) = config.auth.as_ref() {
        // Hash credential bytes instead of retaining them in the
        // long-lived route key. The hash preserves cache separation
        // without storing key material for the entry lifetime. This is
        // a cache-separation heuristic, not a security boundary: two
        // domain-separated `DefaultHasher` (SipHash, random per-process
        // keys) outputs give a 128-bit separator, so an accidental
        // alias between different credentials is negligible. Keys are
        // little-endian by construction; the identity is in-process
        // only, never persisted or compared across endian targets.
        use std::hash::{Hash, Hasher};
        let header = auth.header_value();
        let mut first = std::collections::hash_map::DefaultHasher::new();
        0u8.hash(&mut first);
        header.hash(&mut first);
        let mut second = std::collections::hash_map::DefaultHasher::new();
        1u8.hash(&mut second);
        header.hash(&mut second);
        identity.extend_from_slice(&first.finish().to_le_bytes());
        identity.extend_from_slice(&second.finish().to_le_bytes());
    }
    identity.push(0);
    for (name, value) in config.proxy_headers.iter() {
        identity.extend_from_slice(name.as_str().as_bytes());
        identity.push(b':');
        identity.extend_from_slice(value.as_bytes());
        identity.push(0);
    }
    identity.extend_from_slice(
        config
            .proxy_tls_config
            .as_ref()
            .map_or(0u64, crate::tls::TlsConfig::connection_identity)
            .to_ne_bytes()
            .as_slice(),
    );
    identity.push(0);
    if let Some(addresses) = config.resolved_addresses.as_ref() {
        for address in addresses.iter() {
            identity.extend_from_slice(address.to_string().as_bytes());
            identity.push(0);
        }
    }
    identity
}
