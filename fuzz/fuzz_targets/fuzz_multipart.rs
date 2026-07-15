#![no_main]

use libfuzzer_sys::fuzz_target;

use eggfetch_core::Boundary;

fuzz_target!(|data: &[u8]| {
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Boundary::try_new should never panic.
    let _ = Boundary::try_new(input);

    // Boundary::random should never panic.
    let _ = Boundary::random();

    // If the input is a valid boundary, exercise the Multipart builder.
    if let Ok(boundary) = Boundary::try_new(input) {
        use eggfetch_core::Multipart;
        let mp = Multipart::with_boundary(boundary);

        // Add a text part with arbitrary name and value.
        let name = if let Some(p) = data.get(0..1) {
            std::str::from_utf8(p).unwrap_or("f")
        } else {
            "f"
        };
        let value = if data.len() > 1 {
            std::str::from_utf8(&data[1..]).unwrap_or("")
        } else {
            ""
        };

        if let Ok(mp) = mp.text(name, value) {
            let claimed = mp.content_length();
            let _ = mp.is_replayable();
            let _ = mp.boundary().as_str();
            let _ = mp.parts().len();
            let body = mp.into_body();
            let actual = body.len();
            // content_length must match actual body length.
            assert_eq!(
                claimed,
                actual,
                "content_length ({claimed}) != body.len() ({actual})"
            );
            let _ = body.is_empty();
        }
    }
});
