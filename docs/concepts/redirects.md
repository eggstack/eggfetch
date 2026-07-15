# Redirects

eggfetch follows redirects with configurable policy, method rewriting, sensitive header stripping, and body replayability checks.

## Redirect Policy

By default, redirects are not followed. Enable redirect following:

```rust
let client = Client::builder()
    .redirect_policy(RedirectPolicy::new(true, 20))
    .build();
```

```python
client = eggfetch.Client(follow_redirects=True, max_redirects=20)
```

```bash
eggfetch --follow --max-redirects 20 https://example.com
```

The `max_redirects` setting limits the total number of redirect hops. If the limit is exceeded, a `TooManyRedirects` error is returned.

## Method Rewriting

eggfetch applies standard method rewriting rules on redirects:

| Status | Method change | Body |
|--------|--------------|------|
| 301 / 302 | POST becomes GET | Dropped |
| 303 (See Other) | All non-HEAD methods become GET | Dropped |
| 307 / 308 | Method preserved | Preserved |

For 301, 302, and 303 redirects that drop the body, body-specific headers (`Content-Length`, `Content-Type`, `Transfer-Encoding`) are also removed. This prevents a GET request from carrying stale body headers.

HEAD requests are always preserved as HEAD, even on 303 redirects.

## Sensitive Header Stripping

On cross-origin redirects (different scheme, host, or port), the following headers are stripped to prevent credential leakage:

- `Authorization`
- `Cookie`
- `Proxy-Authorization`

Same-origin redirects preserve all headers. Client-level auth is not reapplied on cross-origin redirect hops. This prevents accidental credential forwarding to third-party origins.

## Body Replayability

For 307 and 308 redirects that preserve the method and body, the redirect engine checks whether the body is replayable. Empty and byte bodies are replayable and can be cloned for the redirect. Stream bodies are not replayable, and attempting to redirect with a stream body returns a `BodyNotReplayableForRedirect` error.

## Redirect History

The response includes a `history` field containing metadata about each redirect hop. Each entry records the status code, URL, and headers of the intermediate response. The final response's body is the one returned to the caller.

```python
response = client.get("https://example.com/old", follow_redirects=True)

for entry in response.history:
    print(f"{entry.status_code} -> {entry.url}")

print(f"Final URL: {response.url}")
print(f"Final status: {response.status_code}")
```

## Cross-Origin Cookie Policy

Request-local cookies (set via the `cookies` kwarg) are stripped on cross-origin redirects. Client-jar cookies are recomputed for the new destination based on domain and path matching. This prevents session cookies from leaking to third-party origins during redirects.

## URL Resolution

Redirect locations are resolved against the original URL. Relative paths, scheme-relative URLs, and absolute URLs are all supported:

- `/new-path` resolves relative to the original host
- `//other.com/path` resolves to the same scheme on the new host
- `https://other.com/path` is used as-is

Only `http` and `https` redirect targets are allowed. Redirects to other schemes (e.g., `ftp://`) produce an error. Redirect URLs containing userinfo (e.g., `user:pass@host`) are also rejected.

## Loop Detection

The `max_redirects` limit acts as loop detection. If a server returns a redirect chain longer than the limit, the request fails with `TooManyRedirects`. This prevents infinite loops from misconfigured servers.

## Total Timeout

The total timeout applies across all redirect hops. If the total timeout expires during a redirect chain, the request fails with a timeout error. Each redirect hop does not reset the total deadline.

## Python API

```python
# Follow redirects (default: off)
response = client.get(url, follow_redirects=True)

# Limit redirects
response = client.get(url, follow_redirects=True, max_redirects=5)

# Access redirect history
for entry in response.history:
    print(entry.status_code, entry.url)

# Async
async with eggfetch.AsyncClient(follow_redirects=True) as client:
    response = await client.get(url)
```

## CLI

```bash
# Follow redirects (default)
eggfetch --follow https://example.com

# Disable redirect following
eggfetch --no-follow https://example.com

# Limit redirects
eggfetch --follow --max-redirects 5 https://example.com
```
