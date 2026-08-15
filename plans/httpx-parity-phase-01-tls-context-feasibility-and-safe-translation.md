# HTTPX Parity Phase 01 — TLS Context Feasibility and Safe Translation

Status: implementation handoff plan.
Depends on: current Stage C-qualified HTTPX 0.28.1 facade.
Blocks: proxy-specific TLS-context parity.

## Objective

Close as much of HTTPX 0.28.1’s `ssl.SSLContext` and `create_ssl_context()` behavior as is safely representable by EggFetch’s rustls transport, while making any irreducible differences explicit and fail-closed.

This phase must not claim arbitrary Python `SSLContext` parity unless the implementation can preserve the security-relevant semantics of that context. Silent fallback to EggFetch defaults is forbidden.

## Reference behavior

HTTPX 0.28.1 `create_ssl_context(verify, cert, trust_env)` behaves as follows:

- `verify=True`: build a default client context, honoring `SSL_CERT_FILE` or `SSL_CERT_DIR` when `trust_env=True`, otherwise using HTTPX’s default CA source;
- `verify=False`: create a client context with hostname checking disabled and `CERT_NONE`;
- `verify=<str>`: deprecated path form, returning a default context loaded from that CA file/directory;
- otherwise: treat `verify` as the caller-supplied `ssl.SSLContext` object;
- deprecated `cert=` loads a client certificate chain into the selected context.

HTTPX then gives that actual context to httpcore. There is no semantic conversion step in the reference implementation.

## Current EggFetch behavior

`crates/eggfetch-python/src/tls.rs` accepts only:

- `verify` as bool or string path;
- `cert` as a string path or `(cert_path, key_path)` tuple.

The compatibility facade exports `create_ssl_context()` but currently raises `NotImplementedError`. Arbitrary Python `ssl.SSLContext` is therefore not a real supported transport input even if the facade’s Python signature accepts it.

The core `TlsConfig` already supports:

- explicit trust stores and custom CA material;
- verification and hostname policy;
- TLS 1.2/1.3 bounds;
- client identity from certificate/key material;
- SNI enablement;
- rustls-backed HTTP/1.1, HTTP/2, proxy, SOCKS, and HTTP/3 configuration paths.

## Hard feasibility result

Do not attempt to “convert everything” from an arbitrary CPython `ssl.SSLContext`.

Python’s public SSL API exposes useful state such as CA certificates, `verify_mode`, `check_hostname`, minimum/maximum TLS versions, and cipher descriptions. It does not provide a lossless public export of the complete OpenSSL `SSL_CTX` state, and in particular does not expose the private key material loaded by `SSLContext.load_cert_chain()`.

Therefore exact arbitrary-context parity is impossible under the current invariants without one of these architectural changes:

1. unsafe CPython/OpenSSL object introspection;
2. a second OpenSSL/native-TLS transport backend;
3. Python-owned TLS/network I/O.

All three are outside this phase and conflict with current repository constraints. The implementation must fail closed on state it cannot prove it represents.

## Required implementation tracks

### Track 1 — Make `create_ssl_context()` real

Implement the public helper in `eggfetch.compat.httpx` instead of raising.

Requirements:

- match HTTPX 0.28.1 argument validation and deprecation behavior;
- return a genuine Python `ssl.SSLContext`;
- honor `trust_env` for `SSL_CERT_FILE` and `SSL_CERT_DIR` exactly as the pinned reference does;
- preserve the 0.28.1 `verify=False` behavior;
- preserve deprecated string-path and `cert=` warning behavior for the pinned profile;
- add differential tests comparing type, verification flags, hostname policy, trust-source effects, warnings, and invalid-input behavior.

Do not import HTTPX itself at runtime to implement this helper.

### Track 2 — Add a Python-side context snapshot model

Create a small internal compatibility representation, for example `_SSLContextSnapshot`, containing only state that can be extracted through documented Python APIs and mapped exactly to rustls.

At minimum capture:

- whether certificate verification is required or disabled;
- whether hostname verification is enabled;
- DER-encoded CA certificates from `get_ca_certs(binary_form=True)` where available;
- minimum and maximum TLS versions when they map to TLS 1.2/1.3;
- context class/type for diagnostics;
- any explicit EggFetch-owned metadata recorded when `create_ssl_context()` itself created the context.

Do not pass arbitrary Python objects into `eggfetch-core`.

### Track 3 — Define representability rules before translation

Add a single decision function that classifies a supplied context as:

- `exactly_representable`;
- `representable_with_known_0.28.1-equivalent defaults`;
- `unrepresentable`.

The classification must be conservative.

Examples of state that must trigger rejection unless a proven exact mapping exists:

- protocol versions below TLS 1.2;
- custom verification callbacks or policies not expressible by rustls;
- custom ciphers/cipher ordering that cannot be faithfully mapped;
- CRL/verify-flag behavior that changes certificate validation and cannot be represented;
- client identity that may have been loaded into an arbitrary context but for which certificate/key material cannot be safely obtained;
- ALPN customization that conflicts with the protocol policy owned by EggFetch;
- third-party `SSLContext` implementations whose trust state cannot be enumerated safely.

A rejection should be a deterministic compatibility exception with a message stating which context property is unsupported. Never downgrade to default trust silently.

### Track 4 — Preserve context metadata created by EggFetch’s helper

Because `create_ssl_context()` sees the original `cert=` paths and construction inputs, maintain private compatibility metadata for contexts created by this helper.

Preferred design:

- use a Python-side weak registry keyed by the actual context object where feasible;
- record only reconstruction metadata, never private key bytes in Python-owned logs/reprs;
- if `cert=` was supplied to EggFetch’s helper, retain the certificate/key paths needed to reconstruct the equivalent core `ClientIdentity`;
- do not rely on object `id()` alone without lifecycle-safe weak-reference handling;
- ensure registry access is thread-safe enough for shared client construction.

This does not make arbitrary caller-created contexts fully representable; it closes the common HTTPX path where EggFetch’s own helper produced the context.

### Track 5 — Extend the PyO3 TLS boundary

Refactor `crates/eggfetch-python/src/tls.rs` so the compatibility facade can pass a typed snapshot/material bundle rather than only bool/string values.

The binding may add private constructors or conversion helpers, but the core should receive only native types such as:

- DER CA certificates;
- verification booleans;
- TLS version bounds;
- client cert/key paths or parsed material when explicitly known.

Do not make `eggfetch-core` depend on PyO3 or Python SSL types.

### Track 6 — Map safe state into `TlsConfig`

Extend `TlsConfigBuilder` only where necessary to accept the native representation required by the compatibility snapshot.

Likely additions:

- custom CA roots from DER bytes without PEM re-encoding;
- explicit hostname-verification policy if the existing verifier implementation does not already separate it correctly;
- explicit TLS version bounds from the snapshot;
- known client identity metadata created through the compatibility helper.

Keep secure defaults unchanged for all native Rust callers.

### Track 7 — Correct `create_ssl_context` and SSLContext compatibility accounting

Update:

- `compat/httpx/0.28.1/allowed-differences.toml`;
- `resolved-differences.toml`;
- `parity-cases.toml` or upstream-derived case registry as appropriate;
- `docs/reference/compatibility.md`;
- the compatibility diagnostics surface.

The current ledger must not simultaneously describe `create_ssl_context()` as missing, functionally equivalent, and a raising stub.

## Required differential tests

Create focused pinned-reference tests covering both sync and async client construction where transport dispatch is relevant.

### `create_ssl_context()` cases

- default `verify=True`, `trust_env=True` and false;
- `SSL_CERT_FILE` override;
- `SSL_CERT_DIR` where the environment can support it;
- `verify=False`;
- deprecated `verify=<file>` and directory forms;
- deprecated `cert=<file>` and tuple forms;
- invalid verify/cert types;
- warning categories/messages at the behavioral level.

### Caller-created SSLContext cases

- `ssl.create_default_context()`;
- custom CA loaded with `cafile`;
- custom CA loaded with `cadata`;
- `CERT_NONE` + hostname disabled;
- TLS 1.2-only;
- TLS 1.3-only;
- min/max range;
- a context with custom ciphers;
- a context with client cert chain loaded manually;
- a third-party context if a small deterministic fixture is available.

The last three are expected to exercise the fail-closed boundary unless exact support is implemented.

### Network proof

Use local TLS fixtures rather than inspecting only Python properties:

- custom CA accepted/rejected identically to HTTPX;
- hostname mismatch behavior;
- verification-disabled behavior;
- TLS-version negotiation success/failure;
- mTLS success for contexts created through EggFetch’s helper when reconstruction metadata is available;
- explicit rejection before dispatch for unrepresentable contexts.

## Security requirements

- no `unsafe` code;
- no extraction of OpenSSL internal pointers;
- no logging or repr of client private-key material;
- no silent trust-store broadening;
- no silent loss of hostname verification;
- no silent loss of client identity;
- CA extraction must have a bounded certificate count/total byte size before crossing the FFI boundary;
- malformed DER must fail at construction, not during an unrelated later request;
- `verify=False` remains an explicit opt-in weakening and must not leak into unrelated clients.

## Non-goals

- replacing rustls with OpenSSL;
- implementing arbitrary OpenSSL callbacks;
- matching every OpenSSL cipher configuration;
- changing the native Rust TLS API to mirror Python;
- adding Python networking;
- proxy `ssl_context` application; that is Phase 05 after this translation boundary exists.

## Acceptance criteria

This phase is complete when:

1. `eggfetch.compat.httpx.create_ssl_context()` returns a real `ssl.SSLContext` and matches HTTPX 0.28.1 for the tested construction/warning cases.
2. Passing an exactly representable caller context results in equivalent network-visible trust, hostname, and TLS-version behavior.
3. Custom CA contexts work through the native Rust engine without adding a second TLS backend.
4. A context with state that cannot be represented exactly is rejected deterministically before network dispatch; it is never silently approximated.
5. The implementation contains no CPython/OpenSSL internal pointer access and no new `unsafe` allowance.
6. Existing bool/string `verify` and `cert` paths remain backward compatible for the native EggFetch Python API.
7. The compatibility ledger truthfully distinguishes resolved behavior from remaining arbitrary-context limitations.
8. Focused TLS differential tests pass against pinned HTTPX 0.28.1 for sync and async paths.
9. `./scripts/check.sh` passes.

## Decision gate for unrestricted SSLContext parity

After the safe subset is implemented, write a short evidence note listing the remaining unrepresentable `SSLContext` behaviors. If maintainers still require exact arbitrary-context parity, stop before adding a second TLS backend and request an explicit architectural decision. Do not solve that residual gap by weakening the safety or crate-boundary rules.
