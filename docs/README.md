# eggfetch documentation

User-facing documentation for the eggfetch HTTP client.

## Structure

```
docs/
├── getting-started/    Installation and quickstart
├── concepts/           Core architecture and behavior explanations
├── rust/               Rust API guide
├── python/             Python sync/async API guide
├── cli/                CLI reference
├── cookbook/            Practical runnable examples
├── migration/          Guides from requests and HTTPX
├── reference/          Feature matrix, errors, versioning
├── releases/           Release process and compatibility policy
└── security/           Security guidelines and troubleshooting
```

## For users

- **New to eggfetch?** Start with `getting-started/quickstart.md`
- **Rust users?** See `rust/guide.md`
- **Python users?** See `python/guide.md`
- **CLI users?** See `cli/guide.md`
- **Migrating?** Check `migration/from-requests.md` or `migration/from-httpx.md`
- **Releasing?** See `releases/process.md` for the release workflow and `releases/compatibility-policy.md` for versioning

## For contributors

Documentation is checked for correctness in CI:

```sh
# Syntax-check all Python code blocks in docs
python scripts/check_doc_examples.py

# Verify internal links resolve
python scripts/check_doc_links.py

# Build rustdoc and run doctests
cargo doc --workspace --all-features --no-deps
cargo test --doc -p eggfetch-core --all-features
```

All examples in `cookbook/examples.md` should be runnable against real or
mocked endpoints. See `reference/versioning.md` for the versioning strategy.
