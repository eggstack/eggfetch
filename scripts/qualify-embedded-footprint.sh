#!/usr/bin/env bash
# Embedded consumer footprint qualification (manual, not CI).
#
# Builds the fixed minimal/JSON/default profiles for eggfetch-core and reqwest,
# gathers `cargo tree` evidence, measures release artifact sizes before and
# after explicit `strip`, and writes a machine-readable result plus a
# human-readable summary.
#
# This script is intentionally manual/bounded: no CI gate, no dashboard, no
# scheduled workflow. Run at release/architecture review points and copy the
# headline numbers into `docs/architecture/embedded-footprint.md`.
#
# Usage:
#   scripts/qualify-embedded-footprint.sh [--output-dir DIR] [--skip-build]
#
# Defaults:
#   output dir: /tmp/eggfetch-embedded-footprint
#   Each profile builds with an isolated CARGO_TARGET_DIR so dependency graphs
#   reflect an ordinary downstream binary and sizes are reproducible.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EMBEDDED_DIR="$REPO_ROOT/qualification/embedded"

OUTPUT_DIR="/tmp/eggfetch-embedded-footprint"
SKIP_BUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    -h|--help)
      sed -n '1,30p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

mkdir -p "$OUTPUT_DIR/trees"

# Profiles: "label|manifest-dir|features|binary-name"
# WebPKI variants are the deterministic embedded primary; `-native` variants
# add the `native` fixture feature (eggfetch: tls-native-roots; reqwest:
# rustls-tls-native-roots alongside webpki, approximating fallback).
# Defaults are informational (Profile C): each library's ordinary features.
PROFILES=(
  "eggfetch-min|eggfetch-min||eggfetch-min"
  "eggfetch-min-native|eggfetch-min|native|eggfetch-min"
  "eggfetch-json|eggfetch-json||eggfetch-json"
  "eggfetch-json-native|eggfetch-json|native|eggfetch-json"
  "reqwest-min|reqwest-min||reqwest-min"
  "reqwest-min-native|reqwest-min|native|reqwest-min"
  "reqwest-json|reqwest-json||reqwest-json"
  "reqwest-json-native|reqwest-json|native|reqwest-json"
  "eggfetch-default|eggfetch-default||eggfetch-default"
  "reqwest-default|reqwest-default||reqwest-default"
)

EGGFETCH_SHA="$(git -C "$REPO_ROOT" rev-parse HEAD)"
RUSTC_VERSION="$(rustc --version)"
CARGO_VERSION="$(cargo --version)"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/{print $2}')"
LINKER_INFO="$(cc --version 2>/dev/null | head -n 1 || echo 'cc not available')"

echo "==> eggfetch embedded footprint qualification"
echo "    repo SHA:      $EGGFETCH_SHA"
echo "    rustc:         $RUSTC_VERSION"
echo "    cargo:         $CARGO_VERSION"
echo "    target:        $TARGET_TRIPLE"
echo "    linker:        $LINKER_INFO"
echo "    output:        $OUTPUT_DIR"

# Record reqwest resolution once (all reqwest fixtures pin the same version).
REQWEST_VERSION="$(cargo tree --manifest-path "$EMBEDDED_DIR/reqwest-min/Cargo.toml" -i reqwest --prefix none 2>/dev/null | head -n 5 || true)"
echo "    reqwest tree:  $(echo "$REQWEST_VERSION" | tr '\n' '; ')"

RESULT_JSON="$OUTPUT_DIR/results.json"
SUMMARY_TXT="$OUTPUT_DIR/summary.txt"

# Gather cargo tree evidence for every profile (fast, no build).
for entry in "${PROFILES[@]}"; do
  IFS='|' read -r label mandir features _bin <<< "$entry"
  manifest="$EMBEDDED_DIR/$mandir/Cargo.toml"
  extra_args=()
  if [[ -n "$features" ]]; then
    extra_args+=(--features "$features")
  fi
  echo "==> trees: $label"
  cargo tree --manifest-path "$manifest" "${extra_args[@]}" > "$OUTPUT_DIR/trees/$label.tree.txt" 2>/dev/null
  cargo tree --manifest-path "$manifest" "${extra_args[@]}" -e features > "$OUTPUT_DIR/trees/$label.features.txt" 2>/dev/null || true
  cargo tree --manifest-path "$manifest" "${extra_args[@]}" -d > "$OUTPUT_DIR/trees/$label.duplicates.txt" 2>/dev/null || true
  cargo tree --manifest-path "$manifest" "${extra_args[@]}" --prefix none 2>/dev/null | sort -u > "$OUTPUT_DIR/trees/$label.packages.txt" || true
done

# Build release artifacts with isolated target dirs (clean per profile).
# Records: wall-clock seconds, target-dir KB, binary bytes, stripped bytes.
TMP_RESULTS="$OUTPUT_DIR/.results.tmp.jsonl"
rm -f "$TMP_RESULTS"
touch "$TMP_RESULTS"

if [[ "$SKIP_BUILD" -eq 1 ]]; then
  echo "==> --skip-build: tree evidence only, no release builds"
else
  for entry in "${PROFILES[@]}"; do
    IFS='|' read -r label mandir features bin <<< "$entry"
    manifest="$EMBEDDED_DIR/$mandir/Cargo.toml"
    target_dir="$OUTPUT_DIR/target-$label"
    rm -rf "$target_dir"
    mkdir -p "$target_dir"
    build_args=(build --release --manifest-path "$manifest")
    if [[ -n "$features" ]]; then
      build_args+=(--features "$features")
    fi
    echo "==> build --release: $label (target $target_dir)"
    start_s="$SECONDS"
    CARGO_TARGET_DIR="$target_dir" cargo "${build_args[@]}" 2>&1 | tail -n 3
    duration_s="$((SECONDS - start_s))"
    bin_path="$target_dir/release/$bin"
    if [[ ! -f "$bin_path" ]]; then
      echo "FAIL: expected binary missing: $bin_path" >&2
      exit 1
    fi
    # Smoke without I/O (client construction + serde round-trip only).
    EGGFETCH_FIXTURE_NOOP=1 "$bin_path" >/dev/null
    size_bytes="$(stat -c %s "$bin_path")"
    stripped_copy="$target_dir/release/$bin.stripped"
    cp "$bin_path" "$stripped_copy"
    strip "$stripped_copy" 2>/dev/null || true
    stripped_bytes="$(stat -c %s "$stripped_copy")"
    target_kb="$(du -sk "$target_dir" | cut -f1)"
    # cargo bloat is optional and never mandatory.
    bloat_note=""
    if command -v cargo-bloat >/dev/null 2>&1; then
      bloat_note="$(CARGO_TARGET_DIR="$target_dir" cargo bloat --release --manifest-path "$manifest" --crates 2>/dev/null | head -n 25 | tr '\n' '; ' || true)"
    fi
    python3 - "$TMP_RESULTS" "$label" "$mandir" "$features" "$bin" "$duration_s" "$target_kb" "$size_bytes" "$stripped_bytes" <<'PY'
import json, sys
out, label, mandir, features, binary, duration_s, target_kb, size_bytes, stripped_bytes = sys.argv[1:10]
rec = {
    "profile": label,
    "manifest_dir": mandir,
    "features": features,
    "binary": binary,
    "build_seconds": int(duration_s),
    "target_dir_kb": int(target_kb),
    "size_bytes": int(size_bytes),
    "stripped_bytes": int(stripped_bytes),
}
with open(out, "a") as f:
    f.write(json.dumps(rec) + "\n")
PY
    # shellcheck disable=SC2086
    echo "    $label: ${size_bytes}B -> stripped ${stripped_bytes}B (${duration_s}s, target ${target_kb}KB) $bloat_note"
  done
fi

# Assemble results.json with toolchain metadata + per-profile records.
python3 - "$TMP_RESULTS" "$RESULT_JSON" "$EGGFETCH_SHA" "$RUSTC_VERSION" "$CARGO_VERSION" "$TARGET_TRIPLE" "$LINKER_INFO" <<'PY'
import json, sys
tmp_path, out_path, sha, rustc_v, cargo_v, triple, linker = sys.argv[1:8]
records = []
try:
    with open(tmp_path) as f:
        for line in f:
            line = line.strip()
            if line:
                records.append(json.loads(line))
except FileNotFoundError:
    pass
doc = {
    "schema_version": 1,
    "generator": "scripts/qualify-embedded-footprint.sh",
    "eggfetch_sha": sha,
    "rustc": rustc_v,
    "cargo": cargo_v,
    "target": triple,
    "linker": linker,
    "release_profile": {"lto": "thin", "codegen-units": 1, "strip": False, "debug": False, "panic": "unwind"},
    "note": "strip=false in fixture manifests; stripped_bytes is an explicit `strip` copy. Build with isolated CARGO_TARGET_DIR per profile.",
    "profiles": records,
}
with open(out_path, "w") as f:
    json.dump(doc, f, indent=2)
print(f"wrote {out_path} ({len(records)} profiles)")
PY

# Human-readable summary.
{
  echo "eggfetch embedded footprint — summary"
  echo "SHA: $EGGFETCH_SHA | $RUSTC_VERSION | $CARGO_VERSION | $TARGET_TRIPLE"
  echo ""
  printf "%-24s %14s %14s %10s\n" "profile" "bytes" "stripped" "build_s"
  python3 - "$TMP_RESULTS" <<'PY'
import json, sys
try:
    with open(sys.argv[1]) as f:
        for line in f:
            r = json.loads(line)
            print(f"{r['profile']:24s} {r['size_bytes']:14d} {r['stripped_bytes']:14d} {r['build_seconds']:10d}")
except FileNotFoundError:
    print("(no builds; --skip-build)")
PY
  echo ""
  echo "Unique package counts (cargo tree --prefix none | sort -u | wc -l):"
  for t in "$OUTPUT_DIR"/trees/*.packages.txt; do
    base="$(basename "$t" .packages.txt)"
    count="$(wc -l < "$t" | tr -d ' ')"
    echo "  $base: $count"
  done
  echo ""
  echo "Full trees: $OUTPUT_DIR/trees/ | machine-readable: $OUTPUT_DIR/results.json"
} | tee "$SUMMARY_TXT"

echo "==> done: $RESULT_JSON + $SUMMARY_TXT"
