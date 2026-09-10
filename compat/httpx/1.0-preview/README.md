# HTTPX 1.0 Preview Tracking (non-qualified reconnaissance)

Status: **preview only — no parity claim, no qualification SHA, no Stage**.

This directory is reference data, not production evidence. Nothing here may
be read as `compat/httpx/0.28.1/` or `compat/httpx2/2.12.0/` qualification
evidence. There is no `profile.toml` here by design, and no CI gate reads
this directory.

## Observed upstream (2026-09-10)

- Stable production contract retained: PyPI `httpx` latest stable `0.28.1`.
- Pre-releases exist but are hidden by default: `pip index versions httpx`
  shows `0.28.1` as latest; `pip index versions --pre httpx` lists
  `1.0.dev1` (2025-07-02), `dev2` (2025-08-04), `dev3` (2025-09-15),
  `dev4` (2026-08-19), `dev5` (2026-08-21), and `dev6` (2026-08-31).
- Pinned preview reference: `httpx==1.0.dev6` (`httpx-1.0.dev6-py3-none-any.whl`,
  ~37 KiB; `0.28.1` is ~72 KiB). The wheel was downloaded into an isolated
  venv and never installed into the qualification environment.
- Upstream tagline for the redesign: "An HTTP toolkit ... Supports both
  client and server functionality ... tightly constrained API ... minimal
  footprint" (sole dependency `truststore`).

## Scope

- Track the original HTTPX 1.0 redesign (client vs toolkit/server scope,
  sync/async organization, transport customization, request/response/URL
  models, streaming/upgrade semantics, TLS/proxy, timeouts/limits,
  auth/cookies/redirects, optional helpers, exception taxonomy,
  public/private boundary) as `reuse` / `adapter` / `engine` /
  `not-applicable` / `unknown` — no implementation from dev releases.
- Preview failures never invalidate the HTTPX 0.28.1 Stage C claim.
- Preview tests, if any, live outside the required compatibility suites.

## Migration trigger (opens a new implementation program; never renames this file)

1. Upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. Release notes indicate no further major compatibility reset before stable;
3. A fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

The same trigger is recorded in `plans/README.md` and `plans/ROADMAP.md`.

## Artifacts

- This README (exact observed version/date, trigger).
- `reference-api.json` — generated `1.0.dev6` manifest (36 symbols) via
  `scripts/generate_httpx_api_manifest.py --package httpx` in an isolated
  venv. Do not hand-edit; regenerate on refresh. Not production evidence.
- `preview-delta-0.28.1-vs-1.0.dev6.json` — machine-readable delta vs the
  `0.28.1` reference (192 differences, empty allowed set, ungated) via the
  shared `scripts/compare_httpx_api_manifest.py`. The existing
  `allowed-differences.toml` schema does not fit preview gating (it is the
  Stage C gate for the pinned stable contract), so the preview delta is
  stored ungated and never enforced in CI.
- `preview-status.toml` — machine-readable preview status (explicit
  `preview-only`, no `qualification-sha`, no Stage). Not a `profile.toml`.
- `redesign-notes.md` — bucket-by-bucket impact classification (`reuse` /
  `adapter` / `engine` / `not-applicable` / `unknown`) plus
  removed/reintroduced notes.
- No `profile.toml` Stage claim is attached to any dev release.
- Nothing here may be read as `compat/httpx/0.28.1/` production evidence.

## Delta headline (shared tooling, ungated)

`0.28.1` (69 symbols) vs `1.0.dev6` (36 symbols): 59 removed top-level
symbols, 26 added, 7 signature/base redesigns plus 52 member diffs (192
total). `Client` collapses from 20 constructor params to
`(url, headers, transport)`; `AsyncClient`, all auth/cookie/proxy/timeout/
limit config, transports/mounts, the exception taxonomy, and top-level
helpers are gone; `Server`/`Connection`/`ConnectionPool`/`Transport`/
`NetworkBackend`/`NetworkStream`/`Stream`/`Content` hierarchy and URL-codec
helpers are new. `__all__` lists `StatusCode`/`run`/`serve_http`, which raise
`AttributeError` in dev6 (broken dev exports — further proof the API is
still churning). Detail: `redesign-notes.md` and
`preview-delta-0.28.1-vs-1.0.dev6.json`.

## Upstream tests (reconnaissance only)

No preview behavioral suite is gated. The dev release's broken exports and
absent subsystems (auth, cookies, proxy, timeouts, async) make behavioral
probing low-signal; only the API-manifest probe above is recorded. Preview
failures never invalidate the `0.28.1` Stage C claim, and no preview test
lives in the required compatibility suites.

## Refresh discipline

A preview refresh updates only generated/reference/docs data in this
directory. Shared-script changes land before the parent program freeze with
full revalidation; none are made in preview refreshes. No automated polling,
release watcher, or new CI job.
