#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::Timeout;
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    let parts: Vec<&str> = input.split('\0').collect();

    // Timeout::from_secs should never panic for any u64.
    if let Some(secs_str) = parts.first() {
        if let Ok(secs) = secs_str.parse::<u64>() {
            let t = Timeout::from_secs(secs);
            let _ = t.pool;
            let _ = t.connect;
            let _ = t.write;
            let _ = t.read;
            let _ = t.total;
        }
    }

    // Timeout::default and Timeout::disabled should never panic.
    let _ = Timeout::default();
    let _ = Timeout::disabled();
    let _ = Timeout::default().has_any();

    // Timeout::builder with arbitrary durations.
    if parts.len() >= 5 {
        let pool_ms = parts[0].parse::<u64>().unwrap_or(0);
        let connect_ms = parts[1].parse::<u64>().unwrap_or(0);
        let write_ms = parts[2].parse::<u64>().unwrap_or(0);
        let read_ms = parts[3].parse::<u64>().unwrap_or(0);
        let total_ms = parts[4].parse::<u64>().unwrap_or(0);

        let t = Timeout::builder()
            .pool(Duration::from_millis(pool_ms))
            .connect(Duration::from_millis(connect_ms))
            .write(Duration::from_millis(write_ms))
            .read(Duration::from_millis(read_ms))
            .total(Duration::from_millis(total_ms))
            .build();

        let _ = t.pool;
        let _ = t.connect;
        let _ = t.write;
        let _ = t.read;
        let _ = t.total;
        let _ = t.has_any();
    }
});
