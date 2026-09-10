# HTTPX2 2.12 Profile and Delta Baseline

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Reference target: `httpx2==2.12.0`
Existing preserved contract: `httpx==0.28.1`
Followed by: `plans/httpx2-2.12-core-facade-parity.md`

## Objective

Establish an independent, versioned compatibility profile and differential baseline for HTTPX2 2.12.0 before implementing parity fixes. Reuse the existing HTTPX qualification machinery while preventing HTTPX2 semantics from contaminating the qualified 0.28.1 facade.

HTTPX2 is a maintained fork from HTTPX 0.28.1, but it is now a distinct package and contract. Treat it as a sibling compatibility target, not as an in-place version bump.

## 1. Define package/surface strategy

Choose and document the candidate import surface before implementing it. Preferred structure:

- preserve `eggfetch.compat.httpx` as the HTTPX 0.28.1 contract;
- add `eggfetch.compat.httpx2` as the explicit HTTPX2 2.12.0 contract;
- do not make importing one mutate `sys.modules` for the other;
- treat HTTPX2's upstream `alias_httpx()` behavior as reference surface to emulate only where safe and explicitly scoped, not as the internal architecture of EggFetch.

Acceptance:
- [ ] both compatibility families can coexist in one environment;
- [ ] importing the HTTPX2 facade cannot silently alter 0.28.1 facade behavior;
- [ ] package/module/version strings are deliberately specified.

## 2. Add independent reference profile

Create `compat/httpx2/2.12.0/` using the existing profile vocabulary where applicable:

- `profile.toml`;
- pinned `requirements.txt`;
- generated `reference-api.json`;
- `allowed-differences.toml`;
- `resolved-differences.toml`;
- `parity-cases.toml`;
- upstream-test inventory / derived-case records only where they provide useful traceability;
- README describing the target and current qualification stage.

Do not copy historical 0.28.1 allowed differences wholesale. Every retained HTTPX2 difference must be re-observed or intentionally inherited with evidence that upstream behavior is unchanged.

Initial status must be unqualified/baseline—not Stage C.

Acceptance:
- [ ] profile pins exactly `httpx2==2.12.0` and matching `httpcore2` requirements;
- [ ] generated reference artifact records package/version identity;
- [ ] no qualification SHA is claimed before implementation/evidence exists.

## 3. Generalize oracle tooling only where needed

The existing `generate_httpx_api_manifest.py` already accepts arbitrary `--package`; retain that generic behavior.

Audit `compare_httpx_api_manifest.py`, Stage C category tooling, extended validation, and downstream scripts for hard-coded `httpx` paths/package names. Refactor only the assumptions required to select a compatibility profile/package/version from explicit arguments or profile data.

Requirements:

- one comparator implementation for both families;
- stable schema unless a real new data requirement justifies a versioned schema change;
- no duplicated `compare_httpx2_api_manifest.py` clone;
- existing HTTPX 0.28.1 commands/results remain byte/semantically compatible where practical;
- all script changes are qualification-sensitive and must land before final freeze.

Acceptance:
- [ ] old 0.28.1 oracle tests remain green;
- [ ] HTTPX2 reference/candidate manifests can be compared using the same core tooling;
- [ ] path/package selection is explicit rather than inferred from current working directory.

## 4. Generate a complete public API delta

Generate manifests for:

1. upstream `httpx==0.28.1`;
2. upstream `httpx2==2.12.0`;
3. current `eggfetch.compat.httpx`;
4. initial `eggfetch.compat.httpx2` candidate skeleton/re-export structure if present.

Classify every HTTPX2-vs-0.28.1 public delta into:

- inherited/unchanged behavior;
- new required public API;
- changed required behavior/signature;
- optional-extra surface;
- intentional difference;
- not applicable to EggFetch's supported runtime;
- deferred with explicit stage rationale.

Known areas requiring explicit classification include `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`, SSE, WebSockets, header operators, warning/status additions, Python-version claims, and aliasing behavior.

Acceptance:
- [ ] zero unclassified public symbols/signature changes;
- [ ] delta report distinguishes package rename noise from behavioral changes;
- [ ] every implementation plan item maps to stable parity IDs or manifest deltas.

## 5. Inventory upstream behavioral changes since 0.28.1

Do not rely only on API manifests. Build a changelog/test inventory for behavior that can change without signatures:

- truststore/default SSL verification;
- IPv6 CIDR `NO_PROXY`;
- chained decoder limit and streaming decompression memory behavior;
- stream closure on decode failure;
- multipart header validation;
- WSGI Transfer-Encoding and buffered body-length semantics;
- cookie extraction changes;
- default-encoding callable returning `None`;
- IDNA handling fixes;
- status aliases/constants;
- WebSocket/SSE correctness/security behavior.

For each, identify the upstream test(s) or a minimal differential probe suitable for EggFetch.

Acceptance:
- [ ] behavior-only changes have parity-case IDs and reference probes;
- [ ] recent HTTPX2 security/correctness fixes are represented in the implementation acceptance set.

## 6. Define optional-extra policy

HTTPX2 includes optional features whose dependencies/transport assumptions differ from core HTTPX:

- `ws` WebSocket support;
- zstd/brotli extras;
- CLI extras;
- Pyodide/jsfetch transport.

Define the Stage C target before implementation. Recommended classification:

- WebSocket: required for the declared HTTPX2-compatible optional `ws` surface if EggFetch chooses to expose that extra;
- SSE: required public behavior because it is native HTTPX2 API;
- Pyodide/jsfetch: `not-applicable` for the Rust native extension distribution unless EggFetch intentionally adds a WebAssembly product;
- CLI cosmetic/dependency extras: classify independently from Python client API.

Acceptance:
- [ ] optional-extra expectations are explicit and cannot silently broaden Stage C later.

## 7. Python-version support decision

HTTPX2 supports newer Python versions than EggFetch's current packaged wheel matrix. Separate API compatibility from distribution support.

Audit PyO3/abi3 and package metadata for Python 3.14/3.15 feasibility. Do not advertise interpreter support merely because pure-Python HTTPX2 supports it. If expansion is not in this program, record it as a packaging/runtime scope difference rather than forcing new CI matrices.

Acceptance:
- [ ] profile states the exact Python versions EggFetch qualifies;
- [ ] reference support and EggFetch distribution support are not conflated.

## 8. Baseline differential corpus

Add candidate-vs-reference tests that intentionally fail/xfail or record gaps until implementation plans close. Keep current 0.28.1 compatibility suite independently runnable.

Required clusters:

- API objects/signatures;
- auth and URL/origin;
- headers/method helpers;
- TLS/proxy/no_proxy;
- compression/multipart/WSGI;
- SSE;
- WebSocket optional surface.

The baseline is an engineering inventory, not qualification evidence.

## Non-goals

- Stage C qualification in this plan;
- deleting `compat/httpx/0.28.1`;
- globally aliasing `httpx` to HTTPX2 during ordinary EggFetch imports;
- implementing all deltas before they are classified;
- adding Pyodide/browser networking;
- new CI architecture.

## Exit criteria

- [ ] `compat/httpx2/2.12.0/` exists with pinned independent reference data;
- [ ] shared oracle tooling can target both HTTPX families;
- [ ] every public and behavior-only delta is classified;
- [ ] parity IDs/probes exist for implementation work;
- [ ] 0.28.1 compatibility machinery remains independently green;
- [ ] HTTPX2 remains explicitly unqualified until later plans and final freeze complete.