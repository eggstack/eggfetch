#![no_main]

use libfuzzer_sys::fuzz_target;
use std::time::{Duration, Instant};

use eggfetch_core::transport::alt_svc::{AltSvcCache, AltSvcLearnContext, AltSvcOrigin};

fuzz_target!(|data: &[u8]| {
    // Split input into origin + header value parts.
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut parts = input.splitn(3, '\0');
    let host = parts.next().unwrap_or("example.com");
    let header = parts.next().unwrap_or("");
    let extra = parts.next().unwrap_or("");

    // Origin construction must never panic.
    let Some(origin) = AltSvcOrigin::new("https", host, 443) else {
        return;
    };

    let cache = AltSvcCache::new();
    let now = Instant::now();

    // Learning must never panic or allocate without bound, even for
    // hostile header values.
    let values: Vec<String> = if extra.is_empty() {
        vec![header.to_owned()]
    } else {
        vec![header.to_owned(), extra.to_owned()]
    };
    let _ = cache.learn(&origin, &values, now);

    // Fresh lookup with lazy expiry must never panic.
    let _ = cache.get_fresh(&origin, now);
    let _ = cache.get_fresh(&origin, now + Duration::from_secs(100_000));

    // Clear must never panic.
    let _ = cache.clear(&origin);

    // Trust gating: untrusted contexts must not install routes, but the
    // check itself must never panic.
    let ctx = AltSvcLearnContext {
        is_https: false,
        tls_authenticated: false,
        via_proxy: true,
        origin_matches_hop: false,
    };
    assert!(!ctx.trustworthy());
});
