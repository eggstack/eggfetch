#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::Headers;

fuzz_target!(|data: &[u8]| {
    // Interpret input as up to 3 name/value pairs separated by null bytes.
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut headers = Headers::new();
    let parts: Vec<&str> = input.split('\0').collect();

    for chunk in parts.chunks(2) {
        let name = chunk[0];
        let value = if chunk.len() > 1 { chunk[1] } else { "" };

        // insert/append should never panic.
        let _ = headers.insert(name, value);
        let _ = headers.append(name, value);

        // Reading should never panic.
        let _ = headers.get(name);
        let _ = headers.get_all(name);
        let _ = headers.get_str(name);
        let _ = headers.contains(name);
    }

    // Extend should never panic.
    let other = headers.clone();
    headers.extend(other);

    // Remove should never panic.
    for (name, _) in headers.clone().iter() {
        let name_str = name.as_str();
        headers.remove(name_str);
    }

    // Iteration should never panic.
    for (name, value) in headers.iter() {
        let _ = name.as_str();
        let _ = value.to_str();
    }

    // Keys should never panic.
    for key in headers.keys() {
        let _ = key.as_str();
    }

    let _ = headers.len();
    let _ = headers.is_empty();
    let _ = headers.into_inner();
});
