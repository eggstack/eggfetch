# Production Track B Plan: Robustness and Fuzzing

## Objective

Establish systematic fuzzing, property testing, and adverse-condition testing for parser, state-machine, and protocol boundaries. The goal is to find crashes, panics, hangs, resource leaks, and semantic inconsistencies before public release.

## Scope

Add fuzz targets for:

- URL/query normalization
- headers and multi-value conversion
- cookie parsing/matching
- redirect resolution and method rewriting
- multipart boundary/encoder generation
- compression/decompression streams and limits
- proxy response parsing and CONNECT authority
- timeout/deadline state transitions
- retry decision state machine
- TLS configuration parsing
- HTTP/2 frame/stream integration where practical

## Tooling

Use `cargo-fuzz`/libFuzzer for core Rust targets. Add property tests with proptest where state generation is easier than byte fuzzing. Evaluate OSS-Fuzz after targets are stable and fast.

## Harness rules

- no external network dependency
- deterministic clocks/randomness where possible
- bounded memory and execution time
- preserve crashing inputs as regression corpus
- exercise feature-gated configurations separately

## Key properties

Examples:

- no panic on arbitrary header/cookie/proxy input
- serialize/parse round-trip where defined
- redirect target never leaks stripped credentials
- multipart output is syntactically complete and length calculation matches emitted bytes
- decompression limits terminate before unbounded allocation
- timeout arithmetic never underflows or extends deadlines
- retry attempts never exceed budget
- parser limits reject oversized proxy input

## Stateful/adverse tests

Add deterministic tests for:

- cancellation at every body/pipeline stage
- partial reads/writes
- connection reset mid-headers/body
- malformed chunking/content lengths
- nested redirects/retries
- slowloris-like proxy responses
- file/stream errors in multipart
- interpreter shutdown/drop paths for Python streaming

Use loom selectively for concurrency primitives if a focused invariant warrants the cost.

## Corpus and CI

Maintain seed corpora and regression fixtures in-tree or downloadable artifacts. Run short fuzz smoke jobs in CI and longer scheduled/manual jobs. Add sanitizers where supported.

## Crash handling

Every discovered issue should produce:

- minimized reproducer
- regression test
- severity assessment
- fix with no silent input acceptance unless intended

## Acceptance criteria

- all high-risk parsers/state machines have targets
- CI runs bounded smoke fuzzing/property tests
- longer fuzzing is reproducible
- crash corpus is retained
- no known panics, unbounded allocations, or hangs remain in supported inputs
