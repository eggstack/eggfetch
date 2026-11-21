# Compatibility Stage Decision

## Current decision

The documented asyncio-supported public surfaces for `eggfetch.compat.httpx`
(HTTPX 0.28.1) and `eggfetch.compat.httpx2` (HTTPX2 2.12.0) passed Stage C
qualification on frozen executable SHA
`cef0e7aa5c23ef92f4a378f370516d9dfbaa466d` (2026-11-16), where the full
compatibility, extended, package, and live security gates passed.

**That binding is invalidated.** Core Transport M011 changed executable inputs
after it; the milestone's freeze `49f8910d…` carries Tier 1 evidence only, and
the current tree is unqualified. A fresh exact-SHA qualification is required
before any Stage C binding can be cited. Earlier bindings are historical.
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
