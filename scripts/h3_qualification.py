#!/usr/bin/env python3
"""Run the opt-in HTTP/3 qualification corpus and emit an audit ledger.

This runner intentionally has no third-party dependencies and never runs as
part of Tier 1. It validates adapter identity and endpoint hygiene, executes
the deterministic local control suite, and executes the capability-selected
external cases one adapter at a time. Unsupported capabilities remain
explicit in the JSON result; they are never counted as passes.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "qualification" / "http3" / "corpus.json"
ALLOWED_CASES = {
    "get",
    "head",
    "buffered_post",
    "streaming_upload",
    "large_streaming_download",
    "response_trailers",
    "multiplexed_requests",
    "connection_reuse",
}
CLIENT_DRIVEN_CASES = {
    "cancellation_before_headers",
    "cancellation_during_upload",
    "cancellation_during_download",
}
SERVER_CONTROLLED_CASES = {
    "stream_reset",
    "graceful_connection_close",
    "goaway_reconnect",
    "alternative_port",
}
CONTROL_ENV_KEYS = {
    "stream_reset": "EGGFETCH_H3_INTEROP_CONTROL_STREAM_RESET",
    "graceful_connection_close": "EGGFETCH_H3_INTEROP_CONTROL_GRACEFUL_CONNECTION_CLOSE",
    "goaway_reconnect": "EGGFETCH_H3_INTEROP_CONTROL_GOAWAY_RECONNECT",
    "alternative_port": "EGGFETCH_H3_INTEROP_CONTROL_ALTERNATIVE_PORT",
}
KNOWN_CONTROL_KEYS = frozenset(CONTROL_ENV_KEYS)
CORPUS_CASE_LINE = re.compile(r"^H3_CORPUS_CASE (\w+)=(\w+)")
STATUSES = {"pass", "fail", "unsupported", "not_run", "skip"}


class ManifestError(ValueError):
    """Raised when a qualification manifest is unsafe or malformed."""


def load_json(path: Path) -> dict:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        raise ManifestError(f"cannot read JSON manifest {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ManifestError(f"JSON manifest {path} must contain an object")
    return value


def validate_endpoint(endpoint: object) -> str:
    if not isinstance(endpoint, str) or not endpoint:
        raise ManifestError("server endpoint must be a non-empty URL")
    parsed = urlsplit(endpoint)
    if parsed.scheme != "https" or not parsed.hostname:
        raise ManifestError(f"server endpoint must be an https URL with a hostname: {endpoint!r}")
    if parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ManifestError(
            "server endpoint must not contain credentials, query strings, or fragments: "
            f"{endpoint!r}"
        )
    return endpoint


def validate_corpus(corpus: dict) -> list[dict]:
    if corpus.get("schema_version") != 1:
        raise ManifestError("unsupported corpus schema_version")
    cases = corpus.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ManifestError("corpus must contain a non-empty cases list")
    seen: set[str] = set()
    for case in cases:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            raise ManifestError("each corpus case must have a string id")
        case_id = case["id"]
        if case_id in seen:
            raise ManifestError(f"duplicate corpus case: {case_id}")
        seen.add(case_id)
        if not isinstance(case.get("required"), bool) or not isinstance(
            case.get("server_control"), bool
        ):
            raise ManifestError(f"case {case_id} must declare required and server_control")
    return cases


def validate_controls(server: dict, index: int, capabilities: list[str]) -> dict[str, str]:
    if "controls" not in server:
        return {}
    controls = server["controls"]
    if not isinstance(controls, dict):
        raise ManifestError(f"server {index} controls must be an object when present")
    capability_set = set(capabilities)
    validated: dict[str, str] = {}
    for key, url in controls.items():
        if key not in KNOWN_CONTROL_KEYS:
            raise ManifestError(f"server {index} declares unknown control key: {key!r}")
        if key not in capability_set:
            raise ManifestError(
                f"server {index} control {key!r} is present but {key} is not in capabilities"
            )
        validated[key] = validate_endpoint(url)
    return validated


def validate_server_manifest(manifest: dict, corpus: list[dict]) -> list[dict]:
    if manifest.get("schema_version") != 1:
        raise ManifestError("unsupported server manifest schema_version")
    servers = manifest.get("servers")
    if not isinstance(servers, list) or not servers:
        raise ManifestError("server manifest must contain a non-empty servers list")
    corpus_ids = {case["id"] for case in corpus}
    validated = []
    for index, server in enumerate(servers):
        if not isinstance(server, dict):
            raise ManifestError(f"server {index} must be an object")
        for field in ("implementation", "version", "identity", "endpoint", "capabilities"):
            if field not in server:
                raise ManifestError(f"server {index} is missing {field}")
        if not isinstance(server["implementation"], str) or not server["implementation"]:
            raise ManifestError(f"server {index} has no implementation name")
        if not isinstance(server["version"], str) or not server["version"]:
            raise ManifestError(f"server {index} has no version")
        identity = server["identity"]
        if not isinstance(identity, dict) or not any(
            isinstance(identity.get(key), str) and identity[key]
            for key in ("source_revision", "image_digest")
        ):
            raise ManifestError(
                f"server {index} identity must include a source_revision or image_digest"
            )
        capabilities = server["capabilities"]
        if not isinstance(capabilities, list) or not all(
            isinstance(item, str) for item in capabilities
        ):
            raise ManifestError(f"server {index} capabilities must be a string list")
        unknown = set(capabilities) - corpus_ids
        if unknown:
            raise ManifestError(f"server {index} declares unknown capabilities: {sorted(unknown)}")
        validate_endpoint(server["endpoint"])
        if not isinstance(server.get("notes", ""), str):
            raise ManifestError(f"server {index} notes must be a string")
        controls = validate_controls(server, index, capabilities)
        server_entry = dict(server)
        server_entry["validated_controls"] = controls
        validated.append(server_entry)
    return validated


def run_command(command: list[str], env: dict[str, str], cwd: Path) -> tuple[int, str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    output = (completed.stdout + "\n" + completed.stderr).strip()
    return completed.returncode, output[-12000:]


def case_result(case_id: str, status: str, **extra: object) -> dict:
    if status not in STATUSES:
        raise ValueError(f"invalid result status: {status}")
    return {"case": case_id, "status": status, **extra}


def local_test_passed(output: str, test_name: str) -> bool:
    """Require the named deterministic test to appear as an actual pass."""
    return any(
        line.startswith(f"test {test_name} ") and line.endswith("... ok")
        for line in output.splitlines()
    )


def parse_corpus_case_lines(output: str) -> dict[str, str]:
    observed: dict[str, str] = {}
    for line in output.splitlines():
        match = CORPUS_CASE_LINE.match(line.strip())
        if match is None:
            continue
        case_id, status = match.group(1), match.group(2)
        observed[case_id] = status
    return observed


def attempted_cases_for_server(server: dict) -> list[str]:
    capabilities = server["capabilities"]
    controls = server.get("validated_controls") or {}
    attempted: list[str] = []
    for case_id in capabilities:
        if case_id in ALLOWED_CASES or case_id in CLIENT_DRIVEN_CASES:
            attempted.append(case_id)
        elif case_id in SERVER_CONTROLLED_CASES:
            if controls.get(case_id):
                attempted.append(case_id)
    return sorted(attempted)


def run_local(cases: list[dict], env: dict[str, str]) -> dict:
    command = [
        "cargo",
        "test",
        "-p",
        "eggfetch-core",
        "--all-features",
        "--test",
        "h3_interop_qualification",
        "--",
        "--test-threads=1",
    ]
    child_env = dict(env)
    child_env.pop("EGGFETCH_H3_INTEROP_URLS", None)
    child_env.pop("EGGFETCH_H3_INTEROP_ENDPOINT", None)
    child_env.pop("EGGFETCH_H3_INTEROP_CASES", None)
    for key in CONTROL_ENV_KEYS.values():
        child_env.pop(key, None)
    return_code, output = run_command(command, child_env, ROOT)
    known_local = {
        "get": "local_quinn_self_interop_get_and_body",
        "head": "h3_head_method_succeeds",
        "buffered_post": "h3_post_with_buffered_body_succeeds",
        "streaming_upload": "h3_streaming_upload_succeeds",
        "large_streaming_download": "h3_large_streaming_download",
        "multiplexed_requests": "h3_concurrent_multiplexed_requests",
        "response_trailers": "h3_response_trailers_surface_after_eof",
        "cancellation_before_headers": "cancellation_during_h3_connect_is_prompt",
        "cancellation_during_upload": "h3_cancellation_during_upload_is_prompt",
        "cancellation_during_download": "h3_cancellation_during_download_is_prompt",
        "stream_reset": "h3_early_close_terminates_and_client_recovers",
        "graceful_connection_close": "h3_graceful_connection_close_reconnects",
        "goaway_reconnect": "h3_goaway_reconnect_observes_metrics",
        "alternative_port": "h3_alternative_port_via_altsvc",
        "connection_reuse": "hundred_sequential_requests_on_reused_h3_connection",
        "server_restart": "server_restart_reconnects_on_next_request",
        "ipv4": "h3_ipv4_loopback_get_succeeds",
        "ipv6": "ipv6_availability_is_detected_not_assumed",
    }
    results = []
    for case in cases:
        case_id = case["id"]
        test_name = known_local.get(case_id)
        if test_name is None:
            results.append(case_result(case_id, "unsupported", reason="no deterministic local fixture"))
        elif return_code == 0 and local_test_passed(output, test_name):
            results.append(case_result(case_id, "pass", test=test_name))
        else:
            reason = "named deterministic test did not report pass"
            results.append(case_result(case_id, "fail", test=test_name, reason=reason, output=output))
    return {
        "status": "pass" if return_code == 0 else "fail",
        "command": command,
        "return_code": return_code,
        "cases": results,
        "output_tail": output,
    }


def run_server(server: dict, corpus: list[dict], env: dict[str, str]) -> dict:
    capabilities = set(server["capabilities"])
    controls = server.get("validated_controls") or {}
    attempted = attempted_cases_for_server(server)
    results: list[dict] = []
    for case in corpus:
        case_id = case["id"]
        if case_id not in capabilities:
            results.append(
                case_result(case_id, "unsupported", reason="adapter did not declare capability")
            )
        elif case_id not in attempted:
            reason = "requires adapter-specific control"
            if case_id in SERVER_CONTROLLED_CASES and not controls.get(case_id):
                reason = "server-controlled case missing control URL"
            results.append(case_result(case_id, "unsupported", reason=reason))
    if not attempted:
        return {
            "implementation": server["implementation"],
            "version": server["version"],
            "identity": server["identity"],
            "endpoint": server["endpoint"],
            "status": "unsupported",
            "cases": results,
            "notes": "No executable cases were declared (missing control URLs or generic cases).",
        }
    child_env = dict(env)
    child_env["EGGFETCH_H3_INTEROP_ENDPOINT"] = server["endpoint"]
    child_env["EGGFETCH_H3_INTEROP_CASES"] = ",".join(attempted)
    child_env.pop("EGGFETCH_H3_INTEROP_URLS", None)
    for case_id, env_key in CONTROL_ENV_KEYS.items():
        child_env.pop(env_key, None)
    for case_id, url in controls.items():
        child_env[CONTROL_ENV_KEYS[case_id]] = url
    command = [
        "cargo",
        "test",
        "-p",
        "eggfetch-core",
        "--all-features",
        "--test",
        "h3_interop_qualification",
        "external_h3_interop_servers_if_configured",
        "--",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]
    return_code, output = run_command(command, child_env, ROOT)
    observed = parse_corpus_case_lines(output)
    attempted_set = set(attempted)
    results_by_case = {entry["case"]: entry for entry in results}
    for case_id in attempted:
        line_status = observed.get(case_id)
        if line_status == "pass":
            status = "pass"
            reason = None
        else:
            status = "fail"
            reason = (
                "external test did not print H3_CORPUS_CASE pass"
                if return_code == 0
                else "external interop test failed"
            )
        extra: dict[str, object] = {"implementation_test": True}
        if reason is not None:
            extra["reason"] = reason
        if return_code != 0 and status == "fail":
            extra["output"] = output
        results_by_case[case_id] = case_result(case_id, status, **extra)
    merged_cases = [results_by_case[case["id"]] for case in corpus if case["id"] in results_by_case]
    any_fail = any(entry["status"] == "fail" for entry in merged_cases if entry["case"] in attempted_set)
    return {
        "implementation": server["implementation"],
        "version": server["version"],
        "identity": server["identity"],
        "endpoint": server["endpoint"],
        "status": "fail" if return_code != 0 or any_fail else "pass",
        "command": command,
        "return_code": return_code,
        "cases": merged_cases,
        "output_tail": output,
    }


def build_result(manifest: dict, corpus: list[dict], local: dict, servers: list[dict]) -> dict:
    passed_independent = sum(server["status"] == "pass" for server in servers)
    return {
        "schema_version": 1,
        "program": "http3-independent-interop-and-impairment-qualification",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "git_revision": git_revision(),
        "deterministic_local": local,
        "independent_servers": {
            "required_passes": 2,
            "passed_implementations": passed_independent,
            "status": "pass" if passed_independent >= 2 else "blocked",
            "servers": servers,
        },
        "impairment": {
            "status": "not_run",
            "matrix": "qualification/http3/impairment-matrix.json",
            "note": "Run scripts/h3_impairment.py with a namespace/netem runner.",
        },
        "public_origins": {
            "status": "not_run",
            "note": "Record manual checks separately; volatile public hosts are not routine fixtures.",
        },
        "graduation": {
            "experimental_label_retained": passed_independent < 2,
            "status": "eligible_for_child-plan-review" if passed_independent >= 2 else "blocked",
        },
        "manifest_metadata": manifest.get("metadata", {}),
    }


def git_revision() -> str | None:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, help="server JSON manifest")
    parser.add_argument("--output", type=Path, required=True, help="machine-readable result JSON")
    parser.add_argument(
        "--local-only",
        action="store_true",
        help="run only deterministic local controls without an external-server manifest",
    )
    parser.add_argument(
        "--danger-accept-invalid-certs",
        action="store_true",
        help="qualification-only opt-in for local adapter certificates; never use for public evidence",
    )
    args = parser.parse_args(argv)
    try:
        corpus = validate_corpus(load_json(CORPUS))
        if args.local_only:
            if args.manifest is not None:
                raise ManifestError("--local-only cannot be combined with --manifest")
            manifest = {"schema_version": 1, "servers": [], "metadata": {"mode": "local-only"}}
            servers = []
        elif args.manifest is None:
            raise ManifestError("--manifest is required unless --local-only is used")
        else:
            manifest = load_json(args.manifest)
            servers = validate_server_manifest(manifest, corpus)
        env = dict(os.environ)
        if args.danger_accept_invalid_certs:
            env["EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS"] = "1"
        local = run_local(corpus, env)
        server_results = [run_server(server, corpus, env) for server in servers]
        result = build_result(manifest, corpus, local, server_results)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    except (ManifestError, OSError, ValueError) as exc:
        print(f"h3 qualification failed closed: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2))
    local_ok = result["deterministic_local"]["status"] == "pass"
    if args.local_only:
        # Local-only acceptance is the deterministic corpus. Independent
        # servers stay blocked in the JSON and are never counted as passes.
        return 0 if local_ok else 1
    external_ok = result["independent_servers"]["status"] == "pass"
    return 0 if local_ok and external_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())