#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::compression::{accept_encoding_value, DecompressionLimit};

fuzz_target!(|data: &[u8]| {
    // accept_encoding_value should never panic.
    let _ = accept_encoding_value();

    // Test Content-Encoding validation with arbitrary strings.
    if let Ok(input) = std::str::from_utf8(data) {
        let _ = eggfetch_core::compression::validate_content_encodings(input);
    }

    // Test decompress_buffered with arbitrary data and encoding header.
    if let Ok(input) = std::str::from_utf8(data) {
        let parts: Vec<&str> = input.split('\0').collect();
        if parts.len() >= 2 {
            let encoding = parts[0];
            let payload = parts[1].as_bytes();
            let limit = DecompressionLimit::new();
            let _ = eggfetch_core::compression::decompress_buffered(payload, encoding, limit);
        }
    }

    // Test decompress_stream with arbitrary encoding headers.
    if let Ok(input) = std::str::from_utf8(data) {
        let parts: Vec<&str> = input.split('\0').collect();
        if let Some(encoding) = parts.first() {
            let stream: eggfetch_core::BoxBytesStream =
                Box::pin(futures_util::stream::empty());
            let limit = DecompressionLimit::new();
            let result = eggfetch_core::compression::decompress_stream(
                stream,
                Some(encoding),
                true,
                limit,
            );
            // Drop the stream without consuming it (decompress returns a boxed stream).
            drop(result);
        }
    }
});
