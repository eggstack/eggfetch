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
