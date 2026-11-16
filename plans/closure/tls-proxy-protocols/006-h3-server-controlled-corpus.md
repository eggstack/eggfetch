# TLS, Proxy and Protocols Milestone 006 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/006-h3-server-controlled-corpus.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-6--server-controlled-h3-corpus`

Repository baseline reviewed: `f11a46b28df88f43e923d5211f82985784daaf40`

Implementation commits:

- `8d599e8402929f5388ed9e1d970833d8f942a52b` — harness and runner changes under
  `crates/eggfetch-core/tests/h3_interop_qualification.rs`,
  `scripts/h3_qualification.py`, `scripts/test_h3_qualification.py`,
  `qualification/http3/servers.example.json`, and architecture docs.

## 1. Executive finding

M006 is **closed**. Every corpus id now has a local Quinn/h3 fixture mapped
through `known_local`, the external path executes client-driven cancellation
and optional `controls` URLs, and the Tier 1 runner self-test covers the new
manifest rules. HTTP/3 remains experimental. Independent non-Quinn evidence
is still owned by M010. Exact-SHA Stage C is rebound to
`6b571028767dde4b0a78f36e6ecef525c6d68482`.

## 2. Requirement-to-evidence matrix

| Corpus id | Local test | External trigger | Result |
|---|---|---|---|
| `get` | `local_quinn_self_interop_get_and_body` | endpoint | local pass |
| `head` | `h3_head_method_succeeds` | endpoint | local pass |
| `buffered_post` | `h3_post_with_buffered_body_succeeds` | endpoint | local pass |
| `streaming_upload` | `h3_streaming_upload_succeeds` | endpoint | local pass |
| `large_streaming_download` | `h3_large_streaming_download` | endpoint | local pass |
| `response_trailers` | `h3_response_trailers_surface_after_eof` | endpoint | local pass |
| `multiplexed_requests` | `h3_concurrent_multiplexed_requests` | endpoint | local pass |
| `cancellation_before_headers` | `cancellation_during_h3_connect_is_prompt` | client-driven | local pass |
| `cancellation_during_upload` | `h3_cancellation_during_upload_is_prompt` | client-driven | local pass |
| `cancellation_during_download` | `h3_cancellation_during_download_is_prompt` | client-driven | local pass |
| `stream_reset` | `h3_early_close_terminates_and_client_recovers` | `controls.stream_reset` | local pass |
| `graceful_connection_close` | `h3_graceful_connection_close_reconnects` | `controls.graceful_connection_close` | local pass |
| `goaway_reconnect` | `h3_goaway_reconnect_observes_metrics` | `controls.goaway_reconnect` | local pass |
| `connection_reuse` | `hundred_sequential_requests_on_reused_h3_connection` | endpoint | local pass |
| `ipv4` | `h3_ipv4_loopback_get_succeeds` | n/a local | local pass |
| `ipv6` | `ipv6_availability_is_detected_not_assumed` | capability probe | local pass / SKIP |
| `alternative_port` | `h3_alternative_port_via_altsvc` | `controls.alternative_port` | local pass |
| `server_restart` | `server_restart_reconnects_on_next_request` | adapter-specific | local pass |

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Capability without control → unsupported | `scripts/test_h3_qualification.py` | pass | Cannot inflate independent_servers pass |
| Unknown control key rejected | same | pass | ManifestError |
| Control URL hygiene | same | pass | No credentials/query/fragment |
| Experimental label retained | architecture doc | pass | No promotion |
| Stage C renewal | ledger | **pass** | Rebound to `6b571028767dde4b0a78f36e6ecef525c6d68482` |

## 3. Production implementation evidence

No production API or route behavior change. Qualification harness only:
path-controlled / shutdown-after-one-response server helpers, Alt-Svc
alternative-port local fixtures, extended external env vars
(`EGGFETCH_H3_INTEROP_CONTROL_*`), and per-case `H3_CORPUS_CASE` parsing.

## 4. Verification executed

### Commands run

```bash
python3 scripts/test_h3_qualification.py
cargo test -p eggfetch-core --all-features --test h3_interop_qualification -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery -- --test-threads=1
python3 scripts/h3_qualification.py --local-only --output target/h3-qualification-local.json
```

### Results

- Self-test: 14 passed
- `h3_interop_qualification`: 31 passed on Darwin arm64
- `h3_hardening` / `h3_alt_svc_discovery`: green
- Local-only coordinator: every corpus id reports pass; exit 0
- Workspace Rust tests in `./scripts/check.sh`: green (including new H3 targets)
- Full Tier 1 blocked on this host after Rust by `maturin develop` building an
  `x86_64` wheel that the arm64 venv rejects (`not a supported wheel on this
  platform`). Host accommodation, not an H3 corpus defect.
- Independent servers: not claimed

## 5. Invariant review

`Http3Only` unchanged for generic external cases; `Auto` only for
`alternative_port`. No new public surface. Redaction unchanged. No shell
command in manifests.

## 6. Failure, timeout, pool, and recovery review

Cancellation asserts prompt return and a usable client. GOAWAY path
observes `h3_reconnected` / drain metrics without string-matching errors.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

`http3` feature gate unchanged. Lean/CLI/FFI/Node profiles unchanged.
Executable inputs changed; Stage C renewed on
`6b571028767dde4b0a78f36e6ecef525c6d68482`. Do not cite September H3 freeze
`639bf186…`.

## 8. Security review

No new trust-store path. `EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS`
remains the only qualification-only cert bypass and stays off by default.

## 9. Documentation and operations

Architecture interop paragraph updated. Historical evidence ledgers under
`plans/http3-*` were not edited. `servers.example.json` remains a template.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| — | Stage C rebound | Closed | Exact-SHA binding `6b571028767dde4b0a78f36e6ecef525c6d68482` |
| info | No independent-server run | Expected | M010 operational evidence |

## 11. Roadmap disposition

M006 harness complete. M010 still blocked on independent servers for
graduation; this closure does not unblock promotion.

## 12. Registry updates

M006 → closed on Stage C binding `6b571028767dde4b0a78f36e6ecef525c6d68482` (harness freeze `8d599e8402929f5388ed9e1d970833d8f942a52b`). Keep H3 experimental gated
future. State that M010 cannot treat local Quinn corpus as independent
interop.
