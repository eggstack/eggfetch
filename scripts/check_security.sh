#!/usr/bin/env bash
set -euo pipefail

# Explicit live dependency-security gate. This is intentionally separate from
# routine check.sh: advisory databases are time-dependent network inputs.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

for command_name in cargo-deny cargo-audit; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "FAIL: required security tool is missing: $command_name" >&2
        exit 1
    fi
done

echo "Security preflight UTC: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "cargo-deny: $(cargo deny --version)"
echo "cargo-audit: $(cargo audit --version)"

# Validate the checked-in lockfile before the live advisory checks. cargo-deny
# fetches its configured advisory database, while cargo-audit refuses a stale
# database unless --stale is explicitly supplied (which this gate forbids).
cargo metadata --locked --format-version 1 --no-deps >/dev/null
cargo deny check advisories licenses bans sources
cargo audit --file Cargo.lock
