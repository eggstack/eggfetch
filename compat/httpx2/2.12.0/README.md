# HTTPX2 2.12.0 Compatibility Profile (baseline, unqualified)

This directory is the independent, versioned compatibility profile for
`httpx2==2.12.0` (Pydantic's maintained HTTPX fork). It is a sibling of
`compat/httpx/0.28.1/`, not a replacement: `eggfetch.compat.httpx` remains
the independently pinned HTTPX 0.28.1 contract and must not be rewritten
to HTTPX2 semantics.

Status: **baseline-unqualified**. No Stage C (or any) qualification is
claimed. Implementation lives in `eggfetch.compat.httpx2` (see
`crates/eggfetch-python/python/eggfetch/compat/httpx2/`). Final
qualification happens only on the program freeze SHA per
`plans/post-next-scope-compatibility-requalification-and-closure.md`.

## Files

- `profile.toml` — reference package, surfaces, extras, categories
- `requirements.txt` — pinned `httpx2==2.12.0` + `httpcore2==2.12.0`
- `reference-api.json` — golden manifest of httpx2 2.12.0 public API (generated;
  do not hand-edit; regenerate with `scripts/generate_httpx_api_manifest.py --package httpx2`)
- `allowed-differences.toml` — reviewed intentional differences (exact typed tuples)
- `resolved-differences.toml` — audit trail of resolved gaps (empty at baseline)
- `parity-cases.toml` — differential/behavioral case registry (baseline IDs)
- `upstream-test-inventory.md` — upstream test mapping (behavior-only changes)

## Usage

```bash
python scripts/generate_httpx_api_manifest.py --package httpx2 --output compat/httpx2/2.12.0/reference-api.json
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx2/2.12.0/reference-api.json \
  --candidate /tmp/eggfetch-httpx2.json \
  --allowed compat/httpx2/2.12.0/allowed-differences.toml
```

The comparator is shared with HTTPX 0.28.1 (one implementation, explicit
paths). No `compare_httpx2_*` clone exists.

## Public delta vs HTTPX 0.28.1 (summary)

New required surface: `FunctionAuth`, `Origin` + `URL.origin`, `QUERY`
(`query` top-level + `Client.query`/`AsyncClient.query`), `Headers.__or__` /
`__ror__` / `__ior__`, SSE (`EventSource`, `ServerSentEvent`, `SSEError` +
`Client.sse`/`AsyncClient.sse`), optional WebSocket (`websocket` top-level +
`Client.websocket`/`AsyncClient.websocket`, `httpx2.websockets.*`),
`alias_httpx`. Removed: top-level `main` (deprecated entry point).

Behavior-only changes (no signature): truststore OS-trust default,
IPv6 CIDR `NO_PROXY`, chained-decoder limit + bounded streaming decode +
close-on-failure, multipart header validation, WSGI Transfer-Encoding +
buffered length, cookie extraction (perf-only unless divergent),
default-encoding `None`, IDNA non-leading-label fix, status aliases,
`HTTPXDeprecationWarning`, version/User-Agent metadata.

Optional-extra policy: `ws` is required for the declared optional surface
(wsproto framing over the existing 101 `network_stream`); SSE is required
(Python-layer framing over streamed responses); `cli` cosmetic extras and
Pyodide/jsfetch are `not-applicable`; `brotli`/`zstd` reuse native
decompression.

Python versions: httpx2 supports 3.10–3.15; eggfetch qualifies 3.10–3.13
(distribution scope, not API scope).
