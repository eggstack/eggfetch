# Compatibility Stage Decision

## Current decision

The documented asyncio-supported public surfaces for `eggfetch.compat.httpx`
(HTTPX 0.28.1) and `eggfetch.compat.httpx2` (HTTPX2 2.12.0) passed Stage C
qualification on frozen executable SHA
`48ff1a39733c295906fd873cae757cc9eb149d6c` (2026-11-21, the 0.4.0 release
qualification), where the full compatibility, extended, package, and live
security gates passed. That renewal re-established the binding Core Transport
M011 had invalidated. Earlier bindings are historical.

That qualification is the last completed one, but the executable-input changes
that followed it — the `rustix`/`url`/`cookie` dependency bumps, two core
fixture determinism fixes, and the free-threaded (PEP 703) wheel work —
invalidate the binding, and a fresh exact-SHA qualification is outstanding.
Current evidence and binding history are in
`plans/httpx-parity-correction-status.md`; the exact facade scopes are in
`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`.

Stage C is not an unrestricted HTTPX replacement. eggfetch does not provide
the top-level `httpx` distribution, Trio or AnyIO support, Python 3.8 or 3.9
support, or private HTTPX modules. SSL context state that cannot be represented
safely by the native TLS backend is rejected. HTTP/3 remains experimental and
is not part of this compatibility decision.

See [the compatibility matrix](compatibility.md) for supported features and
known differences.
