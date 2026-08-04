#!/usr/bin/env bash
# check_lint_suppressions.sh — Fail CI when forbidden broad lint suppressions
# are introduced in production or test Rust source files.
#
# Allowed blanket suppressions (test-only, narrow):
#   - missing_docs, dead_code, unused_mut (test hygiene)
#   - clippy::large_futures (async chain ergonomics in tests)
#   - clippy::missing_panics_doc, clippy::redundant_closure_for_method_calls,
#     clippy::inefficient_to_string, clippy::manual_let_else,
#     clippy::single_char_pattern, clippy::match_same_arms,
#     clippy::needless_borrow, clippy::trim_split_whitespace,
#     clippy::too_many_lines, clippy::unused_self,
#     clippy::items_after_statements, clippy::expect_fun_call,
#     clippy::len_zero, clippy::unnecessary_debug_formatting,
#     clippy::format_push_string, clippy::new_without_default,
#     clippy::map_unwrap_or (test-specific)
#
# Forbidden blanket suppressions:
#   - allow(warnings)
#   - allow(clippy::all)
#   - allow(clippy::pedantic) — use specific lint names instead
#   - allow(clippy::nursery)
#   - allow(clippy::restriction)
#
# Crates with explicit exceptions (FFI/Node unsafe):
#   - crates/eggfetch-ffi/ (unsafe_code = "allow" per project policy)
#   - crates/eggfetch-node/ (unsafe_code = "allow" per project policy)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXIT_CODE=0

echo "Checking for forbidden broad lint suppressions..."

if ! command -v rg &>/dev/null && ! command -v grep &>/dev/null; then
    echo "ERROR: either ripgrep (rg) or grep is required for lint-suppression checks." >&2
    exit 2
fi

search_suppressions() {
    local pattern="$1"
    if command -v rg &>/dev/null; then
        rg -n "$pattern" "$REPO_ROOT/crates/" -g "*.rs"
    else
        grep -RInE --include="*.rs" "$pattern" "$REPO_ROOT/crates/"
    fi
}

# Check for allow(warnings) — forbidden everywhere
if search_suppressions '#!\[allow\(warnings\)\]|#\[allow\(warnings\)\]'; then
    echo "ERROR: Found 'allow(warnings)' — forbidden in all Rust files."
    EXIT_CODE=1
fi

# Check for allow(clippy::all) — forbidden everywhere
if search_suppressions '#!\[allow\(clippy::all\)\]|#\[allow\(clippy::all\)\]'; then
    echo "ERROR: Found 'allow(clippy::all)' — use specific lint names instead."
    EXIT_CODE=1
fi

# Check for allow(clippy::pedantic) — forbidden (use specific lints)
# Exception: crates/eggfetch-ffi and crates/eggfetch-node may use it
if search_suppressions '#!\[allow\(clippy::pedantic\)\]|#\[allow\(clippy::pedantic\)\]' \
    | grep -v 'crates/eggfetch-ffi/' | grep -v 'crates/eggfetch-node/'; then
    echo "ERROR: Found 'allow(clippy::pedantic)' outside FFI/Node crates — use specific lint names."
    EXIT_CODE=1
fi

# Check for allow(clippy::nursery) — forbidden (unstable lints)
if search_suppressions '#!\[allow\(clippy::nursery\)\]|#\[allow\(clippy::nursery\)\]'; then
    echo "ERROR: Found 'allow(clippy::nursery)' — nursery lints are unstable."
    EXIT_CODE=1
fi

# Check for allow(clippy::restriction) — forbidden (restrictive lints)
if search_suppressions '#!\[allow\(clippy::restriction\)\]|#\[allow\(clippy::restriction\)\]'; then
    echo "ERROR: Found 'allow(clippy::restriction)' — restriction lints conflict with pedantic."
    EXIT_CODE=1
fi

if [ "$EXIT_CODE" -eq 0 ]; then
    echo "OK: No forbidden broad lint suppressions found."
else
    echo ""
    echo "To fix: replace broad suppressions with the smallest applicable lint exemption."
    echo "See CONTRIBUTING.md for the lint policy."
fi

exit "$EXIT_CODE"
