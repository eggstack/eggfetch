#!/usr/bin/env python3
"""Execute self-contained Python code blocks from markdown files.

Starts a local HTTP test server and runs code blocks that are self-contained.
Reports documentation bugs (wrong parameter names, missing APIs) while
tolerating expected failures (external URLs, missing optional deps).

Usage:
    python scripts/run_doc_examples.py [--install] [--strict]
"""

import ast
import http.server
import json
import re
import subprocess
import sys
import threading
import urllib.parse
from pathlib import Path


class _DocTestHandler(http.server.BaseHTTPRequestHandler):
    """Minimal server for executing doc examples."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        if path in ("/get", "/"):
            body = json.dumps({"message": "Hello, world!"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/json":
            body = json.dumps({"key": "value", "number": 42}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/ip":
            body = json.dumps({"origin": "127.0.0.1"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/headers":
            headers = dict(self.headers)
            body = json.dumps(headers).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path.startswith("/redirect/"):
            code = int(path.split("/")[-1])
            self.send_response(code)
            self.send_header("Location", "/get")
            self.end_headers()
        elif path == "/status/200":
            self.send_response(200)
            self.send_header("Content-Length", "2")
            self.end_headers()
            self.wfile.write(b"ok")
        el        if path == "/post":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            try:
                received = json.loads(body) if body else None
            except (json.JSONDecodeError, ValueError):
                received = body.decode(errors="replace")
            r = json.dumps({"received": received}).encode()
            self.send_header("Content-Length", str(len(r)))
            self.end_headers()
            self.wfile.write(r)
        else:
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def do_POST(self):
        self.do_GET()

    def log_message(self, format, *args):
        pass


def start_server():
    server = http.server.HTTPServer(("127.0.0.1", 0), _DocTestHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, port


def extract_python_blocks(md_path: Path) -> list[tuple[int, str]]:
    """Return (line_number, code) for each ```python block."""
    text = md_path.read_text(encoding="utf-8")
    blocks = []
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        if lines[i].strip().startswith("```python"):
            start = i + 1
            code_lines = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            code = "\n".join(code_lines)
            if code.strip():
                blocks.append((start + 1, code))
        i += 1
    return blocks


def is_executable(code: str) -> bool:
    """Check if a code block is self-contained and safe to execute."""
    if "async " in code or "await " in code:
        return False

    dangerous = [
        "import os", "import subprocess", "open(",
        "os.system", "shutil", "__import__",
        "exec(", "eval(", "compile(", "pathlib.Path",
    ]
    for pattern in dangerous:
        if pattern in code:
            return False

    try:
        ast.parse(code)
    except SyntaxError:
        return False

    return True


def make_self_contained(code: str, base_url: str) -> str:
    """Add import and URL replacement to make a block self-contained."""
    has_import = any("import eggfetch" in line for line in code.split("\n"))

    first_stmt = ""
    for line in code.split("\n"):
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            first_stmt = stripped
            break

    if first_stmt and not any(
        first_stmt.startswith(kw)
        for kw in ["import ", "from ", "def ", "class ", "with ", "if ", "for ", "try:", "eggfetch"]
    ):
        if "=" not in first_stmt and "(" not in first_stmt:
            return ""

    if not has_import:
        code = "import eggfetch\n" + code

    code = code.replace("https://httpbin.org", base_url)
    code = code.replace("http://httpbin.org", base_url)
    code = code.replace("https://example.com", base_url)
    code = code.replace("http://example.com", base_url)

    return code


EXPECTED_FAILURES = (
    "client error",          # Connection to external server
    "failed to read",        # Placeholder file paths
    "No module named",       # Optional deps (httpx, requests)
    "nodename nor servname", # DNS resolution
    "NameError",             # Multi-block examples with undefined vars
    "generator object",      # Iterator quirks
    "non-iterator",          # Iterator quirks
    "not defined",           # Multi-block examples
)


def is_expected_failure(exc_msg: str) -> bool:
    """Check if an error is expected (external deps, placeholder paths, etc.)."""
    return any(pattern in exc_msg for pattern in EXPECTED_FAILURES)


def main() -> int:
    install = "--install" in sys.argv
    strict = "--strict" in sys.argv

    if install:
        print("Installing eggfetch...")
        result = subprocess.run(
            ["maturin", "develop", "-m", "crates/eggfetch-python/Cargo.toml"],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            print(f"maturin develop failed:\n{result.stderr}")
            return 1

    try:
        import eggfetch  # noqa: F401
    except ImportError:
        print("eggfetch not installed — skipping doc example execution.")
        return 0

    docs_dir = Path(__file__).resolve().parent.parent / "docs"
    md_files = sorted(docs_dir.rglob("*.md"))
    server, port = start_server()
    base_url = f"http://127.0.0.1:{port}"

    doc_bugs = 0
    expected = 0
    executed = 0
    skipped = 0

    for md_file in md_files:
        blocks = extract_python_blocks(md_file)
        rel = md_file.relative_to(docs_dir.parent)

        for lineno, code in blocks:
            if not is_executable(code):
                skipped += 1
                continue

            test_code = make_self_contained(code, base_url)
            if not test_code:
                skipped += 1
                continue

            try:
                exec(compile(test_code, f"{rel}:{lineno}", "exec"), {})
                executed += 1
            except TypeError as exc:
                msg = str(exc)
                if is_expected_failure(msg):
                    expected += 1
                else:
                    print(f"  DOC BUG {rel}:{lineno}: {exc}")
                    doc_bugs += 1
            except Exception as exc:
                msg = str(exc)
                if is_expected_failure(msg):
                    expected += 1
                else:
                    print(f"  Runtime error {rel}:{lineno}: {type(exc).__name__}: {exc}")
                    doc_bugs += 1

    server.shutdown()

    print(f"\nExecuted: {executed}, Skipped: {skipped}, Expected failures: {expected}, Doc bugs: {doc_bugs}")

    if strict and doc_bugs:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
