# Fuzz Testing Skill

Use this skill when working on fuzz targets, property tests, or fuzzing infrastructure in eggfetch.

## Workflow

1. Read `docs/architecture/testing-fuzzing.md` for the testing approach.
2. Read `fuzz/Cargo.toml` for the fuzz target definitions.
3. Read existing fuzz targets in `fuzz/fuzz_targets/` for patterns.

## Running Fuzz Targets

```sh
cd fuzz && cargo +nightly fuzz run <target>
cd fuzz && cargo +nightly fuzz build
```

Nightly Rust is required for cargo-fuzz.

## Available Targets

| Target | Subsystem |
|--------|-----------|
| `fuzz_headers` | Header parsing and validation |
| `fuzz_cookie` | Cookie parsing, matching, jar operations |
| `fuzz_redirect` | Redirect policy and replay logic |
| `fuzz_multipart` | Multipart encoder boundary and streaming |
| `fuzz_compression` | Gzip, deflate, brotli, zstd decompression |
| `fuzz_proxy` | Proxy configuration and NO_PROXY matching |
| `fuzz_proxy_response` | Proxy CONNECT response parsing |
| `fuzz_timeout` | Timeout state machine and scheduling |
| `fuzz_retry` | Retry policy, backoff, Retry-After parsing |
| `fuzz_tls` | TLS configuration and SNI handling |
| `fuzz_url` | URL parsing and normalization |

## Property Testing

Proptest property tests run on stable Rust:

```sh
cargo test -p eggfetch-core --all-features
```

### What Property Tests Cover

| Module | Properties Verified |
|--------|---------------------|
| URL | Parse -> serialize round-trip |
| Headers | Case-insensitive lookup, insertion, validation |
| Cookies | Parse -> serialize, domain/path matching |
| Redirects | Method rewrite rules, header stripping |
| Retry | Backoff calculation, Retry-After parsing |
| PEM | Parse -> serialize for CA bundles and client certs |
| Timeout | State machine transitions |
| Multipart | Boundary generation, encoder output |

## Harness Rules

- No external network access.
- Deterministic execution.
- Bounded memory and time.
- Operates on in-memory data structures and mock transports.

## Architecture Reference

- Testing & fuzzing: `docs/architecture/testing-fuzzing.md`
