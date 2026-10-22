# Python SSLContext Private Contract Hardening

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Relevant architecture: docs/architecture/python-bindings.md, docs/architecture/core-tls-proxy-protocols.md
Normative compatibility record: docs/residual-differences.md

## Objective

Turn the current Python-to-Rust SSLContext translation boundary into one explicit, versioned, private contract without changing any supported Python API or TLS behavior.

The current implementation is deliberately conservative and fail-closed. python/eggfetch/_ssl_context.py snapshots public ssl.SSLContext state, classifies representability, tracks helper-created context provenance in a weak registry, and detects live mutation by fingerprint. crates/eggfetch-python/src/tls.rs imports several private Python objects and fields independently and applies them to eggfetch_core::TlsConfigBuilder.

This plan must preserve the existing security posture while reducing cross-file coupling.

## Non-negotiable behavior baseline

Preserve exactly:

- verify=True/default certificate and hostname verification;
- verify=False behavior and warning/repr semantics already documented;
- CERT_REQUIRED with check_hostname=False as certificate verification enabled but hostname verification disabled;
- CA path and DER-list behavior;
- SSL_CERT_FILE / SSL_CERT_DIR handling under trust_env;
- helper-created custom CA contexts;
- helper-created mTLS cert/key provenance;
- explicit TLS 1.2 / 1.3 minimum and maximum bounds;
- conservative rejection of unrepresentable TLS versions;
- conservative rejection of unsupported cipher/options/ALPN/client-cert state;
- live mutation invalidating stale helper registry metadata;
- external/subclass context fail-closed behavior already recorded in residual differences;
- bounded CA extraction count/bytes;
- no private key extraction and no secret-bearing diagnostics.

Do not expand arbitrary OpenSSL SSLContext compatibility.

## Part A — define one private export contract

Add one underscore-prefixed Python helper in _ssl_context.py that exports all Rust-consumed state in one bounded object.

Preferred contract characteristics:

- an integer schema_version starting at 1;
- a mapping/dict rather than an unversioned positional tuple;
- explicit classification field;
- explicit representable payload fields;
- explicit helper-registry/provenance payload when applicable;
- no live SSLContext object embedded in the payload;
- no private key bytes, passwords, PEM content, or unbounded diagnostic strings.

Candidate shape:

- schema_version;
- classification;
- verify_mode;
- check_hostname;
- ca_certs_der;
- min_version;
- max_version;
- helper_metadata containing only currently required provenance such as verify/cert_path/key_path after fingerprint validation.

The final exact keys may differ if existing types make another shape safer, but there must be exactly one Rust-facing private export function.

Keep snapshot_context(), _classify_context(), and the registry private and reusable internally; Rust should no longer need to know how many private helper objects participate.

## Part B — make Rust consume only that contract

Refactor crates/eggfetch-python/src/tls.rs so SSLContext translation:

1. verifies the object is an ssl.SSLContext under the existing rules;
2. calls the single private export helper;
3. requires schema_version == 1;
4. validates required keys and Python types explicitly;
5. applies the normalized payload to TlsConfigBuilder;
6. rejects unknown/new schema versions before network I/O.

Separate payload decoding from TlsConfigBuilder application so schema/type errors and TLS semantic errors remain testable independently.

Do not use permissive dictionary access that silently treats a missing required key as a default.

## Part C — retain classification and registry safety

The Python export helper must:

- snapshot live public state at call time;
- run the existing conservative representability classifier;
- consult helper registry metadata only after the existing mutation fingerprint check succeeds;
- discard/ignore stale metadata after live mutation exactly as today;
- return no helper cert provenance for a context whose registry fingerprint no longer matches;
- preserve existing distinction between helper-created representable contexts and external unprovable contexts.

Do not move security decisions into Rust merely to make the Python helper smaller. Python owns what public SSLContext state can be observed and what helper provenance exists; Rust owns translation of the normalized safe contract into rustls configuration.

## Part D — version and malformed-contract tests

Add direct tests for the private contract:

- schema version is present and stable;
- expected keys/types for default context;
- verify=False export;
- check_hostname=False export;
- custom CA DER export;
- explicit TLS 1.2/1.3 bounds;
- helper-created mTLS metadata;
- mutated helper context no longer trusts stale registry metadata;
- oversized CA extraction still fails at the existing bound;
- unsupported TLS version remains rejected;
- unsupported/unrepresentable context remains rejected.

Add Rust-side malformed-contract tests by monkeypatching/replacing the private exporter in a controlled test process or by extracting a pure payload-decoder helper:

- missing schema_version;
- unknown schema version;
- missing required key;
- wrong key type;
- malformed CA entry;
- invalid classification;
- invalid TLS version numeric value.

All must fail before request dispatch.

## Part E — cross-version evidence

Run the private contract tests on every Python version readily available in the repository's supported matrix.

Python 3.15 remains subject to the separate wheel-production qualification plan. This plan may add tests compatible with 3.15 but must not claim that the pending 18-wheel rehearsal has completed.

Where CPython exposes version-dependent SSL defaults, tests should assert semantic classification/translation rather than hard-code unrelated cipher ordering.

## Part F — proxy TLS and network proof

Because the same translation machinery can feed proxy TLS, run focused tests covering both destination TLS and Proxy(ssl_context=...) compatibility paths.

Retain/extend network proof for:

- trusted custom CA success;
- wrong/untrusted CA failure;
- verify=False behavior;
- hostname verification enabled/disabled cases;
- mTLS helper context where current fixtures support it;
- TLS version bound enforcement.

Do not add public test-only knobs.

## Part G — documentation

Update only internal architecture/residual-difference text necessary to describe the new private bridge.

Do not document the schema as a supported Python API. It belongs to implementation architecture and may evolve only with coordinated Python/Rust binding changes.

## Focused validation

Required:

- maturin develop -m crates/eggfetch-python/Cargo.toml;
- Python SSLContext translation/classification tests;
- corrective TLS/proxy trust-safety tests;
- destination/proxy TLS network proof tests;
- python scripts/check_native_python_api.py;
- python scripts/check_python_typing_surface.py;
- ./scripts/check.sh.

If any compatibility-facade SSL helper code changes, run the complete pinned facade SSL-related test set before closure.

## Acceptance criteria

- [x] Rust imports exactly one private Python SSLContext export helper for normalized context state. — the only import is `crates/eggfetch-python/src/tls.rs:59-61`, which resolves `eggfetch._ssl_context._export_ssl_context_state`; the helper itself is `crates/eggfetch-python/python/eggfetch/_ssl_context.py:467`.
- [x] The private payload carries schema_version and rejects unknown versions. — the payload carries `schema_version` (`crates/eggfetch-python/python/eggfetch/_ssl_context.py:236,490`) and Rust rejects any version other than `1` with a `ValueError` before dispatch (`crates/eggfetch-python/src/tls.rs:66-73`); covered by `test_malformed_unknown_schema_version_fails_before_dispatch` and `test_export_has_bounded_versioned_shape` in `crates/eggfetch-python/tests/compat/test_ssl_context_translation.py`.
- [x] Missing/wrong-typed required fields fail closed before I/O. — `required_export_item` (`crates/eggfetch-python/src/tls.rs:107-114`) rejects a missing field and `export_type_error` (`:100-105`) rejects a wrong type; `test_malformed_export_fails_before_dispatch`, `test_malformed_payload_not_mapping_fails_before_dispatch` and `test_malformed_missing_header_field_fails_before_dispatch` cover it.
- [x] Existing representability classification remains conservative. — `_classify_context` (`crates/eggfetch-python/python/eggfetch/_ssl_context.py:239`) still yields `unrepresentable` for contexts it cannot prove, and Rust rejects anything outside `exactly_representable` / `representable_with_known_defaults` (`crates/eggfetch-python/src/tls.rs:80-90`).
- [x] Helper mutation invalidation and mTLS provenance are unchanged. — `test_export_mutation_invalidation` and `test_export_helper_mtls_provenance` in `crates/eggfetch-python/tests/compat/test_ssl_context_translation.py`, plus `test_mtls_with_eggfetch_helper_context` in `crates/eggfetch-python/tests/compat/test_ssl_context_network_proof.py`.
- [x] CA extraction bounds and secret-redaction properties are unchanged. — `test_export_custom_ca_der` and `test_export_preserves_helper_provenance_without_secrets` in `crates/eggfetch-python/tests/compat/test_ssl_context_translation.py`; the export carries no secret material.
- [x] TLS 1.2/1.3 bounds, verify, hostname verification, trust_env, destination TLS, and proxy TLS behavior are unchanged. — `test_export_tls12_tls13_bounds` and `test_export_check_hostname_false` (translation suite), and `test_tls12_server_accepted_by_eggfetch` / `test_tls13_server_accepted_by_eggfetch` / `test_tls11_server_rejected_by_eggfetch` / `test_hostname_mismatch_rejected_by_eggfetch` / `test_wrong_ca_rejected_by_eggfetch` / `test_custom_ca_accepted_by_eggfetch` / `test_trust_env_false` (network-proof suite) in `crates/eggfetch-python/tests/compat/`.
- [x] No eggfetch.__all__, constructor signature, typing surface, or documented supported API changes. — `crates/eggfetch-python/python/eggfetch/__init__.py` is unchanged in the working tree, and `scripts/check_native_python_api.py` reports `Native API manifest passed (66 exports)` with `scripts/check_python_typing_surface.py` reporting `Python typing surface passed (66 runtime exports, 32 exception bases, 24 reviewed member contracts)` on 2026-10-22. Note for the reader: the current uncommitted tree has since gained one symmetric keyword-only constructor parameter (`proxy_env=None`) on both `Client` and `AsyncClient` under a later, separately planned milestone; it is not an SSLContext-surface change.
- [x] Residual differences remain truthful without adding a waiver. — `docs/residual-differences.md` §"SSLContext representability" states the rejection reasons and names both proof suites; no waiver was added.
- [x] Tier 1 is green. — the execution record below names the full qualification (helper provenance, mutation detection, CA bounds, TLS version limits, destination/proxy trust behavior, and redaction tests) as passing at freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`. The native API and typing checkers were re-run green on 2026-10-22; the full Tier 1 gate and the compat pytest suites were **not** re-run in this session (the maintainer deferred them), so this criterion is recorded as green as of the named freeze.

## Stop conditions

If consolidating the bridge requires broadening accepted SSLContext state, using OpenSSL private pointers, extracting private key material, weakening mutation detection, silently ignoring unknown schema fields/versions, or changing public verify/cert behavior, stop and retain the current implementation until a separately reviewed security/API change is approved.

## Execution record — complete (2026-09-21)

Implemented on executable freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`.
Rust now consumes only the bounded schema-versioned `_export_ssl_context_state`
contract, with explicit type/schema validation and fail-closed rejection before
dispatch. Helper provenance, mutation detection, CA bounds, TLS version limits,
destination/proxy trust behavior, and redaction tests passed in the full
qualification.

## Acceptance-criteria disposition (2026-10-22)

All ten acceptance criteria above were re-verified against the current tree and
are **met**; none is superseded, deferred, or never applicable. Evidence is
cited inline with each criterion. The execution record above is unchanged.

Two criteria are explicitly bounded to the plan's recorded freeze
`df2549f7c64ebfccde61ed36fef785d39e83b38d` — the public-surface criterion and
the Tier 1 criterion — because the current uncommitted tree carries later
milestone work. The full Tier 1 gate and the compat pytest suites were not
re-run in this session.
