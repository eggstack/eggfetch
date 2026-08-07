"""HTTPX-compatible authentication classes for eggfetch."""

from __future__ import annotations

import base64
import hashlib
import os
import re
import stat
import typing
from pathlib import Path

from eggfetch.compat.httpx._request import Request

if typing.TYPE_CHECKING:
    from collections.abc import AsyncGenerator, Generator

    from eggfetch.compat.httpx._response import Response


class Auth:
    """Base class for authentication.

    Subclasses implement ``auth_flow`` as a generator that yields
    :class:`Request` objects and receives the corresponding
    :class:`Response` via ``.send()``.

    The client drives the flow via ``sync_auth_flow`` (sync) or
    ``async_auth_flow`` (async).  Subclasses may override these to
    provide native async auth logic.
    """

    def auth_flow(self, request: Request) -> Generator[Request, Response, None]:
        raise NotImplementedError()

    def sync_auth_flow(
        self, request: Request
    ) -> Generator[Request, Response, None]:
        """Sync auth driver — drives ``auth_flow`` as a regular generator."""
        yield from self.auth_flow(request)

    async def async_auth_flow(
        self, request: Request
    ) -> AsyncGenerator[Request, Response]:
        """Async auth driver — drives ``auth_flow`` synchronously.

        Subclasses that need real async I/O during authentication should
        override this method.  The default falls back to the sync
        ``auth_flow`` generator.
        """
        # Drive the sync generator inside an async context.
        # We iterate manually so the caller sees an async generator.
        gen = self.auth_flow(request)
        request = next(gen)
        while True:
            response = yield request
            try:
                request = gen.send(response)
            except StopIteration:
                break

    def __repr__(self) -> str:
        return f"<{type(self).__name__}>"


class BasicAuth(Auth):
    """HTTP Basic authentication.

    Args:
        username: The username credential.
        password: The password credential.
        encoding: Character encoding used for ``username:password``.
    """

    def __init__(
        self,
        username: str | None = None,
        password: str | None = None,
        *,
        encoding: str = "latin-1",
    ) -> None:
        self._username = username or ""
        self._password = password or ""
        self._encoding = encoding

    @property
    def username(self) -> str:
        return self._username

    @property
    def password(self) -> str:
        return self._password

    @property
    def encoding(self) -> str:
        return self._encoding

    def _build_auth_header(self) -> str:
        credentials = f"{self._username}:{self._password}"
        encoded = base64.b64encode(credentials.encode(self._encoding)).decode("ascii")
        return f"Basic {encoded}"

    def auth_flow(self, request: Request) -> Generator[Request, Response, None]:
        request.headers["authorization"] = self._build_auth_header()
        yield request

    def __repr__(self) -> str:
        return f"BasicAuth(username={self._username!r})"


# ---------------------------------------------------------------------------
# Digest auth helpers
# ---------------------------------------------------------------------------

def _parse_challenge(header_value: str) -> dict[str, str]:
    """Parse a ``WWW-Authenticate: Digest …`` header into a dict.

    Handles quoted and unquoted values, and the ``qop`` parameter
    (which may be a quoted list).
    """
    # Strip the scheme prefix (case-insensitive)
    match = re.match(r'^Digest\s+(.+)', header_value, re.IGNORECASE)
    if not match:
        raise ValueError(f"Not a Digest challenge: {header_value!r}")

    result: dict[str, str] = {}
    remaining = match.group(1)

    # Match key=value or key="value" pairs separated by commas
    pattern = re.compile(
        r'(\w+)\s*=\s*'
        r'(?:'
        r'"([^"]*)"'   # quoted value
        r'|'
        r'([^,]+)'     # unquoted value
        r')'
    )
    for m in pattern.finditer(remaining):
        key = m.group(1)
        value = m.group(2) if m.group(2) is not None else m.group(3).strip()
        result[key] = value

    return result


def _digest_hash(algorithm: str) -> typing.Callable[..., str]:
    """Return a callable that computes the hash for the given algorithm."""
    algo_upper = algorithm.upper()
    if algo_upper == "MD5":
        return lambda data: hashlib.md5(data).hexdigest()
    if algo_upper in ("SHA-256", "SHA256"):
        return lambda data: hashlib.sha256(data).hexdigest()
    # Default to MD5 per RFC 2617
    return lambda data: hashlib.md5(data).hexdigest()


class DigestAuth(Auth):
    """HTTP Digest authentication (RFC 2617 / RFC 7616).

    Supports MD5 and SHA-256 algorithms, ``qop=auth`` (and ``auth-int``
    when the server advertises it), and stale nonce re-authentication.

    Args:
        username: The username credential.
        password: The password credential.
    """

    def __init__(self, username: str, password: str) -> None:
        self._username = username
        self._password = password
        self._nonce_count = 0

    @property
    def username(self) -> str:
        return self._username

    @property
    def password(self) -> str:
        return self._password

    def _build_digest_response(
        self,
        method: str,
        uri: str,
        challenge: dict[str, str],
        body: bytes | None = None,
    ) -> str:
        realm = challenge.get("realm", "")
        nonce = challenge.get("nonce", "")
        algorithm = challenge.get("algorithm", "MD5")
        opaque = challenge.get("opaque", "")

        # qop may be a comma-separated list (e.g. "auth, auth-int").
        # Select the first supported value.
        raw_qop = challenge.get("qop", "")
        qop = ""
        if raw_qop:
            for candidate in raw_qop.split(","):
                candidate = candidate.strip().strip('"')
                if candidate in ("auth", "auth-int"):
                    qop = candidate
                    break

        hash_fn = _digest_hash(algorithm)

        # HA1 = H(username:realm:password)
        ha1 = hash_fn(f"{self._username}:{realm}:{self._password}".encode("utf-8"))

        # HA2 = H(method:uri)
        # For auth-int, include the body hash
        if qop == "auth-int" and body is not None:
            entity_body_hash = hash_fn(body)
            ha2 = hash_fn(f"{method}:{uri}:{entity_body_hash}".encode("utf-8"))
        else:
            ha2 = hash_fn(f"{method}:{uri}".encode("utf-8"))

        # Nonce count
        self._nonce_count += 1
        nc = f"{self._nonce_count:08x}"

        # Cnonce: a client-generated opaque value
        import secrets
        cnonce = secrets.token_hex(16)

        # Response computation
        if qop in ("auth", "auth-int"):
            response = hash_fn(
                f"{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}".encode("utf-8")
            )
        else:
            response = hash_fn(f"{ha1}:{nonce}:{ha2}".encode("utf-8"))

        # Build the Authorization header value
        parts = [
            f'username="{self._username}"',
            f'realm="{realm}"',
            f'nonce="{nonce}"',
            f'uri="{uri}"',
            f'response="{response}"',
        ]
        if qop in ("auth", "auth-int"):
            parts.append(f'qop={qop}')
            parts.append(f"nc={nc}")
            parts.append(f'cnonce="{cnonce}"')
        if opaque:
            parts.append(f'opaque="{opaque}"')
        if algorithm and algorithm.upper() != "MD5":
            parts.append(f"algorithm={algorithm}")

        return ", ".join(parts)

    def auth_flow(self, request: Request) -> Generator[Request, Response, None]:
        # First request: send without authentication
        response = yield request

        # Only handle 401 Unauthorized
        if response.status_code != 401:
            return

        # Look for Digest challenge
        www_auth = response.headers.get("www-authenticate", "")
        if not www_auth.lower().startswith("digest"):
            return

        try:
            challenge = _parse_challenge(www_auth)
        except ValueError:
            return

        # Check for stale nonce — reset count if so
        if challenge.get("stale", "").lower() == "true":
            self._nonce_count = 0

        # Determine the URI (path + query, without the scheme/host)
        url = request.url
        uri = url.path
        if url.query:
            query = url.query.decode("utf-8") if isinstance(url.query, bytes) else url.query
            uri = f"{uri}?{query}"

        # Get the request method
        method = request.method

        # Get the body for auth-int qop
        body = request.content
        if body is None:
            body = b""

        # Build the Digest authorization header
        digest_header = self._build_digest_response(
            method=method,
            uri=uri,
            challenge=challenge,
            body=body,
        )

        # Create a new request with the Authorization header
        # (We need to copy the request since we can't mutate a yielded request
        # in all generator patterns cleanly)
        auth_request = Request(
            method=request.method,
            url=request.url,
            headers=request.headers,
            content=request.content,
        )
        auth_request.headers["authorization"] = f"Digest {digest_header}"

        # Yield the authenticated request
        yield auth_request

    def __repr__(self) -> str:
        return f"DigestAuth(username={self._username!r})"


class NetRCAuth(Auth):
    """Authentication using a ``.netrc`` file.

    Looks up credentials for the request's host in the netrc file and
    applies HTTP Basic authentication if found.

    Args:
        file: Path to the netrc file.  Defaults to ``~/.netrc``.
              Also accepts ``auth_file`` as an alias.
    """

    def __init__(self, file: str | None = None, *, auth_file: str | None = None) -> None:
        path = file if file is not None else auth_file
        if path is None:
            self._auth_file = Path.home() / ".netrc"
        else:
            self._auth_file = Path(path)

    @property
    def auth_file(self) -> Path:
        return self._auth_file

    @staticmethod
    def _parse_netrc(path: Path) -> dict[str, dict[str, str]]:
        """Parse a netrc file and return a mapping of host → credentials.

        Returns an empty dict if the file does not exist or is not readable.
        """
        if not path.is_file():
            return {}

        # On Unix, respect restrictive file permissions (must be 0600 or stricter)
        if os.name != "nt":
            try:
                mode = stat.S_IMODE(os.stat(path).st_mode)
                # Allow 0600, 0400, or owner-only access
                if mode & 0o077:
                    return {}
            except OSError:
                return {}

        try:
            content = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            return {}

        hosts: dict[str, dict[str, str]] = {}
        current_host: str | None = None
        current_entry: dict[str, str] = {}

        for line in content.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue

            tokens = line.split()
            i = 0
            while i < len(tokens):
                token = tokens[i]
                if token == "machine":
                    # Save previous entry if any
                    if current_host is not None:
                        hosts[current_host] = current_entry
                    i += 1
                    if i < len(tokens):
                        current_host = tokens[i]
                        current_entry = {}
                elif token == "default":
                    # Default entry matches any host
                    if current_host is not None:
                        hosts[current_host] = current_entry
                    current_host = "default"
                    current_entry = {}
                elif token == "login":
                    i += 1
                    if i < len(tokens):
                        current_entry["login"] = tokens[i]
                elif token == "password":
                    i += 1
                    if i < len(tokens):
                        current_entry["password"] = tokens[i]
                elif token == "account":
                    i += 1
                    if i < len(tokens):
                        current_entry["account"] = tokens[i]
                elif token == "macdef":
                    # macdef is followed by a macro name, then lines until
                    # a blank line.  We skip the entire macro body.
                    i += 1  # skip macro name
                    # Skip tokens until we see a blank line (end of macro)
                    while i < len(tokens):
                        # In tokenized form we can't detect blank lines;
                        # just skip the rest of the line
                        break
                i += 1

        # Save last entry
        if current_host is not None:
            hosts[current_host] = current_entry

        return hosts

    def _lookup_credentials(self, host: str) -> tuple[str, str] | None:
        """Look up login/password for *host* in the netrc file.

        Returns ``(login, password)`` or ``None`` if not found.
        """
        entries = self._parse_netrc(self._auth_file)

        # Exact match first
        if host in entries:
            entry = entries[host]
            login = entry.get("login", "")
            password = entry.get("password", "")
            if login:
                return (login, password)

        # Try default entry (case-insensitive per RFC)
        if "default" in entries:
            entry = entries["default"]
            login = entry.get("login", "")
            password = entry.get("password", "")
            if login:
                return (login, password)

        return None

    def auth_flow(self, request: Request) -> Generator[Request, Response, None]:
        host = request.url.host
        if host:
            creds = self._lookup_credentials(host)
            if creds is not None:
                login, password = creds
                basic = BasicAuth(username=login, password=password)
                # Delegate to BasicAuth's flow
                yield from basic.auth_flow(request)
                return

        # No credentials found — pass through unchanged
        yield request

    def __repr__(self) -> str:
        return f"NetRCAuth(auth_file={str(self._auth_file)!r})"
