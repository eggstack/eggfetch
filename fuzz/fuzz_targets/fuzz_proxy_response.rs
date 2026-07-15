#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes through the proxy response parser.
    // This exercises bounded-line reading, status-line parsing,
    // header splitting, and all size/count limits.
    let _ = eggfetch_core::proxy::parse_proxy_response_bytes(data);
});
