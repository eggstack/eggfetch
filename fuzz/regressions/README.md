# Fuzz Regression Corpus

This directory contains minimized crash reproducers found by fuzz testing.
Each subdirectory corresponds to a fuzz target.

## Structure

```
regressions/
  fuzz_retry/
    crash-<hash>    # Minimized reproducer input
  fuzz_headers/
    ...
```

## Adding a regression

When a fuzzer finds a crash:

1. **Minimize** the input:
   ```sh
   cd fuzz
   cargo fuzz tmin <target> artifacts/<target>/crash-<hash>
   ```

2. **Copy** the minimized artifact to `regressions/<target>/`:
   ```sh
   mkdir -p regressions/<target>
   cp artifacts/<target>/crash-<hash> regressions/<target>/
   ```

3. **Write a regression test** in the relevant module test file that
   exercises the same code path with the minimized input.

4. **Verify** the regression test catches the bug before the fix:
   ```sh
   cargo test -p eggfetch-core --all-features -- <test_name>
   ```

## Reproducing a crash

```sh
cd fuzz
cargo fuzz run <target> regressions/<target>/crash-<hash>
```

## CI behavior

The CI `fuzz-smoke` job runs each target for 15 seconds. It does NOT
check the regressions directory directly — regression tests in Rust
unit/integration tests cover these cases.

For deeper fuzzing (hours/days), run locally or in a scheduled CI job.

## Severity assessment

Classify each crash by severity before fixing:

| Severity | Criteria | Example |
|----------|----------|---------|
| **Critical** | Memory safety violation, reachable `unsafe`, data corruption | N/A (workspace forbids `unsafe`) |
| **High** | Panic on arbitrary input reachable from public API | `BackoffPolicy::delay()` panic on NaN factor |
| **Medium** | Panic only reachable with unusual internal state or feature-gated code | Multipart encoder panic on empty boundary |
| **Low** | Resource exhaustion, unbounded allocation, or hang without panic | Decompressor OOM on adversarial input |
| **Informational** | Assertion failure in test-only code, non-observable behavior difference | Round-trip property violation |

### Triage process

1. Reproduce the crash: `cargo fuzz run <target> regressions/<target>/crash-<hash>`
2. Determine the code path: is it reachable from public API?
3. Assign severity from the table above.
4. Write a regression test at the appropriate severity level.
5. Fix the root cause — do not suppress the symptom.
