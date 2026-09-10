# HTTPX 1.0 Preview Tracking (non-qualified reconnaissance)

Status: **preview only — no parity claim, no qualification SHA, no Stage**.

## Observed upstream (2026-09-10)

- PyPI `httpx` latest stable: `0.28.1`. No `1.0.dev*` / `1.0rc*` release is
  published on PyPI at this writing (`pip index versions httpx` lists
  0.28.1 as latest; no 1.0 pre-releases).
- The program plan referenced `httpx==1.0.dev6 (2026-08-31)` as the preview
  observed during planning. That artifact is not available as a pinned
  PyPI distribution in this environment, so no reference manifest is
  generated here. When an RC/stable 1.0 appears, pin it exactly and
  generate `reference-api.json` with the shared tooling
  (`scripts/generate_httpx_api_manifest.py --package httpx`).

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

## Artifacts

- This README (exact observed version/date, trigger).
- No `profile.toml` Stage claim is attached to any dev release.
- No `reference-api.json` is claimed until a pinned RC/stable exists.
- Nothing here may be read as `compat/httpx/0.28.1/` production evidence.
