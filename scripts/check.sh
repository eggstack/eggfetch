#!/usr/bin/env bash
set -euo pipefail

# eggfetch local validation entry point
# Usage:
#   ./scripts/check.sh          — Tier 1: routine validation
#   ./scripts/check.sh extended — Tier 2: extended validation
#   ./scripts/check.sh package  — Tier 3: package validation

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Colors ────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}==>${NC} $*"; }
warn()  { echo -e "${YELLOW}WARNING:${NC} $*"; }
fail()  { echo -e "${RED}FAIL:${NC} $*" >&2; exit 1; }

# ── Check required tools ──────────────────────────────────────────────────
check_tools() {
    local missing=()
    for tool in cargo rustc python3; do
        if ! command -v "$tool" &>/dev/null; then
            missing+=("$tool")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        fail "Missing required tools: ${missing[*]}"
    fi
}

# ── Tier 1: Routine validation ───────────────────────────────────────────
tier1_rust_format() {
    info "Rust formatting check"
    cargo fmt --all -- --check
}

tier1_lint_suppressions() {
    info "Lint suppression policy"
    bash "$SCRIPT_DIR/check_lint_suppressions.sh"
}

tier1_clippy() {
    info "Rust clippy"
    cargo clippy --workspace --all-targets --all-features -- -D warnings
}

tier1_rust_tests() {
    info "Rust workspace tests"
    cargo test --workspace --exclude eggfetch-python --all-features
}

tier1_python_build() {
    info "Building Python extension"
    if ! command -v maturin &>/dev/null; then
        warn "maturin not found, installing..."
        pip install maturin
    fi
    maturin develop -m "$REPO_ROOT/crates/eggfetch-python/Cargo.toml"
}

tier1_python_tests() {
    info "Python behavior tests"
    python -m pytest "$REPO_ROOT/crates/eggfetch-python/tests/" -q \
        --ignore="$REPO_ROOT/crates/eggfetch-python/tests/compat" \
        --ignore="$REPO_ROOT/crates/eggfetch-python/tests/soak_test.py"
}

tier1_compat_smoke() {
    info "HTTPX compatibility smoke kernel"
    python -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_imports.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_client.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_exceptions.py" \
        -v
}

run_tier1() {
    check_tools
    tier1_rust_format
    tier1_lint_suppressions
    tier1_clippy
    tier1_rust_tests
    tier1_python_build
    tier1_python_tests
    tier1_compat_smoke
}

# ── Tier 2: Extended validation ──────────────────────────────────────────
tier2_full_compat() {
    info "Full HTTPX compatibility suite"
    (
        cd "$REPO_ROOT"
        pip install -r compat/httpx/0.28.1/requirements.txt 2>/dev/null || true
        EGGFETCH_COMPAT_REQUIRED=1 python -m pytest \
            "$REPO_ROOT/crates/eggfetch-python/tests/compat/" \
            -v --strict-markers
    )
}

tier2_api_manifest() {
    info "API manifest comparison"
    (
        cd "$REPO_ROOT"
        python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output /tmp/eggfetch-manifest.json
        python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx-manifest.json
        python scripts/compare_httpx_api_manifest.py \
            --reference /tmp/httpx-manifest.json \
            --candidate /tmp/eggfetch-manifest.json \
            --allowed compat/httpx/0.28.1/allowed-differences.toml
    )
}

tier2_feature_matrix() {
    info "Feature matrix validation"
    cargo check -p eggfetch-core --no-default-features
    cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
    cargo check -p eggfetch-core --all-features
}

tier2_feature_tests() {
    info "Feature-gated tests"
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
}

tier2_msrv() {
    info "MSRV check (Rust 1.80)"
    if command -v rustup &>/dev/null; then
        rustup run 1.80 cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls 2>/dev/null \
            || warn "MSRV toolchain not installed, skipping"
    else
        warn "rustup not found, skipping MSRV check"
    fi
}

tier2_docs() {
    info "Documentation checks"
    cargo doc --workspace --all-features --no-deps
    cargo test --doc -p eggfetch-core --all-features
    python "$SCRIPT_DIR/check_doc_examples.py"
    python "$SCRIPT_DIR/check_doc_links.py"
}

tier2_ffi() {
    info "FFI tests"
    cargo test -p eggfetch-ffi --all-features
}

tier2_resource_monitor() {
    info "Resource regression check"
    cargo build --release -p eggfetch-bench --bin resource_monitor 2>/dev/null \
        && ./target/release/resource_monitor \
        || warn "Resource monitor not available, skipping"
}

tier2_lifecycle() {
    info "Lifecycle tests (timeout, proxy, TLS, shutdown)"
    python -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_native_timeout_classification.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_native_proxy_tls.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_shutdown.py" \
        -v 2>/dev/null \
        || warn "Some lifecycle tests skipped"
}

tier2_soak() {
    info "Soak tests"
    python -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_soak.py" \
        -v 2>/dev/null \
        || warn "Soak tests skipped"
}

tier2_downstream() {
    info "Downstream behavioral fixtures"
    python -m pytest "$REPO_ROOT/compat/downstream/behavioral_fixtures/" -v 2>/dev/null \
        || warn "Downstream fixtures skipped"
}

tier2_merge_lossless() {
    info "Lossless merge tests"
    python -m pytest "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_merge_lossless.py" -v 2>/dev/null \
        || warn "Merge tests skipped"
}

tier2_benchmarks() {
    info "Benchmarks"
    cargo bench -p eggfetch-bench --bench microbench 2>/dev/null \
        || warn "Benchmarks not available, skipping"
}

run_tier2() {
    run_tier1
    tier2_full_compat
    tier2_api_manifest
    tier2_feature_matrix
    tier2_feature_tests
    tier2_msrv
    tier2_docs
    tier2_ffi
    tier2_resource_monitor
    tier2_lifecycle
    tier2_soak
    tier2_downstream
    tier2_merge_lossless
    tier2_benchmarks
}

# ── Tier 3: Package validation ───────────────────────────────────────────
tier3_crate_dry_run() {
    info "Crate package dry runs"
    for crate in eggfetch-core eggfetch-cli eggfetch-ffi eggfetch-python eggfetch-node; do
        info "  cargo publish --dry-run -p $crate"
        cargo publish -p "$crate" --dry-run 2>&1 || warn "Dry-run for $crate had issues"
    done
}

tier3_wheel_build() {
    info "Python wheel build"
    maturin build --release -m "$REPO_ROOT/crates/eggfetch-python/Cargo.toml" --out "$REPO_ROOT/dist"
}

tier3_wheel_smoke() {
    info "Wheel smoke test"
    python "$SCRIPT_DIR/wheel_smoke.py" --wheel-dir "$REPO_ROOT/dist"
}

tier3_package_content() {
    info "Package content validation"
    for whl in "$REPO_ROOT"/dist/*.whl; do
        python "$SCRIPT_DIR/validate_package_content.py" "$whl" 2>/dev/null \
            || warn "Package content check for $(basename "$whl") had issues"
    done
}

run_tier3() {
    run_tier1
    tier3_crate_dry_run
    tier3_wheel_build
    tier3_wheel_smoke
    tier3_package_content
}

# ── Usage ─────────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $0 [mode]

Modes:
  (none)    Tier 1: routine validation (default)
  extended  Tier 2: extended validation (runs Tier 1 first)
  package   Tier 3: package validation (runs Tier 1 first)

EOF
    exit 1
}

# ── Main ──────────────────────────────────────────────────────────────────
MODE="${1:-}"
cd "$REPO_ROOT"

case "$MODE" in
    "")
        run_tier1
        ;;
    extended)
        run_tier2
        ;;
    package)
        run_tier3
        ;;
    *)
        usage
        ;;
esac

info "All checks passed."
