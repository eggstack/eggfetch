"""Build an isolated environment, install one eggfetch wheel, and smoke-test it."""

from __future__ import annotations

import argparse
import http.server
import subprocess
import sys
import tempfile
import threading
import venv
from pathlib import Path


class Handler(http.server.BaseHTTPRequestHandler):
    """Serve one buffered and one streaming response for the wheel smoke test."""

    def do_GET(self) -> None:  # noqa: N802 - stdlib protocol method
        body = b"wheel-smoke"
        if self.path == "/stream":
            body = b"chunk-one\nchunk-two\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.path == "/stream":
            midpoint = len(body) // 2
            self.wfile.write(body[:midpoint])
            self.wfile.flush()
            self.wfile.write(body[midpoint:])
        else:
            self.wfile.write(body)

    def log_message(self, _format: str, *_args: object) -> None:
        return


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wheel-dir", type=Path, required=True)
    args = parser.parse_args()

    wheels = sorted(args.wheel_dir.glob("*.whl"))
    if len(wheels) != 1:
        raise SystemExit(f"expected exactly one wheel in {args.wheel_dir}, found {wheels}")

    with tempfile.TemporaryDirectory(prefix="eggfetch-wheel-smoke-") as directory:
        venv_dir = Path(directory) / "venv"
        venv.EnvBuilder(with_pip=True, clear=True).create(venv_dir)
        python = venv_dir / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        subprocess.run(
            [str(python), "-m", "pip", "install", "--quiet", str(wheels[0])],
            check=True,
        )

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            base = f"http://127.0.0.1:{server.server_address[1]}"
            smoke = f"""
import eggfetch

response = eggfetch.get({base!r} + "/")
assert response.status_code == 200
assert response.content == b"wheel-smoke"

with eggfetch.Client() as client:
    with client.stream("GET", {base!r} + "/stream") as streamed:
        assert b"".join(streamed.iter_bytes()) == b"chunk-one\\nchunk-two\\n"
"""
            subprocess.run([str(python), "-c", smoke], check=True)
        finally:
            server.shutdown()


if __name__ == "__main__":
    main()
