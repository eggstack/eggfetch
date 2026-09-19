# Issue #24 — streaming decompression chunk-boundary corrective

Status: **active / ready for handoff**

Planning baseline: `9ecdd04cf76ee128116534e5c65b76f34ed9fa99` (coordinated 0.1.8 release commit)

Trigger: [issue #24](https://github.com/eggstack/eggfetch/issues/24), reproduced against 0.1.7 and still present on the 0.1.8 planning baseline.

## Objective

Correct corruption in automatic streaming decompression when a compressed response body is delivered as multiple byte-stream items, including HTTP/1.1 `Transfer-Encoding: chunked`, without changing eggfetch's public Rust/Python/CLI/FFI/Node/HTTPX APIs, feature graph, timeout semantics, raw-body behavior, compression negotiation, or dependency versions.

The implementation should be deliberately narrower than a decompression refactor: replace the faulty private stream-to-reader adapter with the already-present `tokio_util::io::StreamReader`, preserve the existing outer `tokio::io::BufReader` for this corrective pass, add regressions at the decoder and high-level HTTP paths, then requalify the executable tree under the repository's existing exact-SHA compatibility policy.

## Research conclusion

The proposed direction is correct, with one conservative adjustment: **do not remove the existing `BufReader` in the corrective pass**.

Current `compression.rs` has a crate-private custom `StreamReader` that converts `BoxBytesStream` to Tokio `AsyncRead`. It has two correctness defects:

1. After a source chunk has been fully copied, `offset == buffer.len()` but the buffer is not cleared in the source-chunk branch. On the next source item the code appends the new chunk to the already-consumed bytes, resets `offset = 0`, and can re-emit old compressed bytes. A source sequence `C1, C2, C3` can therefore be presented approximately as `C1, C1||C2, C1||C2||C3`, corrupting gzip/Brotli input and explaining CRC/decoder failures.
2. An empty `Bytes` source item can result in a successful zero-byte `AsyncRead` before the underlying stream is actually exhausted. Empty successful reads are EOF-significant to buffered/decoder consumers and must not be synthesized for a non-EOF stream.

The defect is not intrinsically a transfer-decoding bug. Hyper already removes HTTP/1.1 chunk framing before `wrap_incoming()` exposes DATA frames. Chunked transfer merely makes multi-item delivery deterministic, so it exposes the private adapter defect. A sufficiently fragmented `Content-Length` response can exercise the same class of bug.

The exact `tokio-util 0.7.18` already locked by this repository provides `tokio_util::io::StreamReader`. Its implementation:

- stores one current `Buf` item;
- advances consumed bytes with `Buf::advance`;
- polls the underlying stream only after the current item is exhausted;
- loops over empty chunks instead of reporting them as EOF;
- implements both `AsyncRead` and `AsyncBufRead`.

Therefore no new crate, feature, MSRV increase, or dependency update is needed. The current compression features already enable `tokio-util` with its `io` support.

`async-compression::tokio::bufread::*Decoder` requires an `AsyncBufRead` input, so the upstream adapter can technically feed it directly. However the present eggfetch stack is:

```text
BoxBytesStream
  -> eggfetch private StreamReader (AsyncRead)
  -> tokio::io::BufReader
  -> async-compression decoder
  -> tokio_util::io::ReaderStream
  -> BoxBytesStream
```

For this corrective pass use:

```text
BoxBytesStream
  -> error mapping to std::io::Error
  -> tokio_util::io::StreamReader
  -> tokio::io::BufReader
  -> async-compression decoder
  -> tokio_util::io::ReaderStream
  -> BoxBytesStream
```

Keeping `BufReader` isolates the correction to the broken adapter and minimizes changes in decoder read-ahead, transport polling cadence, decoded chunk segmentation, timeout interaction, and memory behavior. A later optimization may benchmark whether the now-redundant `BufReader` can be removed, but that is explicitly outside this bugfix.

Relevant upstream evidence:

- Tokio `AsyncBufRead` documents that consumed bytes must not be returned again and that an empty filled buffer indicates EOF.
- `tokio-util 0.7.18` `StreamReader` advances the current `Buf` and loops past empty chunks.
- `async-compression 0.4.x` Tokio bufread decoders consume `AsyncBufRead` and expose decoded `AsyncRead`.
- `tokio-util 0.7.18` is already in the lockfile; 0.7.19 exists, but an upgrade is unnecessary for this correction and must not be bundled into it.

## Public/downstream compatibility contract

This pass is a behavior correction behind private implementation boundaries. Preserve all of the following exactly:

- public `ResponseBody` variants, fields, and exhaustive source shape;
- public `BoxBytesStream` alias;
- `Response::bytes()`, `text()`, `bytes_stream()`, and `raw_bytes_stream()` signatures;
- single-consumption semantics;
- `.decompress(false)` / raw encoded-byte behavior;
- `Accept-Encoding` negotiation and feature-dependent advertised codings;
- visible header policy after automatic decoding: strip decoded `Content-Encoding` and `Content-Length` exactly as today while retaining existing wire metadata;
- `Error` enum, `Error::kind()` tokens, and unsupported-content-encoding behavior;
- malformed compressed data remains a decompression failure rather than being reclassified;
- decoded-size and decompression-ratio limits, including stacked-encoding accounting;
- final-boundary `BodyTimeoutStream` ownership and read/total timeout semantics;
- pool-lease lifetime and release semantics;
- raw-vs-decoded stream selection;
- all default/full/lean feature aliases;
- Python, CLI, FFI, Node prototype, HTTPX 0.28.1, and HTTPX2 2.12.0 public surfaces.

Do **not** add a global `From<eggfetch_core::Error> for std::io::Error` conversion merely to satisfy `tokio_util::io::StreamReader`. Map each stream item locally before constructing the adapter so the change stays private and the current string/error classification at the decoder boundary remains explicit.

Decoded `bytes_stream()` chunk sizes are not a stable public framing contract and tests must not freeze exact decoder output chunk boundaries. Compatibility assertions should compare the ordered byte sequence, terminal error classification, timeout behavior, and metadata rather than a particular number or size of decoded chunks.

## Part A — prove the baseline defect before changing implementation

Add a focused regression that deterministically fails on `9ecdd04` before replacing the adapter.

Use a valid compressed payload split into at least three non-empty source `Bytes` items. Include an empty source item in a separate case or in a position that does not obscure the multi-item corruption proof.

At minimum prove:

- gzip multi-item streaming decode fails or yields bytes different from the plaintext on the baseline;
- Brotli multi-item streaming decode fails or yields bytes different from the plaintext on the baseline;
- the identical compressed bytes decode correctly through `decompress_buffered`;
- one-item streaming input remains green, demonstrating that the regression is fragmentation-sensitive rather than payload-invalid.

Record the baseline command and observed failure in this plan's closure record. Do not modify historical commits.

## Part B — replace only the private faulty adapter

In `crates/eggfetch-core/src/compression.rs`:

1. remove the private eggfetch `StreamReader` type and its `AsyncRead` implementation;
2. locally map `BoxBytesStream` item errors to `std::io::Error` using the same textual information currently forwarded by the private adapter;
3. construct `tokio_util::io::StreamReader` from that mapped stream;
4. retain `tokio::io::BufReader::new(reader)` around it for gzip, deflate, Brotli, and zstd;
5. retain the existing `async-compression` decoder selection and `ReaderStream` output conversion;
6. do not change compression feature ownership or Cargo manifests unless compilation proves a currently undeclared feature is actually required. The expected result is **no manifest or lockfile change**.

Prefer a tiny private helper for the repeated stream-to-buffered-reader adaptation only if it materially reduces four identical blocks without obscuring codec-specific feature gating. Do not turn this pass into a compression architecture cleanup.

### Explicit non-solutions

Do not:

- manually parse or dechunk HTTP/1.1 bodies;
- special-case `Transfer-Encoding: chunked`;
- buffer the whole response before decoding;
- switch the streaming path to `decompress_buffered`;
- disable automatic decompression for chunked responses;
- add an eggsearch-specific workaround;
- change Hyper body framing;
- remove `BufReader` as an incidental optimization;
- update Tokio, tokio-util, async-compression, flate2, brotli, or zstd as part of the fix;
- change public body/error/timeout APIs.

## Part C — codec-level fragmentation regression matrix

Add deterministic streaming tests in `compression.rs` (or the existing closest compression test module) that exercise the shared adapter, not only each codec library.

Required cases:

- gzip: valid compressed member split across many source items;
- Brotli: valid compressed body split across many source items;
- deflate: equivalent split-input case when the feature is enabled;
- zstd: equivalent split-input case when the feature is enabled;
- empty source `Bytes` between non-empty compressed chunks does not terminate decode early;
- source chunk boundaries at awkward positions (including headers/trailers for gzip) reconstruct exactly the original plaintext;
- malformed compressed bytes still return `Error::Decompression`;
- configured decoded-size and decompression-ratio enforcement remains green on fragmented input.

Generate compact deterministic test payloads at test time where practical. Do not check the multi-kilobyte base64 issue report into the repository unless a smaller deterministic corpus cannot reproduce the bug. The issue's captured payloads are evidence, not a requirement to duplicate large fixtures.

The regression must fail against the baseline custom adapter and pass after replacement.

## Part D — high-level HTTP/1.1 transfer-shape regression

Add a loopback integration regression through the public path used by downstream Rust consumers:

```text
Client::get(...)
  -> send()
  -> Response::bytes_stream()
  -> collect ordered decoded bytes
```

Serve the **same compressed bytes** in two response shapes:

1. `Content-Length: N`;
2. `Transfer-Encoding: chunked` with deliberately small transfer chunks so the compressed entity arrives as multiple DATA frames.

Cover gzip and Brotli, matching issue #24. Require exact plaintext equality for both transfer shapes.

Use a bounded explicit total deadline so hangs are failures. The server fixture should remain local/deterministic and must not depend on DuckDuckGo, Startpage, or any external network.

Also add/retain a raw-mode assertion for a chunked compressed response:

- automatic decompression disabled;
- returned encoded byte sequence equals the original compressed bytes exactly;
- no decoder-specific error is introduced.

This protects the temporary eggsearch workaround and proves the fix does not alter raw transport consumption.

## Part E — metadata, timeout, and error compatibility checks

Because the implementation changes the stream adapter directly below decompression, add or run focused proof that unrelated observable behavior remains stable:

- decoded response still strips visible `Content-Encoding` and `Content-Length`;
- `wire_content_encoding()` and `wire_content_length()` retain current metadata semantics;
- `raw_bytes_stream()` bypasses decoding and remains one-shot mutually exclusive with decoded selection;
- malformed gzip/Brotli remains `error.kind() == "decompression"`;
- unsupported encoding remains `unsupported_content_encoding`;
- decoded-size limit still trips as `decoded_body_too_large`;
- ratio limit still trips as `decompression_ratio_exceeded`;
- read timeout, total timeout, and pool-lease release tests remain unchanged and green.

Do not assert exact decompressor error strings such as a specific CRC sentence as a stable API unless an existing test already establishes that contract. Preserve typed/error-kind compatibility rather than freezing third-party codec wording.

## Part F — downstream/API regression guard

This corrective must not repeat the public-shape regression previously caught in the total-deadline campaign.

Run at minimum:

```sh
cargo test -p eggfetch-core --all-features --test response_body_public_shape -- --test-threads=1
cargo test -p eggfetch-core --all-features --test total_body_deadline_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test trailer_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_http_body_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_tower_service_tests -- --test-threads=1
cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests -- --test-threads=1
```

Add the new compression/transfer-shape test filters to this focused set once named.

Then run:

```sh
./scripts/check.sh
```

before committing executable changes.

Expected diff boundary for the executable corrective is primarily:

- `crates/eggfetch-core/src/compression.rs`;
- an existing `eggfetch-core` integration test file, or one narrowly named decompression integration file if that is cleaner.

A change to `body.rs`, `response.rs`, `response_decode.rs`, `pipeline/finalize.rs`, public exports, Cargo feature declarations, adapter crates, or release workflows is a scope-expansion signal and requires written justification before proceeding.

## Part G — exact-SHA qualification and release handoff

This changes executable core behavior, so the repository's current exact-SHA compatibility binding becomes historical after implementation.

After focused tests and Tier 1 pass:

1. freeze one clean executable/test SHA;
2. make no further executable/test/build changes while qualifying;
3. run the existing extended/package/security gates required by `docs/verification-policy.md`;
4. renew HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA compatibility evidence on that same freeze;
5. update the live compatibility ledger/profiles only with truthful results;
6. append closure evidence to this plan and move the plan-index entry to completed only after the required evidence exists.

A coordinated patch release containing the correction is required before downstream handoff is marked complete. Do not tell eggsearch or another consumer to remove its workaround based only on an unpublished git commit.

The downstream eggsearch follow-up is explicitly separate: after a fixed `eggfetch-core` version is on crates.io, eggsearch may bump the dependency and remove its narrow HTML-engine `.decompress(false)` workaround under its own plan/test pass. No eggsearch source change belongs in this eggfetch corrective.

## Acceptance criteria

- [ ] Baseline `9ecdd04` has recorded red evidence for fragmented streaming gzip and Brotli through the affected path.
- [ ] Root cause is fixed by replacing the private adapter, not by transfer-encoding special cases or eager buffering.
- [ ] The implementation uses the already-locked `tokio-util 0.7.18`; no dependency/version/MSRV change is required.
- [ ] The existing `tokio::io::BufReader` layer remains in this corrective.
- [ ] The private custom `StreamReader` is removed.
- [ ] gzip fragmented streaming decode equals the original plaintext.
- [ ] Brotli fragmented streaming decode equals the original plaintext.
- [ ] deflate and zstd fragmented cases pass when their features are enabled.
- [ ] Empty source chunks are skipped without premature EOF.
- [ ] High-level `Content-Length` and HTTP/1.1 chunked forms of identical gzip/Brotli bytes both decode correctly through `Client -> send -> bytes_stream`.
- [ ] Raw/decompression-disabled chunked mode returns the exact encoded bytes.
- [ ] No public `ResponseBody`, `BoxBytesStream`, `Response`, `Client`, `Error`, timeout, or compression configuration API changes.
- [ ] No Python/CLI/FFI/Node/HTTPX surface change.
- [ ] Existing metadata stripping/wire-metadata semantics remain unchanged.
- [ ] Existing decompression/size/ratio error kinds remain unchanged.
- [ ] Existing total/read timeout and pool-lease behavior remains green.
- [ ] No exact decoded chunk-size/count contract is introduced.
- [ ] Tier 1 passes on the executable change.
- [ ] Extended/package/security and exact-SHA compatibility qualification complete before release closure.
- [ ] A coordinated crates.io patch is published before downstream workaround-removal handoff is declared complete.

## Closure record template

Append when implemented:

```text
Planning baseline: 9ecdd04cf76ee128116534e5c65b76f34ed9fa99
Issue: #24

Baseline fragmented gzip:
Baseline fragmented Brotli:
Baseline one-item control:
Baseline buffered controls:

Implementation commit(s):
Final executable freeze SHA:
Published coordinated version:

Adapter disposition:
tokio-util version:
BufReader retained:
Dependency/feature/MSRV delta:

gzip fragmented:
Brotli fragmented:
deflate fragmented:
zstd fragmented:
empty-chunk case:
malformed/error-kind cases:
decoded-size/ratio cases:

high-level Content-Length gzip:
high-level chunked gzip:
high-level Content-Length Brotli:
high-level chunked Brotli:
raw/decompress(false) chunked:

ResponseBody public-shape:
wire metadata:
raw-vs-decoded one-shot:
read/total timeout:
pool lease:
native body/service:
lean standard-http1:

Tier 1:
Extended:
Package:
Security:
MSRV:
HTTPX 0.28.1:
HTTPX2 2.12.0:
Remote CI:

eggsearch workaround status:
Downstream handoff:
Known limitations:
Documentation-only descendant SHA:
```

Missing evidence remains missing. Do not convert an unrun qualification gate, unpublished release, or downstream workaround still awaiting adoption into a pass.

## Closure record (2026-09-19)

```text
Planning baseline: 9ecdd04cf76ee128116534e5c65b76f34ed9fa99
Issue: #24

Baseline fragmented gzip: RED — `gzip_fragmented_three_chunks_matches_plaintext`
  FAILED on baseline tree with `Decompression("CRC computed does not match")`;
  1-byte, empty-chunk, awkward-header, and generous-limit gzip cases also FAILED.
Baseline fragmented Brotli: RED — `brotli_fragmented_three_chunks_matches_plaintext`
  FAILED on baseline tree; 1-byte and empty-chunk Brotli cases also FAILED.
Baseline one-item control: GREEN — `gzip_one_item_streaming_control_decodes` ok,
  `brotli_one_item_streaming_control_decodes` ok on baseline.
Baseline buffered controls: GREEN — `gzip_buffered_control_decodes` ok,
  `brotli_buffered_control_decodes` ok on baseline.
Baseline command: `cargo test -p eggfetch-core --all-features
  --test streaming_decompression_tests -- --test-threads=1`
  → 8 passed; 12 failed (all 12 failures are fragmented/high-level decode;
  raw, metadata, malformed, unsupported, and one-item/buffered controls passed).

Implementation commit(s): 37ab02b3873a4f0ce7018bd716e326bcf0595230
  (fix + `streaming_decompression_tests.rs` + compression docs)
Final executable freeze SHA: 37ab02b3873a4f0ce7018bd716e326bcf0595230
Published coordinated version: PENDING — release commit 0ddcecc5e59e82c0cbf2d3d0647fd58b3d534dff
  bumps all six crates + PyPI to 0.1.9 with CHANGELOG; `cargo publish`
  to crates.io, `v0.1.9` tag, and PyPI `pypi.yml` dispatch have NOT been run
  in this pass and require maintainer action from a trusted environment.

Adapter disposition: private eggfetch `StreamReader` removed from
  `crates/eggfetch-core/src/compression.rs`; per-codec mapping via
  `stream.map(|r| r.map_err(|e| std::io::Error::other(e.to_string())))`
  into `tokio_util::io::StreamReader`, wrapped in existing `tokio::io::BufReader`.
tokio-util version: 0.7.18 (already locked; no manifest/lockfile behavior change
  beyond version-bump release commit).
BufReader retained: yes.
Dependency/feature/MSRV delta: none (no Cargo feature, dependency, or
  rust-version change; MSRV stays 1.89).

gzip fragmented: PASS — 3-way split, 1-byte fragments, awkward header/trailer
  splits all equal plaintext after fix (20/20 streaming tests green).
Brotli fragmented: PASS — same matrix green.
deflate fragmented: PASS.
zstd fragmented: PASS.
empty-chunk case: PASS — gzip + Brotli interleaved empty `Bytes` skipped.
malformed/error-kind cases: PASS — fragmented garbage stays
  `error.kind() == "decompression"`; unsupported stays
  `unsupported_content_encoding`.
decoded-size/ratio cases: PASS — fragmented size trips `decoded_body_too_large`,
  ratio trips `decompression_ratio_exceeded`, generous limits decode exactly.

high-level Content-Length gzip: PASS.
high-level chunked gzip: PASS (7-byte transfer chunks + flush/sleep; identical bytes).
high-level Content-Length Brotli: PASS.
high-level chunked Brotli: PASS.
raw/decompress(false) chunked: PASS — exact encoded bytes (also GREEN on baseline,
  proving transport intact).

ResponseBody public-shape: PASS (`response_body_public_shape` 1/1).
wire metadata: PASS — decoded strips visible `Content-Encoding`/`Content-Length`,
  `wire_content_encoding() == "gzip"`, `wire_content_length()` equals wire len.
raw-vs-decoded one-shot: PASS via existing `EncodedStreaming` selection + raw test.
read/total timeout: PASS — `total_body_deadline_tests` 23/23, `timeout_tests`,
  lifecycle compat (`test_native_timeout_classification`, proxy/TLS, shutdown) green.
pool lease: PASS — `pool_tests`, `pool_permit_released_after_decoded_body_limit_error`,
  strengthened native lease proof in extended run.
native body/service: PASS — `native_http_body_tests`, `native_tower_service_tests`.
lean standard-http1: PASS — `lean_route_tests` 10/10 on
  `--no-default-features --features standard-http1,tls-rustls`.

Tier 1: PASS locally (`./scripts/check.sh`, EXIT 0) and remote CI green on fix
  commit (run 35429786728, 7m11s) and release commit (run 35432939352, 7m54s).
Extended: PASS (`./scripts/check.sh extended`, EXIT 0) with 2 expected skips
  (Node JS artifact absent; downstream artifact manifest absent). Includes
  Rust 1.89.0 MSRV matrix, docs, FFI, resource monitor, lifecycle, soak,
  merge-lossless, benchmarks. Full compat in extended run: 1871 passed.
Package: PASS on freeze and on 0.1.9 release commit
  (`./scripts/check.sh package`, EXIT 0; crate dry-run, wheel build/smoke,
  content + typing smoke green; wheel `eggfetch-0.1.9-cp312-...whl` validated).
Security: PASS (`./scripts/check_security.sh`, EXIT 0; advisories/bans/licenses/
  sources ok; 1251 advisories loaded, 273 crates scanned, no vulnerabilities).
MSRV: PASS — exact 1.89.0 toolchain matrix in extended run.
HTTPX 0.28.1: three consecutive `EGGFETCH_COMPAT_REQUIRED=1 pytest
  crates/eggfetch-python/tests/compat/ -q --strict-markers` runs on the freeze:
  1871 passed (extended), 1871 passed (/tmp/opencode/compat2.txt),
  1871 passed (/tmp/opencode/compat3.txt). API oracle: 71 allowed, zero
  unexplained/stale/resolved-active (compare script EXIT 0).
HTTPX2 2.12.0: same three runs cover the httpx2 facade (shared suite);
  API oracle: 79 allowed, zero unexplained/stale/resolved-active.
Remote CI: GREEN — fix commit 37ab02b run 35429786728; release commit 0ddcecc
  run 35432939352.

eggsearch workaround status: RETAINED — downstream keeps its temporary
  HTML-engine `.decompress(false)` workaround until it adopts a published
  fixed `eggfetch-core` version under its own plan/test pass. No eggsearch
  source change in this corrective.
Downstream handoff: NOT COMPLETE — requires published 0.1.9 on crates.io
  (in order http-connect → core → cli → ffi → python → node, verify with
  `cargo search` between publishes), `v0.1.9` tag, and PyPI rehearsal/publication
  dispatch. Do not tell downstream to remove its workaround based on a git commit.
Known limitations: decoded `bytes_stream()` chunk sizes remain unstable by design
  (tests compare ordered bytes, never chunk counts); H3 stays experimental;
  Node stays experimental prototype; downstream/publish automation unchanged.
Documentation-only descendant SHA: none yet — ledger/profile
  (`plans/httpx-parity-correction-status.md`, `compat/*/profile.toml`,
  `docs/reference/compatibility*.md`, `docs/residual-differences.md`) renewal
  to 37ab02b... is left for the publication pass to keep this freeze binding clean.
```

## Exit criterion

Issue #24 is ready to close when fragmented streaming compressed input is byte-correct for all compiled codecs, the high-level HTTP/1.1 chunked gzip/Brotli reproducer passes, raw mode remains exact, no public/downstream API or timeout/error contract regresses, repository qualification is renewed on one exact executable SHA, and the corrected core is available in a coordinated published patch.
