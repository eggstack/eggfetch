# HTTPX 0.28.1 Compatibility Profile

This directory contains the machine-readable compatibility profile for
HTTPX 0.28.1. It is Stage C qualified against the exact executable SHA
`d24101be6ed7be64463813750da5b4043d9905ec` recorded in `profile.toml`;
current evidence is recorded in `plans/httpx-parity-correction-status.md`.
Executable changes require fresh qualification.

## Files

- `profile.toml` — Defines the reference package, supported surfaces, and categories
- `allowed-differences.toml` — Registry of reviewed, allowed behavioral differences with exact typed tuples
- `resolved-differences.toml` — Historical ledger of resolved differences (audit trail only)
- `reference-api.json` — Golden manifest of HTTPX 0.28.1 public API (generated)
- `upstream-test-inventory.md` — Catalog of upstream HTTPX tests mapped to eggfetch cases
- `upstream-derived-cases.toml` — Machine-readable mapping of 36 derived upstream test cases
- `performance-budgets.toml` — Latency and throughput budgets for critical paths
- `compat/downstream/` — 12-package consumer portfolio for downstream validation

## Usage

Generate the reference manifest:
```bash
python scripts/generate_httpx_api_manifest.py --package httpx --output compat/httpx/0.28.1/reference-api.json
```

Generate the eggfetch manifest:
```bash
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
```

Compare manifests:
```bash
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Categories

- `required-now` — Must match for current stage
- `required-later` — Accepted gap assigned to later roadmap phase
- `intentional-difference` — Reviewed and explicitly allowed
- `not-public` — Excluded from contract
- `not-applicable` — Reference feature cannot apply to current product stage
- `resolved` — Previously-allowed differences that have been implemented and verified (tracked in `resolved-differences.toml` as an audit trail)

## Status

This profile targets HTTPX 0.28.1. The compatibility stage is **Stage C
qualified** for the documented Python 3.10+ asyncio-supported surface.
Trio/AnyIO, Python 3.8/3.9, and private HTTPX modules remain excluded.
Proxy URL credentials, proxy headers, and proxy ssl_context are covered.
Arbitrary Python ssl_context objects that cannot be represented by
rustls are rejected at construction time with a clear TypeError.

The current bounded residuals are:

- SSLContext state that rustls cannot represent (cipher suite, ALPN,
  TLS-version policy, client-certificate provenance, and arbitrary
  non-`ssl.SSLContext` subclass contexts) fails closed with `TypeError`
  before dispatch.
- HTTP/2 `stream_id` metadata is unavailable because the current
  hyper-util abstraction does not expose the underlying h2 stream ID.
- HTTP/2 origin framing through an HTTP CONNECT proxy remains HTTP/1.1
  (parity case `H2-009`).
- HTTPX's unsafe four-element null-pointer `socket_options` form is
  rejected at the safe Rust boundary.
- Ordinary pooled HTTP/1.x and HTTP/2 connections expose no writable
  network stream; only 101 responses own an upgraded stream.
- Internal proxy CONNECT tunnels are never surfaced as writable network
  streams.
- Async coroutine trace callbacks are rejected deterministically with
  a `TypeError` before dispatch; sync callbacks work on both sync
  `Client` and `AsyncClient`.

Direct TLS, cleartext prior-knowledge, direct-specialized (local_address
/ socket_options), SNI override, SOCKS HTTPS, and UDS HTTP/2 routes are
enforced and tested. They are recorded as `parity` cases in
`parity-cases.toml` rather than as bounded differences.

## Future Drift

- **FunctionAuth**: HTTPX master (post-0.28.1) exports `FunctionAuth` in
  `__all__` (commit `ae1b9f66`, 2025-12-10). EggFetch already has an internal
  `_FunctionAuth` adapter for callable auth normalization. The pinned 0.28.1
  reference does not export `FunctionAuth`. The next stable HTTPX rebaseline
  should evaluate the public export, signature, and behavior before exporting
  or renaming the internal adapter.
