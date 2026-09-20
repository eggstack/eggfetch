# CLI Development Skill

Use this skill when working on the eggfetch-cli crate.

## Workflow

1. Read `docs/architecture/cli.md` for the argument model, output modes, and exit codes.
2. Read `docs/cli/guide.md` for the user-facing CLI documentation.
3. Read existing CLI source in `crates/eggfetch-cli/src/` for code conventions.

## Key Constraints

- CLI is a thin adapter over eggfetch-core. No HTTP logic here (CONNECT wire lives in `eggfetch-http-connect`).
- All I/O goes through eggfetch-core's public API.
- Exit codes: 0 success, 2 usage, 3 connect/TLS-handshake/pool/proxy transport error (TLS *verification* failures are usage errors → 2), 4 timeout, 5 protocol, 6 status (with `--check-status` on any non-2xx), 7 I/O, 130 interrupted.
- Auth/proxy/cookie headers are redacted in verbose output.
- Body sources `--body`/`--body-file`/`--json` are mutually exclusive with `--form`/`--file`; `--form` + `--file` may combine into one multipart body (see the bail message near `main.rs:914`).
- The default CLI build enables `cookies`, `multipart`, and `proxy` but compiles **no** compression decoders and no `http2`/`http3`: it never sends `Accept-Encoding`, and an encoded response body fails with exit 5 unless `--no-compress` passes it through raw. Requesting an uncompiled protocol fails instead of silently downgrading. Do not document CLI decompression as supported.

## Environment Variables

| Variable | Maps To |
|----------|---------|
| `EGGFETCH_AUTH` | `--auth` |
| `EGGFETCH_BEARER` | `--bearer` |
| `EGGFETCH_PROXY` | `--proxy` |
| `EGGFETCH_PROXY_AUTH` | Proxy auth |

## Architecture Reference

- CLI architecture: `docs/architecture/cli.md`
- CLI guide: `docs/cli/guide.md`
