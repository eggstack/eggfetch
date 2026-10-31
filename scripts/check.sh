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
Create one and install the pinned test tooling:
  python3 -m venv .venv
  source .venv/bin/activate
  python -m pip install -r scripts/ci-requirements.txt
SETUP_GUIDE
        exit 1
    fi

    # Verify required Python modules
    for mod in pytest pytest_asyncio mypy; do
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

tier1_lint_suppression_self_test() {
    info "Lint suppression guard self-test (guard verdicts only — not a tree scan)"
    bash "$SCRIPT_DIR/check_lint_suppressions.sh" --self-test
}

tier1_h3_runner_self_test() {
    info "H3 qualification runner self-test (runner logic only — not an H3 qualification result)"
    # Pure unit self-test of scripts/h3_qualification.py's manifest and corpus
    # validation. It opens no socket, resolves no name, needs no QUIC peer,
    # and requires neither the http3 feature nor the http3 corpus evidence.
    # H3 stays experimental: this asserts the runner's logic, not conformance.
    "$PYTHON_BIN" "$SCRIPT_DIR/test_h3_qualification.py"
}

tier1_adapter_features() {
    info "Adapter feature ownership"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_adapter_features.py"
}

tier1_downstream_artifact_self_test() {
    info "Downstream artifact producer self-test (manifest schema only — not a portfolio result)"
    # Checks that the manifest this repository produces is readable by the
    # downstream consumer, and that a hash mismatch is rejected. It builds no
    # wheel, opens no socket, and asserts nothing about the portfolio.
    "$PYTHON_BIN" "$SCRIPT_DIR/build_downstream_artifact.py" --self-test
}

tier1_release_validation() {
    info "Release version/ref validation"
    "$PYTHON_BIN" "$SCRIPT_DIR/test_validate_release_versions.py"
}

tier1_clippy() {
    info "Rust clippy"
    cargo clippy --workspace --all-targets --all-features -- -D warnings
}

tier1_rust_tests() {
    info "Rust workspace tests"
    # Resource-stabilization tests compare process RSS and must not run beside
    # other workspace tests that allocate concurrently.
    cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
}

tier1_rust_api_contracts() {
    info "Stable Rust public contract fixtures"
    # Lean profiles retain private helpers for other supported profiles. The
    # CI-wide -D warnings flag turns those profile-specific dead-code warnings
    # into errors; clippy and the all-features workspace checks own warning
    # enforcement, while this matrix owns stable public compile coverage.
    # The matrix spans all eight supported profiles: default, the four
    # compatibility aliases (http1, http2, standard-http1, standard-http2),
    # the two native slices (native-http1, native-http2), and all-features.
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features http1,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features http2,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http2,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http1,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http2,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --all-features -- --test-threads=1
    # The lean profile's behavioral suites must run under the lean profile:
    # under the default profile they would exercise the redirect path instead
    # of the lean single-attempt path. The gate names the two portable
    # targets explicitly; the whole-suite lean compile coverage is owned by
    # tier1_lean_profile_compile below.
    RUSTFLAGS= cargo test -p eggfetch-core --test lean_policy_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
    RUSTFLAGS= cargo test -p eggfetch-core --test lean_route_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
}

tier1_profile_whole_suite_compile() {
    info "Feature-isolated profile whole-suite compile"
    # Each feature-isolated profile must compile the whole test suite, not only
    # the portable behavioral targets. Every test target that cannot compile
    # under a profile carries a file-level feature gate expressing what that
    # file needs, so these gates stay green for the whole suite.
    #
    # The bare RUSTFLAGS= matches tier1_rust_api_contracts: the lean and native
    # profiles retain private helpers for other supported profiles, and CI's
    # -D warnings turns profile-specific dead-code warnings into errors. Clippy
    # and the all-features workspace checks own warning enforcement.
    #
    # These are compiles, not runs.
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http2,tls-rustls --all-targets
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls --all-targets
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features native-http2,tls-rustls --all-targets
}

tier1_python_build() {
    info "Building Python extension"
    maturin develop -m "$REPO_ROOT/crates/eggfetch-python/Cargo.toml"
}

tier1_native_api() {
    info "Native Python API manifest"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_native_python_api.py" --self-test
}

tier1_python_typing_surface() {
    info "Python typing surface"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_python_typing_surface.py" --self-test
}

tier1_python_typing() {
    info "Python typing fixture"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_python_typing.py"
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
        "$REPO_ROOT/crates/eggfetch-python/tests/compat/test_corrective_kernel.py" \
        -v
}

tier1_node_binding() {
    info "Node binding (experimental prototype)"
    # Rust-side surface is also covered by the workspace tests above; this
    # explicit step keeps the prototype's compiled surface visible even if
    # the workspace test selection ever changes.
    cargo test -p eggfetch-node --all-features
    # JS surface: optional. The prototype loader (index.js) requires a
    # manually placed ./eggfetch.node artifact, so absence is an explicit
    # skip, not a failure.
    if ! command -v node &>/dev/null; then
        record_skip "Node JS surface" "node is not installed"
        return 0
    fi
    local node_dir="$REPO_ROOT/crates/eggfetch-node"
    if [[ ! -f "$node_dir/eggfetch.node" ]]; then
        record_skip "Node JS surface" "native artifact not built ($node_dir/eggfetch.node missing; run 'npm run build' in crates/eggfetch-node and place the artifact)"
        return 0
    fi
    (cd "$node_dir" && node test.js)
}

run_tier1() {
    check_tools
    require_python_env
    # The H3 runner self-test runs first so a broken runner is caught in under
    # a second, before the tier spends time on cargo work. It is a runner
    # self-test only and asserts nothing about H3 conformance.
    tier1_h3_runner_self_test
    tier1_rust_format
    tier1_lint_suppression_self_test
    tier1_lint_suppressions
    tier1_adapter_features
    tier1_downstream_artifact_self_test
    tier1_release_validation
    tier1_clippy
    tier1_rust_tests
    tier1_rust_api_contracts
    tier1_profile_whole_suite_compile
    tier1_python_build
    tier1_native_api
    tier1_python_typing_surface
    tier1_python_typing
    tier1_python_tests
    tier1_compat_smoke
    tier1_node_binding
}

# ── Tier 2: Extended validation ──────────────────────────────────────────
tier2_full_compat() {
    info "Full versioned HTTPX compatibility suites"
    # Verify compatibility dependencies are installed
    if ! "$PYTHON_BIN" -c "import httpx; assert httpx.__version__ == '0.28.1'" 2>/dev/null; then
        cat >&2 <<'SETUP_GUIDE'
Extended HTTPX compatibility dependencies are not installed.
Install them in the active environment:
  python -m pip install -r compat/httpx/0.28.1/requirements.txt
  python -m pip install -r compat/httpx2/2.12.0/requirements.txt
SETUP_GUIDE
        exit 1
    fi
    if ! "$PYTHON_BIN" -c "import httpx2; assert httpx2.__version__ == '2.12.0'" 2>/dev/null; then
        cat >&2 <<'SETUP_GUIDE'
Extended HTTPX2 compatibility dependencies are not installed.
Install them in the active environment:
  python -m pip install -r compat/httpx2/2.12.0/requirements.txt
SETUP_GUIDE
        exit 1
    fi
    for mod in requests pytest_timeout; do
        if ! "$PYTHON_BIN" -c "import $mod" 2>/dev/null; then
            fail "Required Python module not found: $mod. Install into the active venv: python -m pip install -r compat/httpx/0.28.1/requirements.txt"
        fi
    done
    local profile
    for profile in "httpx/0.28.1" "httpx2/2.12.0"; do
        info "Full compatibility profile: ${profile}"
        (
            cd "$REPO_ROOT"
            EGGFETCH_COMPAT_PROFILE="$profile" EGGFETCH_COMPAT_REQUIRED=1 \
                "$PYTHON_BIN" -m pytest \
                "$REPO_ROOT/crates/eggfetch-python/tests/compat/" \
                -v --strict-markers
        )
    done
}

tier2_api_manifest() {
    info "API manifest comparison"
    require_file "$SCRIPT_DIR/generate_httpx_api_manifest.py"
    require_file "$SCRIPT_DIR/compare_httpx_api_manifest.py"
    require_file "$REPO_ROOT/compat/httpx/0.28.1/reference-api.json"
    require_file "$REPO_ROOT/compat/httpx/0.28.1/allowed-differences.toml"
    require_file "$REPO_ROOT/compat/httpx2/2.12.0/reference-api.json"
    require_file "$REPO_ROOT/compat/httpx2/2.12.0/allowed-differences.toml"
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    (
        trap 'rm -rf "$tmp_dir"' EXIT
        cd "$REPO_ROOT"
        "$PYTHON_BIN" scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output "$tmp_dir/eggfetch-manifest.json"
        "$PYTHON_BIN" scripts/compare_httpx_api_manifest.py \
            --reference compat/httpx/0.28.1/reference-api.json \
            --candidate "$tmp_dir/eggfetch-manifest.json" \
            --allowed compat/httpx/0.28.1/allowed-differences.toml
        "$PYTHON_BIN" scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx2 --output "$tmp_dir/eggfetch-httpx2-manifest.json"
        "$PYTHON_BIN" scripts/compare_httpx_api_manifest.py \
            --reference compat/httpx2/2.12.0/reference-api.json \
            --candidate "$tmp_dir/eggfetch-httpx2-manifest.json" \
            --allowed compat/httpx2/2.12.0/allowed-differences.toml
    )
}

tier2_feature_matrix() {
    info "Feature matrix validation"
    # Lean profiles retain private helpers for other supported profiles. The
    # CI-wide -D warnings flag turns those profile-specific dead-code warnings
    # into errors; clippy and the all-features workspace checks own warning
    # enforcement, while this matrix owns profile compile coverage. Bare
    # RUSTFLAGS= overrides CI's -D warnings for the profile-isolated checks
    # only; the all-features check below keeps the flag.
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
    cargo check -p eggfetch-core --all-features
}

tier2_rust_public_api() {
    info "Rust public API oracle"
    require_command cargo-public-api
    require_command cargo-semver-checks
    "$PYTHON_BIN" "$SCRIPT_DIR/test_rust_public_api_oracle.py"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_rust_public_api.py"
}

tier2_feature_tests() {
    info "Feature-gated tests"
    # Pool and resource fixtures depend on loopback scheduling. Keep the
    # feature profiles serialized just like the all-features workspace suite.
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip -- --test-threads=1
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli -- --test-threads=1
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd -- --test-threads=1
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate -- --test-threads=1
    cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy -- --test-threads=1
}

tier2_msrv() {
    local msrv_toolchain="1.89.0"
    info "MSRV check (Rust ${msrv_toolchain})"
    require_command rustup

    if ! rustup run "$msrv_toolchain" rustc --version &>/dev/null; then
        fail "Rust ${msrv_toolchain} is required for the MSRV gate. Install it with: rustup toolchain install ${msrv_toolchain} --profile minimal"
    fi

    local checks=(
        "--locked -p eggfetch-core --no-default-features"
        "--locked -p eggfetch-core --no-default-features --features http1"
        "--locked -p eggfetch-core --no-default-features --features http1,tls-rustls"
    )
    local warning_checks=(
        "--locked -p eggfetch-core --all-features"
        "--locked --workspace --all-targets --all-features"
    )
    local check_args
    # Lean profiles retain private helpers for other supported profiles. The
    # CI-wide -D warnings flag turns those profile-specific dead-code warnings
    # into errors; clippy and the all-features workspace checks own warning
    # enforcement, while these profile-isolated checks own compile coverage.
    # Bare RUSTFLAGS= overrides CI's -D warnings for the profile-isolated
    # checks only; the all-features and workspace checks keep the flag.
    for check_args in "${checks[@]}"; do
        info "MSRV: rustup run ${msrv_toolchain} cargo check ${check_args}"
        # shellcheck disable=SC2086
        RUSTFLAGS= rustup run "$msrv_toolchain" cargo check $check_args
    done
    for check_args in "${warning_checks[@]}"; do
        info "MSRV: rustup run ${msrv_toolchain} cargo check ${check_args}"
        # shellcheck disable=SC2086
        rustup run "$msrv_toolchain" cargo check $check_args
    done
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
    local artifact_manifest="$REPO_ROOT/target/downstream-qualification/artifact-manifest.json"
    if [[ ! -f "$artifact_manifest" ]]; then
        # This portfolio is release-blocking (docs/verification-policy.md), so
        # it fails closed instead of recording a skip. The skipped message used
        # to point at a "downstream qualification workflow" that no longer
        # exists in .github/workflows/; only the artifact and the scripts that
        # consume it are named here.
        cat >&2 <<DOWNSTREAM_GUIDE
FAIL: the downstream compatibility portfolio is release-blocking and its
      artifact manifest is missing. It is not optional and cannot be skipped.

  Missing artifact manifest:
    ${artifact_manifest}

  The manifest must list two wheels with verified SHA-256 hashes:
    - role "eggfetch"                     the eggfetch wheel built from this checkout
    - role "httpx-controlled-replacement"  the controlled httpx replacement wheel

  Network access: this gate needs it. The isolated runner creates a virtual
  environment, downloads the pinned downstream packages and their wheels from
  PyPI, and verifies each package's source hash against
  compat/downstream/manifest.toml. The package test suites themselves run
  with outbound network disabled.

  Operator obligation: build the eggfetch wheel and the controlled httpx
  replacement wheel, write their paths and SHA-256 hashes into the manifest
  at the path above, and re-run ./scripts/check.sh extended.

  This repository produces the manifest with one command:

    python scripts/build_downstream_artifact.py

  It builds both wheels from this checkout and writes the manifest at the
  path above. The gate still fails closed when the manifest is missing:
  producing it is a separate, explicit step, not something the gate does
  for you. The procedure is documented in compat/downstream/README.md
  ("Update Process"); the consumer is scripts/run_downstream_compat.py.
DOWNSTREAM_GUIDE
        fail "Downstream behavioral fixtures — release-blocking artifact manifest missing"
    fi
    "$PYTHON_BIN" scripts/run_downstream_compat.py \
        --artifact-manifest "$artifact_manifest" \
        --required-only
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
    tier2_rust_public_api
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

# Find exactly one wheel in a directory. Fails on zero or multiple wheels.
find_single_wheel() {
    local wheel_dir="$1"
    local wheels=()

    shopt -s nullglob
    wheels=("$wheel_dir"/*.whl)
    shopt -u nullglob

    if [[ ${#wheels[@]} -ne 1 ]]; then
        fail "Expected exactly one wheel in $wheel_dir; found ${#wheels[@]}"
    fi

    printf '%s\n' "${wheels[0]}"
}

# Validate that all publishable internal dependencies have concrete version
# requirements using cargo metadata (portable, no grep, no GNU extensions).
validate_publishable_dependencies() {
    require_command "$PYTHON_BIN"
    "$PYTHON_BIN" "$SCRIPT_DIR/validate_publishable_internal_dependencies.py"
}

tier3_crate_packages() {
    info "Crate package validation"

    # eggfetch-http-connect is the dependency leaf — full dry-run succeeds independently.
    info "  cargo publish --dry-run -p eggfetch-http-connect"
    cargo publish -p eggfetch-http-connect --dry-run

    # Dependent crates cannot run cargo package --no-verify or cargo publish --dry-run
    # because their internal dependencies (eggfetch-http-connect, eggfetch-core,
    # eggfetch-ffi) are not yet on crates.io. This is a Cargo limitation.
    # The local fallback validates:
    #   1. cargo package --list succeeds (manifest/package inclusion)
    #   2. Cargo.toml contains a non-path version for every publishable dependency
    #   3. The crate builds and tests pass through Tier 1 validation
    # Full cargo publish --dry-run runs at publication time, after deps are visible.
    for crate in eggfetch-core eggfetch-cli eggfetch-ffi eggfetch-python eggfetch-node; do
        info "  cargo package --list -p $crate"
        cargo package --list -p "$crate" >/dev/null
    done
    info "  Validating publishable internal dependency versions"
    validate_publishable_dependencies
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
    "$PYTHON_BIN" "$SCRIPT_DIR/wheel_smoke.py" --wheel "$PACKAGE_WHEEL"
}

tier3_package_content() {
    info "Package content validation"
    require_file "$SCRIPT_DIR/validate_package_content.py"
    "$PYTHON_BIN" "$SCRIPT_DIR/validate_package_content.py" "$PACKAGE_WHEEL"
}

tier3_wheel_typing() {
    info "Installed wheel typing smoke"
    require_file "$SCRIPT_DIR/check_wheel_typing.py"
    "$PYTHON_BIN" "$SCRIPT_DIR/check_wheel_typing.py" --wheel "$PACKAGE_WHEEL"
}

run_tier3() {
    run_tier1
    PACKAGE_TMP="$(mktemp -d)"
    trap 'rm -rf "$PACKAGE_TMP"' EXIT
    tier3_crate_packages
    tier3_wheel_build
    PACKAGE_WHEEL="$(find_single_wheel "$PACKAGE_TMP/wheels")"
    tier3_wheel_smoke
    tier3_package_content
    tier3_wheel_typing
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
