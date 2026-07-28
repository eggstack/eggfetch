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
warn()  { echo -e "${YELLOW}WARNING:${NC} $*" >&2; }
fail()  { echo -e "${RED}FAIL:${NC} $*" >&2; exit 1; }

# ── Helpers ───────────────────────────────────────────────────────────────

# Track optional skips for the final summary.
SKIPS=()

record_skip() {
    local name="$1" reason="$2"
    SKIPS+=("- ${name}: ${reason}")
    echo -e "${YELLOW}SKIP:${NC} ${name} — ${reason}"
}

# Require a command to exist.
require_command() {
    local cmd="$1"
    if ! command -v "$cmd" &>/dev/null; then
        fail "Required command not found: $cmd"
    fi
}

# Require a file or directory to exist.
require_file() {
    local path="$1"
    if [[ ! -e "$path" ]]; then
        fail "Required file not found: $path"
    fi
}

# ── Python environment ────────────────────────────────────────────────────

PYTHON_BIN="${PYTHON:-python3}"

require_python_env() {
    require_command "$PYTHON_BIN"

    # Verify Python 3.10+
    if ! "$PYTHON_BIN" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)'; then
        fail "Python 3.10+ is required. Found: $("$PYTHON_BIN" --version 2>&1)"
    fi

    # Verify active virtual environment
    if ! "$PYTHON_BIN" -c 'import sys; raise SystemExit(0 if sys.prefix != sys.base_prefix else 1)'; then
        cat >&2 <<'SETUP_GUIDE'
Python validation requires an active virtual environment.
Create one and install test tooling:
  python3 -m venv .venv
  source .venv/bin/activate
  python -m pip install maturin pytest pytest-asyncio
SETUP_GUIDE
        exit 1
    fi

    # Verify required Python modules
    for mod in pytest pytest_asyncio; do
        if ! "$PYTHON_BIN" -c "import $mod" 2>/dev/null; then
            fail "Required Python module not found: $mod. Install into the active venv: python -m pip install $mod"
        fi
    done

    # Verify maturin is available in the active environment
    if ! command -v maturin &>/dev/null; then
        fail "Required command not found: maturin. Install into the active venv: python -m pip install maturin"
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
    maturin develop -m "$REPO_ROOT/crates/eggfetch-python/Cargo.toml"
}

tier1_python_tests() {
    info "Python behavior tests"
    "$PYTHON_BIN" -m pytest "$REPO_ROOT/crates/eggfetch-python/tests/" -q \
        --ignore="$REPO_ROOT/crates/eggfetch-python/tests/compat"
}

tier1_compat_smoke() {
    info "HTTPX compatibility smoke kernel"
    "$PYTHON_BIN" -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_imports.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_client.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_exceptions.py" \
        -v
}

run_tier1() {
    check_tools
    require_python_env
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
    # Verify compatibility dependencies are installed
    if ! "$PYTHON_BIN" -c "import httpx; assert httpx.__version__ == '0.28.1'" 2>/dev/null; then
        cat >&2 <<'SETUP_GUIDE'
Extended HTTPX compatibility dependencies are not installed.
Install them in the active environment:
  python -m pip install -r compat/httpx/0.28.1/requirements.txt
SETUP_GUIDE
        exit 1
    fi
    for mod in requests pytest_timeout; do
        if ! "$PYTHON_BIN" -c "import $mod" 2>/dev/null; then
            fail "Required Python module not found: $mod. Install into the active venv: python -m pip install -r compat/httpx/0.28.1/requirements.txt"
        fi
    done
    (
        cd "$REPO_ROOT"
        EGGFETCH_COMPAT_REQUIRED=1 "$PYTHON_BIN" -m pytest \
            "$REPO_ROOT/crates/eggfetch-python/tests/compat/" \
            -v --strict-markers
    )
}

tier2_api_manifest() {
    info "API manifest comparison"
    require_file "$SCRIPT_DIR/generate_httpx_api_manifest.py"
    require_file "$SCRIPT_DIR/compare_httpx_api_manifest.py"
    require_file "$REPO_ROOT/compat/httpx/0.28.1/allowed-differences.toml"
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT
    (
        cd "$REPO_ROOT"
        "$PYTHON_BIN" scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output "$tmp_dir/eggfetch-manifest.json"
        "$PYTHON_BIN" scripts/generate_httpx_api_manifest.py --package httpx --output "$tmp_dir/httpx-manifest.json"
        "$PYTHON_BIN" scripts/compare_httpx_api_manifest.py \
            --reference "$tmp_dir/httpx-manifest.json" \
            --candidate "$tmp_dir/eggfetch-manifest.json" \
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
    if ! command -v rustup &>/dev/null; then
        record_skip "MSRV" "rustup is not installed"
        return 0
    fi
    if ! rustup toolchain list | grep -Eq '^1\.80([.-]|$)'; then
        record_skip "MSRV" "Rust 1.80 toolchain is not installed"
        return 0
    fi
    rustup run 1.80 cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
}

tier2_docs() {
    info "Documentation checks"
    cargo doc --workspace --all-features --no-deps
    cargo test --doc -p eggfetch-core --all-features
    "$PYTHON_BIN" "$SCRIPT_DIR/check_doc_examples.py"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_doc_links.py"
}

tier2_ffi() {
    info "FFI tests"
    cargo test -p eggfetch-ffi --all-features
}

tier2_resource_monitor() {
    info "Resource regression check"
    cargo build --release -p eggfetch-bench --bin resource_monitor
    ./target/release/resource_monitor
}

tier2_lifecycle() {
    info "Lifecycle tests (timeout, proxy, TLS, shutdown)"
    "$PYTHON_BIN" -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_native_timeout_classification.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_native_proxy_tls.py" \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_shutdown.py" \
        -v
}

tier2_soak() {
    info "Soak tests"
    "$PYTHON_BIN" -m pytest \
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_soak.py" \
        -v
}

tier2_downstream() {
    info "Downstream behavioral fixtures"
    "$PYTHON_BIN" -m pytest "$REPO_ROOT/compat/downstream/behavioral_fixtures/" -v
}

tier2_merge_lossless() {
    info "Lossless merge tests"
    "$PYTHON_BIN" -m pytest "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_merge_lossless.py" -v
}

tier2_benchmarks() {
    info "Benchmarks"
    cargo bench -p eggfetch-bench --bench microbench
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
    # eggfetch-core has no internal deps — dry-run succeeds independently.
    info "  cargo publish --dry-run -p eggfetch-core"
    cargo publish -p eggfetch-core --dry-run

    # Dependent crates require their internal deps to be visible on crates.io.
    # Use cargo package to verify packaging without upload simulation.
    for crate in eggfetch-cli eggfetch-ffi eggfetch-python eggfetch-node; do
        info "  cargo package -p $crate"
        cargo package -p "$crate"
    done
}

tier3_wheel_build() {
    info "Python wheel build"
    local out_dir="$PACKAGE_TMP/wheels"
    mkdir -p "$out_dir"
    maturin build --release -m "$REPO_ROOT/crates/eggfetch-python/Cargo.toml" --out "$out_dir"
}

tier3_wheel_smoke() {
    info "Wheel smoke test"
    require_file "$SCRIPT_DIR/wheel_smoke.py"
    local wheels=("$PACKAGE_TMP"/wheels/*.whl)
    if [[ ${#wheels[@]} -eq 0 ]]; then
        fail "No wheels found in $PACKAGE_TMP/wheels"
    fi
    "$PYTHON_BIN" "$SCRIPT_DIR/wheel_smoke.py" --wheel-dir "$PACKAGE_TMP/wheels"
}

tier3_package_content() {
    info "Package content validation"
    require_file "$SCRIPT_DIR/validate_package_content.py"
    local wheels=("$PACKAGE_TMP"/wheels/*.whl)
    if [[ ${#wheels[@]} -eq 0 ]]; then
        fail "No wheels found for content validation"
    fi
    for whl in "${wheels[@]}"; do
        "$PYTHON_BIN" "$SCRIPT_DIR/validate_package_content.py" "$whl"
    done
}

run_tier3() {
    run_tier1
    PACKAGE_TMP="$(mktemp -d)"
    trap 'rm -rf "$PACKAGE_TMP"' EXIT
    tier3_crate_dry_run
    tier3_wheel_build
    tier3_wheel_smoke
    tier3_package_content
}

# ── Check required tools ──────────────────────────────────────────────────
check_tools() {
    require_command cargo
    require_command rustc
    require_command "$PYTHON_BIN"
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
        echo ""
        info "All routine checks passed."
        ;;
    extended)
        run_tier2
        echo ""
        if [[ ${#SKIPS[@]} -gt 0 ]]; then
            info "Extended validation passed with ${#SKIPS[@]} optional check(s) skipped:"
            for skip in "${SKIPS[@]}"; do
                echo "  $skip"
            done
        else
            info "All extended checks passed."
        fi
        ;;
    package)
        run_tier3
        echo ""
        info "All package checks passed."
        ;;
    *)
        usage
        ;;
esac
