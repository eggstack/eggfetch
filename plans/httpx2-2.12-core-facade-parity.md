# HTTPX2 2.12 Core Facade Parity

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Depends on: `plans/httpx2-2.12-profile-and-delta-baseline.md`
Followed by: `plans/httpx2-2.12-sse-and-websocket-parity.md`

## Objective

Implement the non-SSE/WebSocket HTTPX2 2.12.0 public and behavioral delta using the existing Rust engine and version-specific Python compatibility adapters. Preserve the qualified HTTPX 0.28.1 facade exactly; where HTTPX2 intentionally changed semantics, branch at the compatibility layer or shared helpers parameterized by profile rather than changing old behavior globally.

## 1. Establish the HTTPX2 package adapter without copying the client stack

Create `eggfetch.compat.httpx2` as a thin compatibility package that reuses common internal implementation where semantics are identical.

Preferred approach:

- factor reusable private compatibility primitives out of the existing `eggfetch.compat.httpx` package only where doing so reduces duplication without changing its public module layout;
- keep profile-specific exports/defaults/parsers in small HTTPX/HTTPX2 modules;
- do not duplicate the ~97 KB client implementation into a second divergent `_client.py`;
- add regression tests proving the 0.28.1 facade still imports and reports the same API/signatures.

Acceptance:
- [ ] shared behavior has one implementation where practical;
- [ ] HTTPX2-specific semantics have explicit profile boundaries;
- [ ] import/module/version identities match the declared target.

## 2. Add `FunctionAuth`

Match HTTPX2 2.12.0 construction, callable expectations, request transformation, sync/async client integration, repr/type relationships, and error propagation.

Reuse the existing auth-flow state machine. Do not add a Rust-only auth abstraction if the reference behavior is fundamentally a Python callable adapter.

Acceptance:
- [ ] reference differential tests cover sync and async clients;
- [ ] redirects/retries do not call the function at incorrect times or leak credentials;
- [ ] 0.28.1 exports remain unchanged.

## 3. Add `Origin` and `URL.origin`

Implement normalized, immutable/hashable origin semantics matching HTTPX2:

- scheme normalization;
- IDNA/host normalization;
- default-port handling;
- IPv4/IPv6 behavior;
- equality/hash/repr;
- `URL.origin` return type and stability.

Prefer reuse of existing URL normalization primitives; do not create a second URL parser.

Acceptance:
- [ ] reference differential corpus covers default/explicit ports, IDNA, IPv6, equality/hash and repr;
- [ ] origin comparison can be used by compatibility code without changing native core origin keys accidentally.

## 4. Add QUERY method helpers

Add the `QUERY` method to the top-level/client convenience surface exactly where HTTPX2 exposes it, including sync and async clients.

Do not special-case QUERY in the Rust transport; it is an ordinary method token unless protocol rules require otherwise.

Acceptance:
- [ ] method, signature, body/params semantics, redirects and mock transports match reference probes;
- [ ] API manifest is exact for the declared surface.

## 5. Add Headers merge operators

Implement `Headers.__or__` and `Headers.__ior__` with reference-verified ordering, case-insensitive replacement, duplicate-value, operand-type and mutation semantics.

Acceptance:
- [ ] differential tests cover Headers|Headers, mapping operands, duplicate keys, case variants and invalid operands;
- [ ] `|=` mutates exactly as reference does without violating existing redaction behavior.

## 6. Split TLS trust-default semantics by compatibility profile

HTTPX2 changed its default SSL verification to OS truststore behavior. EggFetch native Rust already has a deliberate system-roots then WebPKI fallback policy; HTTPX 0.28.1 compatibility has separately qualified SSLContext translation semantics.

Research/probe the exact HTTPX2 2.12 behavior and implement only the representable compatibility boundary:

- default verification source;
- explicit `verify=True/False`;
- explicit SSLContext;
- deprecated/path/cert combinations still present in the target;
- client certificate conflicts;
- environment interactions.

Do not weaken native TLS defaults and do not change 0.28.1 behavior to follow HTTPX2.

Acceptance:
- [ ] network-backed certificate tests distinguish system/private/custom roots;
- [ ] unsupported Python SSLContext states continue to fail closed;
- [ ] HTTPX and HTTPX2 facades can deliberately differ without sharing mutable default policy.

## 7. Implement HTTPX2 `NO_PROXY`/proxy deltas

HTTPX2 fixed IPv6 CIDR support and may differ from the deliberately pinned 0.28.1 environment parser. Introduce a versioned parser policy or common parser with explicit behavior mode.

Cover:

- bracketed/unbracketed IPv6;
- IPv6 CIDR;
- host:port;
- domain/subdomain boundaries;
- lowercase/uppercase env precedence;
- explicit proxy vs environment proxy;
- `socks5`/`socks5h` reference wire behavior.

Acceptance:
- [ ] 0.28.1's currently qualified oddities remain unchanged;
- [ ] HTTPX2-specific cases match direct upstream probes;
- [ ] no proxy bypass broadening occurs from malformed CIDR parsing.

## 8. Match decompression hardening semantics

Audit EggFetch's native decompression implementation against HTTPX2 2.12 changes:

- cap chained Content-Encoding decoders at the reference limit;
- bound transient memory during streaming compressed responses;
- close response streams when decoder failure occurs;
- multi-frame zstd split across chunks;
- Python-version-dependent zstd implementation differences only where externally visible.

Where EggFetch is stricter for security, classify the exact difference instead of weakening limits automatically.

Acceptance:
- [ ] adversarial compressed-stream tests demonstrate bounded memory/decoder count;
- [ ] decode errors close/release the underlying body/pool lease;
- [ ] differential wire/content tests cover gzip/deflate/brotli/zstd chains.

## 9. Match multipart validation changes

HTTPX2 validates multipart part header names/values before serialization. Apply equivalent validation at the earliest common safe boundary where it benefits native EggFetch too, provided it does not break documented native behavior without migration.

Cover CR/LF injection, invalid names, binary/unicode values, custom per-part headers and streaming parts.

Acceptance:
- [ ] header-injection attempts fail before bytes are emitted;
- [ ] validation errors/types match the HTTPX2 facade where required;
- [ ] native core retains a coherent safe error taxonomy.

## 10. Match WSGI/content-length and related behavior-only fixes

Implement reference-proven behavior for:

- explicit `Transfer-Encoding` preservation/handling;
- buffered request-body length exposure to WSGI applications;
- empty/default encoding callable behavior if still divergent;
- IDNA non-leading label decoding if divergent;
- cookie extraction optimization only if semantics differ (performance-only changes need no facade behavior change).

Acceptance:
- [ ] upstream-derived WSGI probes pass against the HTTPX2 facade;
- [ ] no changes regress the HTTPX 0.28.1 WSGI/ASGI corpus.

## 11. Status codes, warnings, and public metadata

Match HTTPX2's public status aliases/constants, `HTTPXDeprecationWarning`, version/User-Agent/package metadata, and type-visible return annotations where the API oracle observes them.

Do not mechanically reproduce private modules excluded from the declared compatibility contract.

Acceptance:
- [ ] API oracle has zero unexplained non-streaming deltas;
- [ ] warning classes/category behavior are reference-tested.

## 12. Security regression cluster

Create a focused differential/security cluster based on post-0.28.1 HTTPX2 fixes:

- multipart header injection;
- conflicting framing headers where applicable;
- decompression amplification/decoder-chain limits;
- proxy/SOCKS/TLS trust boundaries;
- stream closure after decoding failure.

Use upstream behavior as compatibility evidence but keep EggFetch's stricter safe boundary when reference behavior is unsafe; record intentional differences explicitly.

## Validation

During implementation run focused HTTPX2 probes plus existing HTTPX 0.28.1 regression tests and Tier 1. Extended/full qualification occurs only in the final closure plan.

## Non-goals

- SSE/WebSocket implementation (next plan);
- Pyodide/jsfetch;
- copying httpcore2 into EggFetch;
- changing native APIs solely to resemble Python;
- Stage C declaration before final freeze;
- altering 0.28.1 semantics for convenience.

## Exit criteria

- [ ] all non-SSE/WebSocket required HTTPX2 2.12 public deltas are implemented or explicitly classified;
- [ ] all behavior-only security/correctness deltas have direct differential evidence;
- [ ] HTTPX2 API oracle has no unexplained non-streaming differences;
- [ ] HTTPX 0.28.1 focused/full regressions remain green;
- [ ] Tier 1 passes;
- [ ] qualification remains pending until the program closure freeze.