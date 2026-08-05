# HTTPX 0.28.1 Native Compressed-Raw Adapter — Closure Record

Status: adapter implementation and final corrective closure complete for the
documented HTTPX 0.28.1 asyncio-supported surface.

The complete detailed planning record from PR #20 is preserved at
[`plans/httpx-native-compressed-raw-adapter-implementation-plan.md`](httpx-native-compressed-raw-adapter-implementation-plan.md).
Original planning PR: #20. Adapter executable SHA:
`1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`. The final corrective status is
bound to the corrective plan at
[`plans/httpx-final-metadata-ci-hygiene-corrective-pass.md`](httpx-final-metadata-ci-hygiene-corrective-pass.md).

This plan closes the native compressed-response raw-stream gap while keeping
the encoded transport body single-owner. Core now defers decompression for
compressed streaming responses until a consumer selects one mode:

- decoded mode uses the existing `compression::decompress_stream()` path;
- raw mode returns the encoded source through the narrow
  `Response::raw_bytes_stream()` accessor.

The Python binding selects the mode at the first body-consuming operation.
Native raw iteration uses the raw accessor; decoded iteration, reads, text,
and line operations use the decoded accessor. Pool leases and read timeouts
remain attached to the selected source, and no buffering, tee, or second
decompression implementation was added.

The compatibility tests cover deterministic sync and async gzip loopback
responses, source-byte accounting before chunk adaptation, one-shot mode
selection, immediate source failure, cancellation, close, and normal versus
partial finalization. The buffered raw behavior is explicitly aligned with
HTTPX 0.28.1: buffered responses reject raw iteration as already consumed.

The core decoded-header policy of removing `Content-Encoding` and
`Content-Length` after automatic decoded-response processing remains unchanged.
Core retains a narrow read-only snapshot of the original wire values for those
two headers. The HTTPX compatibility facade restores only those two original
wire values without deriving wire length from decoded bytes or changing decoder
selection. Raw and decoded body selection remains one-shot and unchanged. No
Python decompression stack, generalized metadata framework, or transport
redesign was added.

Out of scope remains a second independently consumable body, a Python
decompression stack, transport/pool redesign, HTTPX version rebasing, new CI
jobs, and promotion beyond the Stage C candidate designation.

## Evidence

The executable implementation SHA and exact final routine/extended evidence
are recorded in [httpx-parity-correction-status.md](httpx-parity-correction-status.md)
after the implementation commit and final documentation-only status commit.
