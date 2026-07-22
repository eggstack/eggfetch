#!/usr/bin/env python3
"""Soak test profiles for eggfetch.

Runs long-running workloads and produces machine-readable JSON reports.
Intended to be run manually or in CI with a special flag, not in
every PR.

Usage:
    python crates/eggfetch-python/tests/soak_test.py [--duration SECONDS] [--output FILE]
"""

import argparse
import http.server
import json
import os
import resource
import sys
import threading
import time
from datetime import datetime, timezone


def get_rss_bytes():
    """Get current RSS in bytes (Linux only)."""
    try:
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    kb = int(line.split()[1])
                    return kb * 1024
    except (FileNotFoundError, ValueError, IndexError):
        pass
    return 0


def get_fd_count():
    """Get current file descriptor count (Linux only)."""
    try:
        return len(os.listdir("/proc/self/fd"))
    except FileNotFoundError:
        return 0


class _EchoHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"OK"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


def start_server():
    srv = http.server.HTTPServer(("127.0.0.1", 0), _EchoHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    return srv, f"http://127.0.0.1:{port}"


def workload_sustained_requests(client, base_url, iterations):
    """Issue sustained sequential requests."""
    for _ in range(iterations):
        resp = client.get(f"{base_url}/test")
        assert resp.status_code == 200
        resp.close()


def workload_client_churn(iterations):
    """Create and destroy clients repeatedly."""
    import eggfetch
    for _ in range(iterations):
        c = eggfetch.Client()
        c.close()


def run_soak(duration_seconds):
    """Run soak workloads for the specified duration."""
    import eggfetch

    srv, base_url = start_server()
    report = {
        "tool": "eggfetch-soak-test",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "duration_seconds": duration_seconds,
        "platform": sys.platform,
        "python_version": sys.version,
        "workloads": [],
        "final_rss_bytes": 0,
        "final_fd_count": 0,
    }

    start = time.monotonic()
    iteration = 0

    try:
        while time.monotonic() - start < duration_seconds:
            iteration += 1
            elapsed = time.monotonic() - start

            # Workload 1: sustained requests
            client = eggfetch.Client()
            wl_start = time.monotonic()
            workload_sustained_requests(client, base_url, 50)
            wl_elapsed = time.monotonic() - wl_start
            client.close()

            report["workloads"].append({
                "name": "sustained_requests",
                "iteration": iteration,
                "elapsed_seconds": round(wl_elapsed, 3),
                "rss_bytes": get_rss_bytes(),
                "fd_count": get_fd_count(),
            })

            # Workload 2: client churn
            wl_start = time.monotonic()
            workload_client_churn(10)
            wl_elapsed = time.monotonic() - wl_start

            report["workloads"].append({
                "name": "client_churn",
                "iteration": iteration,
                "elapsed_seconds": round(wl_elapsed, 3),
                "rss_bytes": get_rss_bytes(),
                "fd_count": get_fd_count(),
            })

    except KeyboardInterrupt:
        pass
    finally:
        srv.shutdown()

    report["final_rss_bytes"] = get_rss_bytes()
    report["final_fd_count"] = get_fd_count()
    report["iterations"] = iteration

    return report


def main():
    parser = argparse.ArgumentParser(description="eggfetch soak test")
    parser.add_argument("--duration", type=int, default=30, help="Duration in seconds")
    parser.add_argument("--output", type=str, default=None, help="Output file path")
    args = parser.parse_args()

    report = run_soak(args.duration)

    output = json.dumps(report, indent=2)
    if args.output:
        with open(args.output, "w") as f:
            f.write(output)
        print(f"Report written to {args.output}")
    else:
        print(output)


if __name__ == "__main__":
    main()
