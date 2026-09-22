# Verification & Qualification Skill

Use this skill for any change that touches executable code, tests, build/validation
scripts, packaging config, or compatibility claims. It is the single entry point
for Tier 1/2/3 gates and the exact-SHA qualification rule; language- and
crate-specific skills defer to it for validation.

## Tiers (normative: `docs/verification-policy.md`, entry point `scripts/check.sh`)

```sh
./scripts/check.sh          # Tier 1: required before every commit (CI repeats it on ubuntu-latest)
./scripts/check.sh extended # Tier 2: before release (full compat, API oracle, feature matrix, MSRV, docs, FFI, soak, bench)
./scripts/check.sh package  # Tier 3: before publish (crate packaging + wheel build/smoke)
./scripts/check_security.sh # Live RustSec/license/source preflight before publication
```

- `check.sh` refuses to run outside an active venv with Python 3.10+ and pinned
  tooling in `scripts/ci-requirements.txt`.
- Never parallelize Rust workspace tests (`--test-threads=1`); workspace tests
  exclude `eggfetch-python` (PyO3 builds separately via `maturin develop`).
- After changing `crates/eggfetch-python` Rust code, rebuild before testing:
  `maturin develop -m crates/eggfetch-python/Cargo.toml`.
- Tier 1 Node JS surface (`node test.js`) is an explicit SKIP when `node` or
  `crates/eggfetch-node/eggfetch.node` is absent, not a failure.
- `qualification/` fixtures and H3 (experimental) are manual, never Tier 1 gates.
- `scripts/performance_benchmark.py` is non-CI timing evidence only.
- Tier 2 requires the exact Rust 1.89.0 toolchain
  (`rustup toolchain install 1.89.0 --profile minimal`); it fails, never skips.
  The extended Rust public-surface oracle requires `cargo-public-api 0.52.0`,
  `cargo-semver-checks 0.49.0`, and pinned nightly `nightly-2026-05-07`
  (see `compat/rust-public-api/README.md`).

## Exact-SHA qualification rule (most important)

- The live ledger `plans/httpx-parity-correction-status.md` plus
  `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`
  record the exact executable SHA the HTTPX 0.28.1 / httpx2 2.12.0 Stage C
  qualifications are bound to.
- Any change to executable inputs (Rust sources, tests, build/validation
  scripts, packaging config, compat fixtures) invalidates the binding and
  requires a fresh exact-SHA requalification from a new freeze. Docs-only
  commits do not invalidate it.
- Never hardcode a live SHA outside the canonical qualification records
  (`docs/residual-differences.md`, `docs/reference/compatibility.md`,
  `docs/reference/compatibility-stage-decision.md`). Everywhere else —
  including all skills — reference the live ledger.
- Completed plans in `plans/` are historical records, not active gates
  (verification-policy principle 9). Never hand-edit generated API manifests;
  regenerate via `scripts/generate_httpx_api_manifest.py` +
  `scripts/compare_httpx_api_manifest.py`.

## Focused equivalents (same flags `check.sh` uses)

```sh
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
cargo test -p eggfetch-core --all-features <filter> -- --test-threads=1
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
python scripts/check_doc_examples.py
python scripts/check_doc_links.py
```

## Architecture References

- Verification policy: `docs/verification-policy.md`
- Build & CI deep dive: `docs/architecture/build-ci.md`
- Compat suites: `docs/architecture/testing-fuzzing.md`
- Live ledger: `plans/httpx-parity-correction-status.md`
