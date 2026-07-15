#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let parts: Vec<&str> = input.split('\0').collect();
    if parts.len() < 2 {
        return;
    }

    let header_value = parts[0];
    let url_str = parts[1];

    let url = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return,
    };

    // parse_set_cookie_headers should never panic.
    let headers = vec![header_value.to_owned()];
    let _ = eggfetch_core::cookie::parse_set_cookie_headers(&url, &headers);

    // CookieJar operations should never panic.
    let jar = eggfetch_core::cookie::CookieJar::new();
    jar.update_from_response(&url, &headers);
    let _ = jar.cookies_for_url(&url);
    let _ = jar.all_cookies();
    let _ = jar.len();
    let _ = jar.is_empty();
    jar.expire_stale();
});
