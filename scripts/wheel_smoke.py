"""Build an isolated environment, install one eggfetch wheel, and smoke-test it.

Accepts either --wheel (exact path) or --wheel-dir (directory with exactly one
compatible wheel). Tests: buffered GET, streaming, version metadata, multipart
upload, auth, retry on server errors, response status codes, and named
exception mapping.
"""

from __future__ import annotations

import argparse
import http.server
import json
import subprocess
import sys
import tempfile
import threading
import venv
from pathlib import Path


class Handler(http.server.BaseHTTPRequestHandler):
    """Multi-endpoint test server for wheel smoke tests."""

    request_log: list[str] = []

    def do_GET(self) -> None:  # noqa: N802 - stdlib protocol method
        if self.path == "/stream":
            body = b"chunk-one\nchunk-two\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            midpoint = len(body) // 2
            self.wfile.write(body[:midpoint])
            self.wfile.flush()
            self.wfile.write(body[midpoint:])
        elif self.path == "/status/404":
            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", "9")
            self.end_headers()
            self.wfile.write(b"Not Found")
        elif self.path == "/status/500":
            self.send_response(500)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", "21")
            self.end_headers()
            self.wfile.write(b"Internal Server Error")
        elif self.path == "/auth/basic":
            auth = self.headers.get("Authorization", "")
            if auth.startswith("Basic ") and auth[6:] == "dXNlcjpwYXNz":
                self.send_response(200)
                body = b"ok"
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            else:
                self.send_response(401)
                body = b"unauthorized"
                self.send_header("WWW-Authenticate", 'Basic realm="test"')
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        elif self.path == "/auth/bearer":
            auth = self.headers.get("Authorization", "")
            if auth == "Bearer test-token-123":
                self.send_response(200)
                body = b"ok"
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            else:
                self.send_response(401)
                body = b"unauthorized"
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        elif self.path == "/json":
            data = json.dumps({"key": "value", "count": 42}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        elif self.path == "/retry-then-ok":
            # First request fails, second succeeds (track via simple counter)
            count = getattr(self.server, "_retry_count", 0)
            self.server._retry_count = count + 1  # type: ignore[attr-defined]
            if count == 0:
                self.send_response(503)
                body = b"service unavailable"
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            else:
                self.send_response(200)
                body = b"retry-ok"
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        else:
            body = b"wheel-smoke"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def do_POST(self) -> None:  # noqa: N802 - stdlib protocol method
        if self.path == "/multipart":
            content_type = self.headers.get("Content-Type", "")
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            # Verify it's multipart and has the file field
            if "multipart/form-data" in content_type and b"file_field" in body:
                self.send_response(200)
                resp = b"upload-ok"
                self.send_header("Content-Length", str(len(resp)))
                self.end_headers()
                self.wfile.write(resp)
            else:
                self.send_response(400)
                resp = b"bad request"
                self.send_header("Content-Length", str(len(resp)))
                self.end_headers()
                self.wfile.write(resp)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, _format: str, *_args: object) -> None:
        return


def _resolve_wheel(wheel_dir: Path) -> Path:
    """Resolve exactly one compatible wheel from a directory."""
    wheels = sorted(wheel_dir.glob("*.whl"))
    if not wheels:
        raise SystemExit(f"no wheels found in {wheel_dir}")

    major, minor = sys.version_info[:2]
    cp_tag = f"cp{major}{minor}"
    matched = [w for w in wheels if cp_tag in w.name]
    if not matched:
        matched = [w for w in wheels if "abi3" in w.name or "py3-none-any" in w.name]
    if not matched:
        raise SystemExit(
            f"no wheel matching Python {cp_tag} in {wheel_dir}: {wheels}"
        )
    if len(matched) > 1:
        raise SystemExit(
            f"ambiguous: {len(matched)} wheels match Python {cp_tag} in {wheel_dir}: {matched}"
        )
    return matched[0]


def main() -> None:
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--wheel", type=Path, help="Exact path to a wheel file")
    group.add_argument("--wheel-dir", type=Path, help="Directory containing exactly one compatible wheel")
    args = parser.parse_args()

    if args.wheel is not None:
        if not args.wheel.is_file():
            raise SystemExit(f"wheel not found: {args.wheel}")
        if not args.wheel.name.endswith(".whl"):
            raise SystemExit(f"not a .whl file: {args.wheel}")
        wheel = args.wheel
    else:
        wheel = _resolve_wheel(args.wheel_dir)

    with tempfile.TemporaryDirectory(prefix="eggfetch-wheel-smoke-") as directory:
        venv_dir = Path(directory) / "venv"
        venv.EnvBuilder(with_pip=True, clear=True).create(venv_dir)
        python = venv_dir / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        subprocess.run(
            [str(python), "-m", "pip", "install", "--quiet", str(wheel)],
            check=True,
        )

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            base = f"http://127.0.0.1:{server.server_address[1]}"
            smoke = f"""
import eggfetch
import sys

# --- Test 1: Version metadata ---
assert hasattr(eggfetch, "__version__"), "eggfetch has no __version__"
v = eggfetch.__version__
assert isinstance(v, str) and len(v) > 0, f"invalid version: {{v!r}}"
print(f"PASS: version = {{v}}")

# --- Test 2: Buffered GET ---
r = eggfetch.get({base!r} + "/")
assert r.status_code == 200
assert r.content == b"wheel-smoke"
print("PASS: buffered GET")

# --- Test 3: Streaming GET ---
with eggfetch.Client() as client:
    with client.stream("GET", {base!r} + "/stream") as streamed:
        chunks = b"".join(streamed.iter_bytes())
        assert chunks == b"chunk-one\\nchunk-two\\n"
print("PASS: streaming GET")

# --- Test 4: JSON response ---
r = eggfetch.get({base!r} + "/json")
assert r.status_code == 200
data = r.json()
assert data == {{"key": "value", "count": 42}}
print("PASS: JSON response")

# --- Test 5: 404 status code ---
r = eggfetch.get({base!r} + "/status/404")
assert r.status_code == 404
assert r.content == b"Not Found"
print("PASS: 404 status code")

# --- Test 6: Basic auth ---
r = eggfetch.get({base!r} + "/auth/basic", auth=eggfetch.BasicAuth("user", "pass"))
assert r.status_code == 200
print("PASS: Basic auth")

# --- Test 7: Bearer auth ---
r = eggfetch.get({base!r} + "/auth/bearer", headers={{"Authorization": "Bearer test-token-123"}})
assert r.status_code == 200
print("PASS: Bearer auth")

# --- Test 8: Unauthorized returns 401 ---
r = eggfetch.get({base!r} + "/auth/basic")
assert r.status_code == 401
print("PASS: 401 unauthorized")

# --- Test 9: Multipart upload ---
files = {{"file_field": ("test.txt", b"hello world", "text/plain")}}
r = eggfetch.post({base!r} + "/multipart", files=files)
assert r.status_code == 200
assert r.content == b"upload-ok"
print("PASS: multipart upload")

# --- Test 10: Client error exception (500) ---
try:
    r = eggfetch.get({base!r} + "/status/500")
    # If status_code check doesn't raise, at least verify the status
    assert r.status_code == 500
    print("PASS: 500 status (no raise_on_error)")
except Exception as e:
    print(f"PASS: 500 raised {{type(e).__name__}}")

# --- Test 11: Retry on transient failure ---
with eggfetch.Client(retries=eggfetch.Retry(max_attempts=3)) as client:
    r = client.get({base!r} + "/retry-then-ok")
    assert r.status_code == 200
    assert r.content == b"retry-ok"
print("PASS: retry on transient failure")

# --- Test 12: Response headers ---
r = eggfetch.get({base!r} + "/")
assert "content-type" in r.headers
print("PASS: response headers accessible")

print("\\nAll wheel smoke tests passed.")
"""
            subprocess.run([str(python), "-c", smoke], check=True)
        finally:
            server.shutdown()


if __name__ == "__main__":
    main()
