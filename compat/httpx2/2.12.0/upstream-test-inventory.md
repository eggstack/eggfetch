# HTTPX2 2.12.0 upstream test inventory (behavior-only changes since 0.28.1)

Behavior that can change without signatures. Each row names the upstream
area, the probe strategy for eggfetch, and the parity-case ID.

| Upstream area | Probe | Parity ID |
|---|---|---|
| truststore default verification (`truststore.SSLContext`) | network-backed cert tests: system/private/custom roots; explicit verify/Context/cert conflicts | H2X-TLS-001 |
| IPv6 CIDR `NO_PROXY` + bracketed/unbracketed forms | versioned parser differential vs 0.28.1 pinned oddities | H2X-PROXY-001 |
| chained decoder cap + bounded streaming decode | adversarial compressed streams, decoder-count/memory bounds | H2X-COMP-001 |
| stream close on decode failure | decode-error releases body/pool lease | H2X-COMP-001 |
| multi-frame zstd split across chunks | chunk-split wire/content tests (gzip/deflate/brotli/zstd chains) | H2X-COMP-001 |
| multipart part header validation | CR/LF injection, invalid names, binary/unicode, per-part headers | H2X-MP-001 |
| WSGI Transfer-Encoding preservation | upstream-derived WSGI probes vs httpx2 facade | H2X-WSGI-001 |
| WSGI buffered body-length exposure | buffered length visible to WSGI app | H2X-WSGI-001 |
| default-encoding callable returning None | empty/default encoding behavior probe | H2X-WSGI-001 |
| IDNA non-leading-label decode | `xn--` in non-leading labels (`display=True`) | H2X-API-002 |
| cookie extraction changes | semantics probe (perf-only needs no facade change) | H2X-WSGI-001 |
| status aliases/constants + warnings | oracle + warning-category tests | H2X-META-001 |
| WebSocket/SOCKS/TLS fixes | wss-via-SOCKS, trust isolation, proxy-header non-forwarding | H2X-WS-003 |
| SSE correctness | chunk-boundary/max-event/lifecycle corpus | H2X-SSE-001/002 |
