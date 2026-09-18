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
- The one live ledger is `plans/httpx-parity-correction-status.md`: it records the exact executable SHA that the HTTPX 0.28.1 and httpx2 2.12.0 Stage C qualifications are bound to. Earlier bindings are historical after subsequent qualification-sensitive changes. Any change to executable code (Rust sources, tests, build/validation scripts, packaging config) invalidates the current binding and requires a fresh exact-SHA requalification from a new freeze, following the current closure plan and status procedure. Never hardcode a SHA in docs or skills — always reference the ledger.
- The only places that state the current binding SHA are the canonical qualification records (`docs/residual-differences.md`, `docs/reference/compatibility.md`, `docs/reference/compatibility-stage-decision.md`). Every other doc and skill must reference the live ledger instead of naming a SHA. When the ledger advances to a new freeze, update those three records together (demoting the previous SHA to historical) and leave historical plan entries untouched.
- Docs-only commits do not invalidate the SHA binding.
- When finishing new work that changes a compatibility claim, update the status ledger and both `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml` together; never hand-edit generated manifests.
- The canonical embedded/core feature recipes are the profile matrix in
  `docs/architecture/feature-flags.md`; README and `docs/rust/guide.md` should
  link to it rather than inventing ambiguous “minimal” or “lite” labels.

## Key Constraints

- Keep documentation accurate against the current codebase state.
- Reference architecture docs from AGENTS.md using relative paths.
- Ensure all internal links resolve.
- All examples should be runnable or clearly marked as illustrative.
- HTTP/3 qualification claims must point to the versioned corpus and evidence
  ledger under `qualification/http3/` and `plans/`; unsupported or unexecuted
  external cases must remain explicit.
- HTTP/3 remains experimental with named graduation blockers in
  `docs/architecture/core-tls-proxy-protocols.md`
  (§ "Production Graduation Decision"). The deterministic H3 suites
  re-passed on the frozen executable recorded in the live ledger
  `plans/httpx-parity-correction-status.md`; earlier freeze SHAs in plan
  history are not the current binding. Do not describe documented controls
  as independent-server or production evidence.
- Embedded footprint numbers live only in
  `docs/architecture/embedded-footprint.md` (manual qualification in
  `qualification/embedded/`); link there instead of copying byte counts
  into README/guides. The full compatibility profile is not a footprint
  win; the lean `standard-http1` profile is a measured linked-byte
  improvement on its target/toolchain — never claim slimming beyond
  `embedded-footprint.md`.
- The external `qualification/native-tower-service/` fixture is manual
  qualification, pinned to Tonic 0.14.6 with `codegen` only. Keep its
  Tonic `transport`/`Channel`-free scope and distinguish it from the
  historical Tonic 0.12.3 evidence in the adapter plan.
- Security-sensitive information belongs in `docs/security/` or `docs/architecture/`.
- Native request-failure documentation must distinguish stable `Error::kind()`
  values from opt-in `RequestFailure` subtypes. Standard HTTP/HTTPS DNS and
  refusal detail are evidence-backed; keep proxy, UDS, H3, and custom-dialer
  route gaps explicit, and do not recommend matching error display strings.
  `max_decoded_body_size` documentation must include unencoded/identity
  buffered and streaming responses.
