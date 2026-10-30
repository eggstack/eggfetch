# Using EggFetch behind a real HTTPX client

The optional `eggfetch.integrations.httpx` module lets a third-party SDK keep
using a genuine `httpx.Client` while EggFetch handles synchronous network
requests. Install the integration extra with `pip install
"eggfetch[httpx-transport]"`. It supports HTTPX 0.28.x and requires the
matching EggFetch package installation.

```python
import httpx
from eggfetch.integrations.httpx import EggFetchTransport

transport = EggFetchTransport(
    trust_env=True,
    http1=True,
    http2=False,
    limits=httpx.Limits(max_connections=8, max_keepalive_connections=4),
)
client = httpx.Client(transport=transport, timeout=60.0)
try:
    response = client.get("https://example.com/")
    response.raise_for_status()
finally:
    client.close()
```

The HTTPX client remains responsible for SDK-visible request construction,
authentication, hooks, redirects, and exception handling. EggFetch receives
HTTPX's timeout extension and has internal retries disabled so the SDK or
application owns retries. Closing the HTTPX client closes the EggFetch
transport; closing or exhausting a response closes its EggFetch response
stream.

Pass connection options to `EggFetchTransport`, because HTTPX does not pass
client TLS, proxy, UDS, local-address, socket-option, or limit settings into a
custom transport. The integration is synchronous. It does not provide
`httpx.AsyncBaseTransport`; async SDK clients must continue using their
existing transport. Async request streams are unsupported. Request/response
streaming is synchronous and follows HTTPX stream lifecycle rules.

The current bridge is intended for HTTP/1.1 and HTTP/2. EggFetch's experimental
HTTP/3 route is not selected through this HTTPX integration. Its header-read
timeout behavior is separately documented in the core timeout contract.

The supported HTTPX range is intentionally limited to 0.28.x. Do not install
the `eggfetch.compat.httpx` facade as a replacement for the public `httpx`
package: this integration composes the public HTTPX transport interface and
keeps both distributions intact.
