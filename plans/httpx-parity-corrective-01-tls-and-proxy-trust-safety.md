# HTTPX Parity Corrective 01 — TLS Translation and Proxy Trust Safety

Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Depends on: `plans/httpx-parity-post-phase06-corrective-roadmap-2026-08-18.md`

## Objective

Correct the security-sensitive parts of Phase 01 and Phase 05 without introducing a second TLS stack. The implementation must stop treating approximate SSLContext similarity as semantic equivalence, stop reconstructing stale helper-created contexts after caller mutation, and make HTTPS proxy TLS policy independent from origin TLS policy.

This phase must preserve the existing rustls-backed architecture and fail closed whenever Python/OpenSSL state cannot be represented exactly enough to preserve the behavior being claimed.

## Current defects to correct

### 1. CA-count heuristic is not a safe trust-store equivalence test

`context_to_eggfetch_kwargs()` currently compares the number of loaded CA certificates with a default context and, when the counts are within a broad threshold, treats the caller context as `verify=True` instead of carrying the actual trust anchors.

That can silently expand or alter trust. Two CA stores with similar cardinality are not equivalent.

Required correction: remove any trust-store equivalence decision based on certificate count, approximate similarity, names, ordering, or other heuristics.

### 2. Registry metadata can become stale

Contexts returned from EggFetch's `create_ssl_context()` are registered with reconstruction metadata. The caller can subsequently mutate the actual `SSLContext` using public APIs, for example:

- `load_verify_locations()`
- `minimum_version` / `maximum_version`
- `verify_mode`
- `check_hostname`
- `set_ciphers()`
- `load_cert_chain()`

The registry does not observe those mutations. Reconstructing from the original helper arguments can therefore silently differ from the live object.

Required correction: a registered context may only use stored metadata when the live context can be proven unchanged for all represented semantics. Otherwise snapshot and reclassify the live object or reject it.

### 3. Passing an existing SSLContext through the helper is not reconstruction metadata

HTTPX returns an already-supplied `ssl.SSLContext` unchanged. EggFetch may register that object, but it must not reinterpret “returned by our helper” as proof that its original trust/cert state is known.

Required correction: distinguish contexts EggFetch constructed from primitive helper arguments from contexts merely passed through the helper. The latter must be treated as external caller contexts for translation purposes.

### 4. Externally loaded client certificates cannot be exported safely

A caller-created SSLContext may contain client certificate/private-key state loaded through OpenSSL. Public Python APIs do not provide safe stable extraction of the private key. EggFetch must not silently translate such a context as if mTLS were absent.

Required correction: only support client identity when reconstruction metadata contains the source cert/key paths or another already-supported explicit identity representation. Otherwise reject before dispatch if exact behavior cannot be established.

### 5. Proxy TLS config currently falls back to origin TLS config

`ProxyRequestContext` has separate fields, but the pipeline still injects origin TLS config as a fallback when no proxy TLS config is set. That means origin CA/client identity settings may affect an `https://` proxy endpoint.

Required correction: `None` proxy TLS config means the proxy connection uses the proxy/default trust policy, not the origin configuration. Origin TLS configuration applies only to the origin connection after CONNECT.

### 6. Proxy metadata redaction must be explicit

Arbitrary proxy headers can contain credentials or deployment-sensitive metadata. Core `Headers` derives `Debug` without redaction.

Required correction: any `Debug`, diagnostic, tracing, or error representation containing proxy headers must use the existing redaction rules or a proxy-specific redacted representation. Do not globally make ordinary internal header iteration lossy; correct diagnostic formatting only.

## Required design

### A. Replace heuristic translation with a deterministic representability matrix

Define the supported arbitrary-context subset explicitly. A suggested matrix:

| SSLContext property | Translation rule |
| --- | --- |
| `CERT_NONE`, hostname disabled | exact -> native verification disabled |
| `CERT_REQUIRED`, hostname enabled | exact if trust store can be represented |
| `CERT_REQUIRED`, hostname disabled | exact only if rustls verifier path can preserve cert validation while disabling hostname validation; otherwise reject |
| public CA certs from `get_ca_certs(binary_form=True)` | use the actual DER anchors, never infer default trust from count |
| TLS min/max limited to 1.2/1.3 | translate exactly |
| TLS versions below 1.2 | reject |
| custom cipher suite policy different from supported rustls policy | reject |
| unknown ALPN policy on arbitrary external context | reject if it can materially conflict with requested transport behavior and cannot be proven absent/default |
| caller-loaded client cert/private key without reconstruction metadata | reject |
| helper-created client cert with known source paths | translate using existing native identity path |
| unknown SSLContext subclass | reject unless proven semantically identical through supported public state |

If a property cannot be inspected but can affect security/handshake behavior, classify the context as unrepresentable rather than assuming default behavior.

### B. Store a construction fingerprint for genuinely EggFetch-created contexts

For contexts constructed from EggFetch helper primitive arguments, store enough public-state fingerprint data at registration time to detect mutation before reconstruction.

The fingerprint should cover all public state EggFetch relies on or must constrain, including at minimum:

- verification mode
- hostname checking state
- actual CA DER set/hash
- minimum/maximum TLS versions
- supported/selected cipher set fingerprint used for the representability decision
- whether a client identity was loaded by the helper and the reconstruction paths
- helper trust-env decision and source CA path when relevant

At translation time:

1. snapshot the live context;
2. compare its fingerprint to the registration fingerprint;
3. if unchanged, reconstruction metadata may be used;
4. if changed, do not use stale construction metadata blindly;
5. reclassify the live context from public state;
6. if the mutation introduced state that cannot be reconstructed exactly, raise `TypeError` before dispatch.

Do not hash or store private-key contents.

### C. Treat passthrough contexts as external

If `create_ssl_context(verify=<SSLContext>)` returns the input context, either do not register it as constructible metadata or register it with a marker that forbids argument-based reconstruction. Translation must follow the external-context path.

### D. Preserve helper mTLS only through explicit reconstruction provenance

Helper-created `cert=` behavior may remain supported because the helper knows the source paths. Add tests proving:

- helper-created context with client cert succeeds when passed as `verify=ctx` without separately passing `cert=`;
- mutating/replacing the client identity after helper creation does not silently reuse the stale original identity;
- external context with client identity but no extractable provenance is rejected rather than downgraded to no client auth.

### E. Make proxy TLS independent

Change pipeline construction so:

- `origin_tls_config` is used only for the destination TLS handshake;
- `proxy_tls_config` is populated only from explicit proxy TLS configuration;
- when `proxy_tls_config` is absent, `connect_to_proxy()` uses its default proxy trust roots/settings;
- origin `verify=False`, custom origin CA, origin client identity, SNI override, and TLS version policy do not mutate proxy endpoint policy;
- explicit `Proxy(ssl_context=...)` still controls the proxy endpoint through the same safe translator.

### F. Redact proxy header diagnostics

Audit:

- `Proxy::Debug`
- `ProxyConfig::Debug` if present/added
- request/pipeline debug output
- trace/log/error formatting
- test failure helper formatting

Ensure common sensitive names (`proxy-authorization`, `authorization`, cookies, API-key-like headers under existing redaction policy) are not printed in plaintext.

## Required tests

### SSLContext unit/classification tests

Add direct tests for:

1. custom CA set whose count resembles default trust but contents differ -> actual custom anchors are translated, never `verify=True` by count;
2. two different same-sized CA sets -> never considered equivalent;
3. helper-created context unchanged -> reconstructs successfully;
4. helper-created context then `load_verify_locations()` -> live mutation is detected;
5. helper-created context then min/max TLS change -> live mutation is detected and represented or rejected correctly;
6. helper-created context then cipher policy change -> rejected before dispatch;
7. `create_ssl_context(verify=external_ctx)` -> returned identity is unchanged, but translation does not assume helper provenance;
8. external mTLS context with client key state but no safe provenance -> rejected;
9. helper-created mTLS context passed as `verify=ctx` -> client certificate is actually presented;
10. `CERT_REQUIRED` + `check_hostname=False` -> either a differential success path if exactly supported or explicit fail-closed rejection; no silent hostname-verifying substitution.

### Network proof tests

Use local CA/server fixtures only. No public network dependency.

Prove:

- custom trust permits the intended server;
- a certificate trusted only by a default/system root is not accepted when the caller supplied a deliberately narrow custom CA context;
- mutated helper context changes network behavior accordingly or is rejected before socket dispatch;
- mTLS provenance rules above are observable on the server side;
- TLS 1.2/1.3 bounds survive context translation.

### Proxy trust-domain matrix

Use separate proxy CA and origin CA.

Required cases:

1. default/no explicit proxy context + custom origin CA: proxy handshake must not use origin custom CA as its sole trust store;
2. explicit proxy CA only: proxy TLS succeeds; origin TLS fails if origin CA is not trusted;
3. explicit origin CA only: proxy behavior follows default proxy trust and does not inherit origin CA;
4. explicit proxy CA + explicit origin CA: both handshakes succeed;
5. origin `verify=False`: does not disable HTTPS proxy certificate verification;
6. proxy `ssl_context` with verification disabled: affects only proxy endpoint;
7. origin mTLS identity is never presented to the proxy endpoint;
8. proxy client identity, if supported by representable proxy SSLContext, is never presented to the origin.

### Redaction tests

Construct proxy headers with sentinel secrets and assert they do not appear in:

- `Debug` output;
- error strings produced after proxy connection failure;
- trace/debug snapshots used by tests.

## Documentation/ledger updates during this phase

Do not declare final qualification.

Update only the records necessary to keep the repository truthful while implementation is in flight:

- describe the exact representable SSLContext subset;
- remove language that arbitrary contexts are “resolved” if the implementation intentionally rejects some;
- state that unrepresentable arbitrary contexts are a bounded difference from HTTPX passthrough semantics;
- document proxy TLS independence;
- preserve the security rationale for fail-closed behavior.

## Acceptance criteria

Corrective 01 is complete only when:

- [ ] No CA-count or approximate trust-store heuristic remains.
- [ ] No helper registry entry can silently override materially mutated live SSLContext state.
- [ ] Passthrough external SSLContexts are not treated as reconstructable helper-created contexts.
- [ ] Client identity is preserved only when provenance supports exact reconstruction; otherwise translation fails before dispatch.
- [ ] No unsafe/private CPython/OpenSSL extraction is introduced.
- [ ] No second TLS/network backend is introduced.
- [ ] Origin and proxy TLS configs have independent defaults and trust domains.
- [ ] Origin `verify=False` cannot disable proxy verification.
- [ ] Origin client identity cannot be presented to proxy endpoint by fallback.
- [ ] Proxy headers/secrets are redacted in diagnostic formatting.
- [ ] Focused TLS classification tests pass.
- [ ] Focused TLS network proof tests pass using only local fixtures.
- [ ] Proxy CA/origin CA matrix passes sync and async where both facades expose the behavior.
- [ ] Existing TLS/proxy compatibility tests remain green.
- [ ] Documentation describes the safe subset and residual arbitrary-context boundary accurately.

## Out of scope

Do not implement OpenSSL-backed networking, private-key extraction from arbitrary contexts, OpenSSL cipher-suite emulation, arbitrary callbacks/certificate verification hooks, or a second proxy implementation.