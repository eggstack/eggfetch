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

## Key Constraints

- Keep documentation accurate against the current codebase state.
- Reference architecture docs from AGENTS.md using relative paths.
- Ensure all internal links resolve.
- All examples should be runnable or clearly marked as illustrative.
- Security-sensitive information belongs in `docs/security/` or `docs/architecture/`.
