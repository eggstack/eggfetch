# HTTPX 0.28.1 Post-Phase-06 Corrective Roadmap

Date: 2026-08-18
Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Reference contract: `httpx==0.28.1` / `httpcore==1.0.9`

## Purpose

The remaining-parity implementation substantially improved EggFetch, but the current repository should not yet be treated as closed against the acceptance criteria in the prior Phase 01-06 plans. This corrective program is intentionally narrow. It does not reopen already-qualified HTTPX behavior, replace Hyper, add another networking backend, or broaden the compatibility contract.

The goal is to correct the implementation/qualification inconsistencies found after Phase 06, preserve the architectural invariants of EggFetch, explicitly classify unavoidable Hyper-backed differences, and produce a new exact-SHA Stage C qualification only after the executable tree is stable.

## Findings that require correction

1. **The current qualification record is stale for HEAD.** `compat/httpx/0.28.1/profile.toml` binds qualification to `48bad19fa1bb7ab7c91bcd67787efb2e41127fff`, while `4571cb55...` subsequently changed executable Rust and Python compatibility code. The status record incorrectly says only documentation/status changes followed the qualified executable SHA.
2. **SSLContext translation is not fail-closed enough.** The arbitrary-context translator uses CA-count similarity as a heuristic for deciding whether a caller context should become `verify=True`; helper-created contexts are reconstructed from registry metadata that can become stale after the caller mutates the actual `SSLContext`; externally supplied contexts passed through the helper can be registered with semantics that are not provably equivalent.
3. **HTTP/2-only construction is exposed but HTTPX semantics are not fully matched.** Cleartext H2 prior knowledge is absent; an H2-only TLS client can silently fall back to HTTP/1.1 against an H1-only peer; specialized direct-connector paths do not provide H2. These must either be implemented or formally retained as bounded differences with truthful qualification claims.
4. **Request extension plumbing is asymmetric.** `target` and `sni_hostname` reach the native engine through the streaming method, but the ordinary native request path does not accept/forward the extension channel. The Python `trace` extension is not yet bridged into the core trace observer.
5. **Response metadata is only partially surfaced.** Core stores wire reason phrases, but buffered Python responses derive the canonical phrase rather than the retained wire phrase. Response extension assembly does not yet provide all of the available native metadata consistently.
6. **`network_stream` core support is stronger than facade exposure.** 101 upgrade ownership exists, but HTTPX response extensions do not consistently receive a `network_stream`; successful proxy CONNECT is not currently exposed as an owned upgraded stream; sync wrapper runtime ownership and `start_tls` semantics require correction or narrowing.
7. **Proxy TLS trust domains are structurally split but still coupled by fallback.** The pipeline passes origin TLS configuration as the fallback proxy TLS configuration. A custom origin trust/client identity must not silently become proxy endpoint policy. Proxy metadata also needs explicit redaction coverage.

## Architectural invariants

Every corrective phase MUST preserve these rules:

- All actual network I/O remains in `eggfetch-core`.
- The sync Python facade remains a facade over the canonical async/core engine; no parallel sync networking stack.
- Do not add OpenSSL/native-tls as a second TLS/network backend merely to emulate arbitrary Python `SSLContext` internals.
- Do not use unsafe CPython `_ssl`/OpenSSL pointer extraction, private interpreter layout coupling, or private-key exfiltration.
- Do not expose ordinary Hyper pooled sockets as writable Python I/O handles.
- Do not fabricate `stream_id`, connection metadata, reason phrases, trace events, or TLS settings that the underlying transport cannot truthfully provide.
- Do not add new CI matrices, qualification formats, or release automation. Use the repository's existing Tier 1 and manual qualification paths.
- Compatibility claims are test/evidence claims, not implementation-intent claims.

## Phase order

### Corrective 01 — TLS translation and proxy trust isolation

Plan: `plans/httpx-parity-corrective-01-tls-and-proxy-trust-safety.md`

Remove heuristic SSLContext translation, make helper/context mutation behavior explicit and safe, preserve representable state exactly, fail closed for state that cannot be proven equivalent, separate proxy endpoint TLS policy from origin TLS policy, and ensure proxy metadata is redacted.

This phase is first because TLS trust mistakes are security-significant and later network-stream `start_tls` work must use the same translation boundary.

### Corrective 02 — request extensions and wire metadata

Plan: `plans/httpx-parity-corrective-02-extension-and-wire-metadata-plumbing.md`

Make ordinary and streaming request dispatch use the same typed transport-hint path, bridge HTTPX `trace` callbacks to the Rust trace observer without storing Python objects in core, and surface truthful wire response metadata through the Python compatibility response.

### Corrective 03 — network stream and upgrade exposure

Plan: `plans/httpx-parity-corrective-03-network-stream-upgrade-exposure.md`

Finish the facade exposure of 101/CONNECT-owned streams, correct runtime/lifecycle semantics, make `start_tls` use the safe TLS boundary when feasible, and retain ordinary pooled HTTP/1.1/HTTP/2 `network_stream` as a bounded difference if Hyper cannot expose safe ownership.

### Corrective 04 — H2-only semantics and residual classification

Plan: `plans/httpx-parity-corrective-04-h2-only-semantics-and-residual-classification.md`

Determine whether H2-only ALPN enforcement and cleartext prior-knowledge can be implemented cleanly using the existing Hyper/h2 dependency stack without replacing the transport architecture. Implement what is justified; otherwise classify each remaining behavior precisely and remove misleading parity wording.

`stream_id` belongs to this residual review as well: use it only if a supported Hyper/h2 seam exposes the actual ID; otherwise keep it absent and documented.

### Corrective 05 — exact-SHA requalification and closure truthfulness

Plan: `plans/httpx-parity-corrective-05-exact-sha-requalification-and-ledger-closure.md`

Freeze the final executable SHA, reconcile compatibility ledgers and status documents with actual behavior, rerun the complete pinned qualification and downstream suite at that exact SHA, then update `profile.toml` and closure records only from recorded evidence.

## Explicit non-goals

The corrective program does NOT target:

- Trio/AnyIO support.
- Python 3.8/3.9 support in EggFetch.
- HTTPX private-module compatibility.
- HTTPX CLI emulation.
- Unsafe four-element socket-option pointer forms.
- Replacing Hyper solely to expose an HTTP/2 stream ID.
- Exposing writable raw sockets for normal pooled HTTP responses.
- A new release process or additional CI matrix.
- Rebaselining against unreleased HTTPX master. `FunctionAuth` remains a future stable-version rebaseline item.

## Program acceptance criteria

This corrective program is complete only when all of the following are true:

1. There is no executable commit after the SHA recorded as `qualification-sha`.
2. No SSLContext translation decision uses probabilistic/heuristic equivalence. Represented TLS policy is either proven from public/recorded state or rejected before dispatch.
3. Mutating an EggFetch-created `SSLContext` cannot silently produce a stale reconstruction that differs from the live context.
4. Origin TLS settings do not alter HTTPS proxy endpoint trust unless the caller explicitly configures proxy TLS that way.
5. `target` and `sni_hostname` work through normal buffered requests as well as streaming requests, sync and async.
6. HTTPX `trace` extension behavior is either implemented with pinned event vocabulary and callback error propagation or retained as an explicit active difference. It must not be claimed resolved without Python differential tests.
7. Wire reason phrase and available response metadata are propagated truthfully through the compat `Response.extensions`/properties.
8. 101/CONNECT `network_stream` behavior is exposed only when EggFetch actually owns the stream. Ordinary pooled responses remain safe.
9. Network-stream wrappers remain usable after client lifecycle transitions exactly to the degree claimed by tests; they do not depend accidentally on an ambient Tokio runtime.
10. H2-only behavior has an explicit final disposition for TLS ALPN enforcement, h2c prior knowledge, and specialized direct-connector routes. Tests must distinguish reference and candidate behavior rather than normalizing a difference away.
11. `stream_id` is never synthesized. If unavailable through supported APIs, the residual ledger says so.
12. `allowed-differences.toml`, `resolved-differences.toml`, `docs/residual-differences.md`, `AGENTS.md`, compatibility documentation, and closure/status records agree with each other.
13. Existing Tier 1 passes on the frozen executable SHA.
14. Full `EGGFETCH_COMPAT_REQUIRED=1` pinned HTTPX compatibility suite passes with no unapproved skips/xfails.
15. API oracle reports zero unexplained/stale/resolved-active differences.
16. Required downstream portfolio passes against the wheel built from the same frozen executable SHA.
17. The final closure record lists all retained residuals explicitly rather than describing Phases 01-05 as unrestricted parity.

## Handoff rule

Implement these phases in order. Do not update the compatibility profile to a new qualified SHA during intermediate phases. During implementation, the profile may be marked qualification-pending/reopened if the existing repository convention requires it. Only Corrective 05 may establish the new Stage C exact-SHA qualification.