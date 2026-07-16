#!/usr/bin/env bash
# Record benchmark baselines for regression detection.
#
# Usage:
#   ./scripts/record_baselines.sh
#
# This script runs all benchmark suites and saves the results as baselines.
# The baselines are stored in target/criterion/ and can be used for comparison
# in CI or local development.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "Recording benchmark baselines..."
echo "================================"

# Run microbenchmarks
echo ""
echo "Running microbenchmarks..."
cargo bench -p eggfetch-bench --bench microbench -- --save-baseline main

# Run e2e benchmarks (with timeout)
echo ""
echo "Running e2e benchmarks (timeout: 5 minutes)..."
timeout 300 cargo bench -p eggfetch-bench --bench e2e -- --save-baseline main || echo "Warning: e2e benchmarks timed out"

# Run resource benchmarks
echo ""
echo "Running resource benchmarks..."
cargo bench -p eggfetch-bench --bench resources -- --save-baseline main

echo ""
echo "Baselines recorded successfully!"
echo "Baseline location: target/criterion/"
echo ""
echo "To compare against baselines, run:"
echo "  cargo bench -p eggfetch-bench --bench microbench -- --baseline main"
