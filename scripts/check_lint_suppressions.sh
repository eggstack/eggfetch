#!/usr/bin/env bash
# check_lint_suppressions.sh — Fail CI when forbidden broad lint suppressions
# are introduced in production, test, or fuzz Rust source files.
#
# Usage:
#   bash scripts/check_lint_suppressions.sh             # scan the repository
#   bash scripts/check_lint_suppressions.sh --self-test # assert the verdicts
#
# Forbidden blanket suppressions (the lint list may contain other entries and
# a `reason = "..."` field; the attribute may be `allow`, `deny` or `expect`):
#   - warnings
#   - clippy::all
#   - clippy::pedantic — use specific lint names instead
#   - clippy::nursery
#   - clippy::restriction
#
# The match is anchored on the lint list, not on the whole attribute body, so a
# blanket lint combined with other lints (`#[allow(clippy::all, clippy::pedantic)]`)
# is rejected too. `#[expect(...)]` has the same effect as `#[allow(...)]` and
# is rejected identically.
#
# Scan roots: `crates/` and `fuzz/` (every `*.rs` file).
#
# Known limitation: the scanner reads source text, so a blanket attribute
# quoted inside a string literal is ignored, but one written inside a comment
# is still reported. Do not paste a blanket attribute into a comment.
#
# Crates with explicit exceptions (FFI/Node pedantic exemption):
#   - crates/eggfetch-ffi/  (pedantic clippy is overridden there by policy)
#   - crates/eggfetch-node/ (pedantic clippy is overridden there by policy)
# The exemption covers `allow` and `expect` of `clippy::pedantic` only; the
# blanket `deny` forms stay rejected everywhere, as do `warnings`,
# `clippy::all`, `clippy::nursery` and `clippy::restriction`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Directories scanned relative to the scanned root.
SCAN_DIRS=("crates" "fuzz")

# Directories (relative to the scanned root) exempt from the pedantic rule.
PEDANTIC_EXEMPT_DIRS=("crates/eggfetch-ffi" "crates/eggfetch-node")

# The scanner. It parses every Rust file as one string, so attributes whose
# lint list spans several lines are seen exactly like single-line ones.
# Findings are printed as: <relative path>:<line>:<column>: <kind> <lint>
AWK_SCANNER='
function trim(s) {
    sub(/^[ \t\r\n]+/, "", s)
    sub(/[ \t\r\n]+$/, "", s)
    return s
}

# Extract the balanced group that starts at `start` (1-based, pointing at the
# opening delimiter). Sets CONTENT and ENDPOS. Returns 1 on success.
function balanced(text, start, opench, closech,    depth, out, ch, i, n, instr, esc) {
    depth = 0
    out = ""
    instr = 0
    esc = 0
    n = length(text)
    for (i = start; i <= n; i++) {
        ch = substr(text, i, 1)
        if (instr) {
            if (esc) { esc = 0; out = out ch; continue }
            if (ch == "\\") { esc = 1; out = out ch; continue }
            if (ch == "\"") instr = 0
            out = out ch
            continue
        }
        if (ch == "\"") { instr = 1; out = out ch; continue }
        if (ch == opench) {
            depth++
            if (depth == 1) continue
            out = out ch
            continue
        }
        if (ch == closech) {
            depth--
            if (depth == 0) { CONTENT = out; ENDPOS = i; return 1 }
            out = out ch
            continue
        }
        out = out ch
    }
    return 0
}

# Split a list on its top-level commas. Fills `arr` and returns the count.
function split_top(s, arr,    i, n, ch, depth, instr, esc, cur, c) {
    depth = 0
    instr = 0
    esc = 0
    cur = ""
    c = 0
    n = length(s)
    for (i = 1; i <= n; i++) {
        ch = substr(s, i, 1)
        if (instr) {
            cur = cur ch
            if (esc) { esc = 0; continue }
            if (ch == "\\") { esc = 1; continue }
            if (ch == "\"") instr = 0
            continue
        }
        if (ch == "\"") { instr = 1; cur = cur ch; continue }
        if (ch == "(" || ch == "[" || ch == "{") { depth++; cur = cur ch; continue }
        if (ch == ")" || ch == "]" || ch == "}") { depth--; cur = cur ch; continue }
        if (ch == "," && depth == 0) { arr[++c] = cur; cur = ""; continue }
        cur = cur ch
    }
    arr[++c] = cur
    return c
}

function relative_path(path,    rl) {
    rl = length(ROOT)
    if (substr(path, 1, rl) == ROOT) return substr(path, rl + 2)
    return path
}

function line_of(text, pos,    prefix, count) {
    prefix = substr(text, 1, pos - 1)
    count = gsub(/\n/, "", prefix)
    return count + 1
}

function column_of(text, pos,    prefix, j, last) {
    prefix = substr(text, 1, pos - 1)
    last = 0
    for (j = length(prefix); j > 0; j--) {
        if (substr(prefix, j, 1) == "\n") { last = j; break }
    }
    return pos - last
}

function is_pedantic_exempt(rel, kind, lint,    i, dir) {
    if (lint != "clippy::pedantic") return 0
    if (kind != "allow" && kind != "expect") return 0
    for (i = 1; i <= length(EXEMPT_LIST); i++) {
        dir = EXEMPT_LIST[i]
        if (substr(rel, 1, length(dir) + 1) == dir "/") return 1
    }
    return 0
}

function report(kind, lint) {
    if (is_pedantic_exempt(REL, kind, lint)) return
    print REL ":" CUR_LINE ":" CUR_COL ": " kind " " lint
}

# Inspect one attribute body (the text between `[` and `]`).
function scan_attribute(content,    name, rest, spec, item, i, n) {
    while (1) {
        content = trim(content)
        if (match(content, /^[A-Za-z_][A-Za-z0-9_]*/) == 0) return
        name = substr(content, 1, RLENGTH)
        rest = trim(substr(content, RLENGTH + 1))
        if (name == "cfg_attr") {
            # cfg_attr(condition, attribute): peel the condition and rescan.
            if (substr(rest, 1, 1) != "(") return
            if (balanced(rest, 1, "(", ")") == 0) return
            n = split_top(CONTENT, ITEMS)
            if (n < 2) return
            spec = trim(ITEMS[2])
            content = spec
            continue
        }
        if (name == "allow" || name == "deny" || name == "expect") {
            if (substr(rest, 1, 1) != "(") return
            if (balanced(rest, 1, "(", ")") == 0) return
            n = split_top(CONTENT, ITEMS)
            for (i = 1; i <= n; i++) {
                item = trim(ITEMS[i])
                if (item in BLANKET) {
                    report(name, item)
                    return
                }
            }
            return
        }
        return
    }
}

BEGIN {
    RS = "\034"
    BLANKET["warnings"] = 1
    BLANKET["clippy::all"] = 1
    BLANKET["clippy::pedantic"] = 1
    BLANKET["clippy::nursery"] = 1
    BLANKET["clippy::restriction"] = 1
    split(EXEMPT_DIRS, EXEMPT_LIST, ",")
}

{
    text = $0
    n = length(text)
    i = 1
    instr = 0
    esc = 0
    while (i <= n) {
        ch = substr(text, i, 1)
        if (instr) {
            if (esc) { esc = 0; i++; continue }
            if (ch == "\\") { esc = 1; i++; continue }
            if (ch == "\"" || ch == "\n") instr = 0
            i++
            continue
        }
        if (ch == "\"") { instr = 1; i++; continue }
        if (ch == "#") {
            j = i + 1
            if (substr(text, j, 1) == "!") j++
            if (substr(text, j, 1) == "[") {
                if (balanced(text, j, "[", "]") == 1) {
                    REL = relative_path(FILENAME)
                    CUR_LINE = line_of(text, i)
                    CUR_COL = column_of(text, i)
                    # Save the attribute extent before the nested scan runs:
                    # the nested scan reuses CONTENT and ENDPOS.
                    attr_content = CONTENT
                    attr_end = ENDPOS
                    scan_attribute(attr_content)
                    i = attr_end + 1
                    continue
                }
            }
        }
        i++
    }
}
'

# Collect the Rust files to scan, relative to the given root.
RUST_FILES=()

collect_rust_files() {
    local root="$1"
    local rel dir file
    RUST_FILES=()
    for rel in "${SCAN_DIRS[@]}"; do
        dir="$root/$rel"
        [[ -d "$dir" ]] || continue
        while IFS= read -r file; do
            [[ -n "$file" ]] && RUST_FILES+=("$file")
        done < <(find "$dir" -type f -name '*.rs' | LC_ALL=C sort)
    done
}

# Scan a root and print one line per forbidden suppression.
scan_root() {
    local root="$1"
    collect_rust_files "$root"
    if [[ ${#RUST_FILES[@]} -eq 0 ]]; then
        return 0
    fi
    awk -v ROOT="$root" -v EXEMPT_DIRS="$(IFS=,; echo "${PEDANTIC_EXEMPT_DIRS[*]}")" \
        "$AWK_SCANNER" "${RUST_FILES[@]}"
}

explain_finding() {
    local line="$1"
    local kind lint
    kind="$(printf '%s' "$line" | awk '{print $2}')"
    lint="$(printf '%s' "$line" | awk '{print $3}')"
    if [[ "$lint" == "clippy::pedantic" ]]; then
        echo "  ${line} — blanket pedantic suppression; name the specific lints instead."
    elif [[ "$lint" == "clippy::all" || "$lint" == "clippy::nursery" || "$lint" == "clippy::restriction" ]]; then
        echo "  ${line} — blanket ${lint} group; name the specific lints instead."
    elif [[ "$lint" == "warnings" ]]; then
        echo "  ${line} — blanket warnings suppression; name the specific lints instead."
    elif [[ "$kind" == "deny" ]]; then
        echo "  ${line} — blanket deny form; use workspace lints plus specific per-item attributes."
    else
        echo "  ${line} — blanket ${lint} suppression; name the specific lints instead."
    fi
}

scan_repository() {
    echo "Checking for forbidden broad lint suppressions in crates/ and fuzz/..."
    local findings
    if ! findings="$(scan_root "$REPO_ROOT")"; then
        echo "ERROR: the lint-suppression scan could not complete." >&2
        return 1
    fi
    if [[ -z "$findings" ]]; then
        echo "OK: No forbidden broad lint suppressions found."
        return 0
    fi
    echo "ERROR: Found forbidden broad lint suppressions:"
    local line
    while IFS= read -r line; do
        [[ -n "$line" ]] || continue
        explain_finding "$line"
    done <<< "$findings"
    echo ""
    echo "To fix: replace broad suppressions with the smallest applicable lint exemption."
    echo "See CONTRIBUTING.md for the lint policy."
    return 1
}

# Assert the guard's verdict for every supported attribute form. The fixtures
# live in a temporary directory; the working tree is never modified.
run_self_test() (
    set -uo pipefail

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    # path|attribute|verdict  (verdict: reject | permit)
    cases=(
        'crates/eggfetch-core/src/allow_pedantic.rs|#[allow(clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/allow_pedantic_reason.rs|#[allow(clippy::pedantic, reason = "x")]|reject'
        'crates/eggfetch-core/src/allow_warnings_reason.rs|#[allow(warnings, reason = "x")]|reject'
        'crates/eggfetch-core/src/allow_all_and_pedantic.rs|#[allow(clippy::all, clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/expect_pedantic.rs|#[expect(clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/allow_pedantic_reason_nursery.rs|#[allow(clippy::pedantic, reason = "x", clippy::nursery)]|reject'
        'crates/eggfetch-core/src/allow_nursery.rs|#[allow(clippy::nursery)]|reject'
        'crates/eggfetch-core/src/allow_restriction.rs|#[allow(clippy::restriction)]|reject'
        'crates/eggfetch-core/src/expect_warnings.rs|#[expect(warnings)]|reject'
        'crates/eggfetch-core/src/expect_all.rs|#[expect(clippy::all)]|reject'
        'crates/eggfetch-core/src/expect_nursery.rs|#[expect(clippy::nursery)]|reject'
        'crates/eggfetch-core/src/expect_restriction.rs|#[expect(clippy::restriction)]|reject'
        'crates/eggfetch-core/src/deny_pedantic.rs|#[deny(clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/deny_warnings.rs|#[deny(warnings)]|reject'
        'crates/eggfetch-core/src/deny_all.rs|#[deny(clippy::all)]|reject'
        'crates/eggfetch-core/src/deny_nursery.rs|#[deny(clippy::nursery)]|reject'
        'crates/eggfetch-core/src/deny_restriction.rs|#[deny(clippy::restriction)]|reject'
        'crates/eggfetch-core/src/inner_allow_pedantic.rs|#![allow(clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/inner_expect_nursery.rs|#![expect(clippy::nursery)]|reject'
        'crates/eggfetch-core/src/allow_pedantic_multi.rs|#[allow(clippy::pedantic, reason = "x", clippy::nursery)]|reject'
        'crates/eggfetch-core/src/cfg_attr_pedantic.rs|#[cfg_attr(feature = "x", allow(clippy::pedantic))]|reject'
        'crates/eggfetch-core/src/multiline_pedantic.rs|#[allow(
    clippy::pedantic
)]|reject'
        'fuzz/fuzz_targets/allow_pedantic.rs|#[allow(clippy::pedantic)]|reject'
        'crates/eggfetch-core/src/allow_specific.rs|#[allow(clippy::cast_precision_loss)]|permit'
        'crates/eggfetch-core/src/allow_unused_mut.rs|#[allow(unused_mut)]|permit'
        'crates/eggfetch-core/src/allow_dead_code_reason.rs|#[allow(dead_code, reason = "test helper")]|permit'
        'crates/eggfetch-core/src/allow_allow_attributes.rs|#[allow(clippy::allow_attributes)]|permit'
        'crates/eggfetch-core/src/expect_specific.rs|#[expect(clippy::cast_precision_loss)]|permit'
        'crates/eggfetch-core/src/expect_unused_mut.rs|#[expect(unused_mut)]|permit'
        'crates/eggfetch-core/src/deny_specific.rs|#[deny(clippy::cast_precision_loss)]|permit'
        'crates/eggfetch-ffi/src/allow_pedantic.rs|#[allow(clippy::pedantic)]|permit'
        'crates/eggfetch-node/src/allow_pedantic.rs|#[allow(clippy::pedantic)]|permit'
        'crates/eggfetch-ffi/src/expect_pedantic.rs|#[expect(clippy::pedantic)]|permit'
    )

    for entry in "${cases[@]}"; do
        path="${entry%%|*}"
        rest="${entry#*|}"
        attribute="${rest%%|*}"
        verdict="${rest##*|}"
        mkdir -p "$tmp/$(dirname "$path")"
        printf '%s\nfn fixture() {}\n' "$attribute" >"$tmp/$path"
    done

    findings="$(scan_root "$tmp")"

    failures=0
    checked=0
    for entry in "${cases[@]}"; do
        path="${entry%%|*}"
        rest="${entry#*|}"
        attribute="${rest%%|*}"
        verdict="${rest##*|}"
        checked=$((checked + 1))
        if printf '%s\n' "$findings" | grep -q "^${path}:"; then
            found="rejected"
        else
            found="permitted"
        fi
        if [[ "$verdict" == "reject" && "$found" != "rejected" ]]; then
            echo "FAIL: expected a rejection for ${path} (${attribute}) but it was permitted"
            failures=$((failures + 1))
        elif [[ "$verdict" == "permit" && "$found" != "permitted" ]]; then
            echo "FAIL: expected ${path} (${attribute}) to be permitted but it was rejected"
            failures=$((failures + 1))
        else
            printf '  ok  %-9s %s\n' "$verdict" "${path}"
        fi
    done

    if [[ "$failures" -ne 0 ]]; then
        echo ""
        echo "Lint-suppression guard self-test FAILED: ${failures} of ${checked} case(s) wrong."
        exit 1
    fi

    echo ""
    echo "OK: lint-suppression guard self-test passed (${checked} cases)."
)

main() {
    case "${1:-}" in
        --self-test)
            run_self_test
            ;;
        "")
            scan_repository
            ;;
        *)
            echo "usage: $0 [--self-test]" >&2
            exit 2
            ;;
    esac
}

main "$@"
