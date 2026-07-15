#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // URL parsing should never panic for any string.
    if let Ok(url) = url::Url::parse(input) {
        // Query parameter operations should never panic.
        let mut url = url;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("key", "value");
            pairs.append_pair("foo", "bar");
        }

        let _ = url.query();
        let _ = url.query_pairs().count();
        let _ = url.host_str();
        let _ = url.port();
        let _ = url.scheme();
        let _ = url.path();
        let _ = url.fragment();
    }

    // Test URL joining with arbitrary inputs.
    if let Some(parts) = input.split('\0').collect::<Vec<&str>>().get(0..2).map(|p| p.to_vec()) {
        if let (Ok(base), Ok(relative)) = (url::Url::parse(parts[0]), url::Url::parse(parts[1])) {
            let _ = base.join(relative.as_str());
        }
    }

    // Test URL with query_pairs_mut for various inputs.
    if let Ok(mut url) = url::Url::parse("https://example.com/path") {
        let mut pairs = url.query_pairs_mut();
        // Append with arbitrary key/value (using the raw input split).
        let halves: Vec<&str> = input.splitn(2, '\0').collect();
        if halves.len() >= 2 {
            pairs.append_pair(halves[0], halves[1]);
        } else {
            pairs.append_pair(input, "test");
        }
    }
});
