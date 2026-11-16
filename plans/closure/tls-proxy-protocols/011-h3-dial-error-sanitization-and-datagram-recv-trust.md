# TLS, Proxy and Protocols Milestone 011 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-11--h3-dial-error-sanitization-and-datagram-recv-trust`

Repository baseline reviewed: `8af32d7ca4f1140e266875798522f5da09c17850`

## 1. Executive finding

M011 is closed. Connection-kind dialer failures map to fixed-text
`Error::H3Connect` (`QUIC handshake failed`) without embedding
`DialError::message()`. `DatagramRoute::recv` now reports `(len, source)`;
the private Quinn bridge drops off-peer datagrams before enqueueing and
continues without terminating the route. Authentication / Rejected / Other
remain `CustomTransport` with provider text, matching stream `Dialer`.
H3 stays experimental; no adapter exposure of the dialer.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Connection H3Connect never embeds provider text | `connection_dial_errors_never_reexpose_provider_text`; `route_dial_error` / `route_error_override` | pass | Secret URL/password absent from Display/Debug |
| Recv source checked in bridge | `foreign_source_datagrams_are_dropped_before_quinn` | pass | Drop + continue |
| Existing datagram suites | `h3_datagram_route` (16) + `datagram::tests` (9) | pass | |
| Threat model aligned | `docs/architecture/threat-model.md` § Malicious Datagram Route | pass | CustomTransport exception documented |
| Public shapes / no Quinn leak | traits unchanged in name; recv success type refined to include `SocketAddr` | pass | Experimental Rust-only; oracle `all-features.txt` updated |
| Stage C | exact-SHA renewal after executable change | pending freeze | Recorded with M012 on shared freeze |

## 3. Production implementation evidence

- `crates/eggfetch-core/src/transport/http3.rs` — `H3_ROUTE_CONNECT_TEXT`
- `crates/eggfetch-core/src/transport/datagram.rs` — recv `(len, source)` + peer check
- Docs: threat-model, core-tls-proxy-protocols, feature-flags
- `compat/rust-public-api/all-features.txt` — `DatagramRecvFuture` Output update

## 4. Verification executed

```bash
cargo test -p eggfetch-core --all-features --lib -- datagram:: --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_datagram_route -- --test-threads=1
./scripts/check.sh   # Tier 1 (shared with M012)
```

## 5–10. Reviews

Invariants held: no direct-UDP fallback; Auth/Rejected/Other evidence path
unchanged; bounded queues; H3 experimental. No ResponseBody change.
Compatibility: intentional fixed `H3Connect` text for Connection; recv
return type is an experimental Rust-only breaking refinement (SocketAddr
already on the surface via `peer_addr`).

## 11. Closure recommendation

**Closed.** Stage C SHA rebound on the shared M011+M012 freeze commit.
