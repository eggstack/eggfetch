#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::redirect::{drops_body_on_redirect, is_redirect_status, redirect_method};
use http::{Method, StatusCode};

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let parts: Vec<&str> = input.split('\0').collect();
    if parts.len() < 2 {
        return;
    }

    // Parse status code from first part.
    let status_num: u16 = match parts[0].parse() {
        Ok(n) => n,
        Err(_) => return,
    };
    let status = match StatusCode::from_u16(status_num) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Parse method from second part.
    let method = match parts[1] {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        _ => return,
    };

    // These functions should never panic.
    let _ = is_redirect_status(status);
    let _ = redirect_method(status, &method);
    let _ = drops_body_on_redirect(status, &method);

    // If we have a location, try resolving it against a base URL.
    if parts.len() >= 3 {
        let location = parts[2];
        if let Ok(base) = url::Url::parse("https://example.com/origin") {
            let _ = base.join(location);
        }
    }
});
