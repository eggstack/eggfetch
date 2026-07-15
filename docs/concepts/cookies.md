# Cookies

eggfetch implements RFC 6265 cookie handling with domain/path matching, thread-safe storage, and client-level state.

## Cookie Properties

Each cookie has the following attributes:

- **name** and **value** -- the cookie data
- **domain** -- the domain the cookie applies to
- **path** -- the path prefix for cookie scope
- **secure** -- whether the cookie is only sent over HTTPS
- **httpOnly** -- whether the cookie is inaccessible to JavaScript
- **sameSite** -- cross-site policy (Strict, Lax, or None)
- **expires** / **max-age** -- when the cookie expires

## CookieJar

`CookieJar` is a thread-safe cookie store. Each `Client` owns a `CookieJar` that accumulates cookies across requests. The jar handles:

- **set** -- store a cookie
- **get** -- retrieve matching cookies for a URL
- **delete** -- remove a specific cookie
- **clear** -- remove all cookies
- **update_from_response** -- ingest `Set-Cookie` headers from a response
- **cookies_for_url** -- compute which cookies to send for a URL

## Domain and Path Matching

When the client sends a request, it selects cookies whose domain matches the request host and whose path is a prefix of the request path.

### Host-Only vs Domain Cookies

Cookies set without a `Domain` attribute are **host-only**: they are only sent to the exact host that set them. Cookies set with an explicit `Domain` attribute are sent to the specified domain and all subdomains.

For example, a cookie set with `Domain=example.com` is sent to `example.com` and `sub.example.com`. A cookie set without a `Domain` attribute is only sent to the exact host in the `Set-Cookie` response.

### Path Matching

The cookie's `Path` attribute is matched as a prefix of the request path. A cookie with `Path=/api` matches `/api/users` but not `/api2/users`. If no `Path` is specified, the default path is the directory portion of the request URL path.

### Secure Flag

Cookies with `secure=true` are only sent over HTTPS connections. Sending a secure cookie over plain HTTP is a protocol violation.

### Expiry Handling

Cookies with `Max-Age=0` or a past `Expires` date are removed automatically. Negative `Max-Age` values are treated as zero. The jar cleans up expired cookies during matching.

## Client-Level Cookie State

The client jar persists cookies across requests. Cookies set by a response are automatically added to the jar via `Set-Cookie` header ingestion and sent on subsequent matching requests.

Request-local cookies (set via the `cookies` kwarg) are sent with that specific request but are not added to the persistent jar. They are stripped on cross-origin redirects.

## Cookie/Header Interaction

Cookies are serialized as a `Cookie` header before sending. If a raw `Cookie` header is already set on the request, the jar cookies are not added (the explicit header takes precedence). Request-local cookies and jar cookies are merged, with request-local values taking precedence for duplicate names.

## Python API

```python
# Initialize client with pre-set cookies
client = eggfetch.Client(cookies={"session": "abc123"})

# Send request-local cookies (not persisted in jar)
response = client.get(url, cookies={"pref": "dark"})

# Access response Set-Cookie values
for cookie in response.cookies:
    print(cookie.name, cookie.value)

# Access persistent client jar cookies
for cookie in client.cookies:
    print(cookie.name, cookie.value, cookie.domain)
```

### Response.cookies

`response.cookies` returns a `Cookies` mapping of `Set-Cookie` values from the response. These are the cookies the server wants to set, not necessarily all cookies that will be sent on future requests.

### Client.cookies

`client.cookies` exposes the persistent cookie jar. Changes here affect all subsequent requests through that client.

## CLI

```bash
# Send cookies with a request
eggfetch --cookie "session=abc123" --cookie "pref=dark" https://example.com

# Read cookies from a file (Netscape format)
eggfetch --cookie-jar cookies.txt https://example.com
```

## Cookie/Auth Interaction

Disabling auth (via `auth=eggfetch.NOAUTH`) does not affect cookie handling. Cookies are sent independently of the auth state. A request can carry both cookies and an Authorization header, or cookies without auth, or auth without cookies.

## Security Considerations

- HttpOnly cookies are not accessible to JavaScript in browser contexts, but eggfetch sends them normally since it is not a browser
- SameSite=Strict cookies are sent only for same-site requests; eggfetch does not perform same-site checks since it has no concept of "site"
- Cross-origin redirects strip the `Cookie` header to prevent credential leakage to third-party origins
- Cookie values are not validated beyond header safety; application-level validation is the caller's responsibility
