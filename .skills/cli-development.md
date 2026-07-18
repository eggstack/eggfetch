# CLI Development Skill

Use this skill when working on the eggfetch-cli crate.

## Workflow

1. Read `docs/architecture/cli.md` for the argument model, output modes, and exit codes.
2. Read `docs/cli/guide.md` for the user-facing CLI documentation.
3. Read existing CLI source in `crates/eggfetch-cli/src/` for code conventions.

## Key Constraints

- CLI is a thin adapter over eggfetch-core. No HTTP logic here.
- All I/O goes through eggfetch-core's public API.
- Exit codes: 0 success, 2 usage, 3 connect/TLS, 4 timeout, 5 protocol, 6 status (with `--check-status`), 7 I/O, 130 interrupted.
- Auth/proxy/cookie headers are redacted in verbose output.
- Body modes are mutually exclusive except `--form` + `--file`.

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
