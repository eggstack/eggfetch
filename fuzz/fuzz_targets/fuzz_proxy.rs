#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::{NoProxy, Proxy};

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let parts: Vec<&str> = input.split('\0').collect();

    // NoProxy::parse should never panic.
    if let Some(nop_str) = parts.first() {
        let _ = NoProxy::parse(nop_str);
    }

    // Proxy::all / Proxy::http / Proxy::https should never panic.
    if let Some(proxy_url) = parts.first() {
        let _ = Proxy::all(proxy_url);
        let _ = Proxy::http(proxy_url);
        let _ = Proxy::https(proxy_url);
    }

    // If we have both a no_proxy string and a URL, test should_bypass.
    if parts.len() >= 2 {
        let nop_str = parts[0];
        let url_str = parts[1];

        if let Ok(np) = NoProxy::parse(nop_str) {
            if let Ok(url) = url::Url::parse(url_str) {
                let _ = np.should_bypass(&url);
            }
        }
    }

    // Test Proxy with auth if we have 3 parts.
    if parts.len() >= 3 {
        let proxy_url = parts[0];
        let username = parts[1];
        let password = parts[2];

        if let Ok(proxy) = Proxy::all(proxy_url) {
            if let Ok(auth) = eggfetch_core::ProxyAuth::basic(username, password) {
                let proxy = proxy.auth(auth);
                let _ = proxy.should_use_for_scheme("http");
                let _ = proxy.should_use_for_scheme("https");
                let _ = proxy.uri();
                let _ = proxy.rule();
            }
        }
    }
});
