# Documentation Maintenance Skill

Use this skill when updating documentation in the eggfetch workspace.

## Workflow

1. Read `docs/README.md` for the documentation structure.
2. Read the specific doc file being updated.
3. Verify accuracy against source code and architecture docs.

## Validation

```sh
# Syntax-check Python code blocks
python scripts/check_doc_examples.py

# Verify internal links
python scripts/check_doc_links.py

# Build rustdoc and run doctests
cargo doc --workspace --all-features --no-deps
cargo test --doc -p eggfetch-core --all-features
```

## Documentation Structure

```
docs/
├── getting-started/    Installation and quickstart
├── concepts/           Core concept explanations
├── rust/               Rust API guide
├── python/             Python sync/async API guide
├── cli/                CLI reference
├── cookbook/            Practical runnable examples
├── migration/          Guides from requests and HTTPX
├── reference/          Feature matrix, errors, versioning
├── architecture/       Internal architecture documentation
├── ffi/                C ABI and FFI binding guide
├── releases/           Release process and compatibility policy
└── security/           Security guidelines and troubleshooting
```

## Plans Directory (`plans/`)

- Completed plans are **historical records, not active requirements** (verification-policy principle 9). Do not treat their step lists as current CI or release gates.
- The one live ledger is `plans/httpx-parity-correction-status.md`: it records the exact executable SHA that the HTTPX Stage C qualification is bound to. Any change to executable code (Rust sources, tests, build/validation scripts, packaging config) invalidates that binding and requires a fresh exact-SHA requalification from a new freeze, following the current closure plan and status procedure.
- Docs-only commits do not invalidate the SHA binding.
- When finishing new work that changes a compatibility claim, update the status ledger and `compat/httpx/0.28.1/profile.toml` together; never hand-edit generated manifests.

## Key Constraints

- Keep documentation accurate against the current codebase state.
- Reference architecture docs from AGENTS.md using relative paths.
- Ensure all internal links resolve.
- All examples should be runnable or clearly marked as illustrative.
- Security-sensitive information belongs in `docs/security/` or `docs/architecture/`.
