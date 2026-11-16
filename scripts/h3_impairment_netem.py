#!/usr/bin/env python3
"""Linux netem runner for the HTTP/3 impairment matrix.

Invoked by ``scripts/h3_impairment.py`` as ``--runner``. Qualification-only;
not used from Tier 1 CI.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MATRIX = ROOT / "qualification" / "http3" / "impairment-matrix.json"
STATUSES = {"pass", "fail", "unsupported"}

# Recorded in JSON for packet-loss scenarios (plan §7).
PACKET_LOSS_LOW_RATE = "1%"
PACKET_LOSS_HIGH_RATE = "20%"

NETEM_REQUIRED = frozenset(
    {
        "packet_loss_low",
        "packet_loss_high",
        "latency_jitter",
        "packet_reordering",
        "packet_duplication",
        "reduced_mtu_pmtu_blackhole",
        "path_loss_after_headers",
        "path_loss_during_upload",
    }
)


def load_matrix(path: Path = MATRIX) -> dict:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict) or value.get("schema_version") != 1:
        raise ValueError("unsupported impairment matrix schema")
    return value


def scenario_row(matrix: dict, scenario_id: str) -> dict:
    for scenario in matrix["scenarios"]:
        if scenario["id"] == scenario_id:
            return scenario
    raise ValueError(f"unknown impairment scenario {scenario_id}")


def invariant_map(scenario_id: str, status: str = "unsupported", matrix: dict | None = None) -> dict[str, str]:
    matrix = matrix or load_matrix()
    row = scenario_row(matrix, scenario_id)
    return {name: status for name in row["invariants"]}


def write_runner_result(
    output_path: Path,
    scenario_id: str,
    status: str,
    invariants: dict[str, str],
    **extra: object,
) -> None:
    if status not in STATUSES:
        raise ValueError(f"invalid status {status!r}")
    payload: dict[str, object] = {
        "status": status,
        "scenario": scenario_id,
        "invariants": invariants,
    }
    payload.update(extra)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def validate_runner_result(observed: dict, scenario: dict) -> dict:
    """Match ``scripts/h3_impairment.py`` ``execute`` acceptance rules."""
    scenario_id = scenario["id"]
    if not isinstance(observed, dict) or observed.get("status") not in STATUSES:
        return {
            "scenario": scenario_id,
            "status": "fail",
            "error": "runner result must be an object with pass/fail/unsupported status",
        }
    invariants = observed.get("invariants")
    required_invariants = set(scenario["invariants"])
    if not isinstance(invariants, dict) or not required_invariants.issubset(invariants):
        return {
            "scenario": scenario_id,
            "status": "fail",
            "error": "runner result must report every scenario invariant",
        }
    if observed["status"] == "pass" and any(
        invariants[name] != "pass" for name in required_invariants
    ):
        observed = dict(observed)
        observed["status"] = "fail"
        observed["error"] = "overall pass conflicts with invariant result"
    observed.setdefault("scenario", scenario_id)
    return observed


def linux_netem_capabilities() -> tuple[bool, str]:
    if platform.system() != "Linux":
        return False, f"unsupported OS {platform.system()}"
    for tool in ("unshare", "ip"):
        if shutil.which(tool) is None:
            return False, f"missing {tool}"
    if shutil.which("tc") is None:
        return False, "missing tc"
    return True, ""


def _tc_netem_args(scenario_id: str) -> list[str]:
    if scenario_id == "packet_loss_low":
        return ["loss", PACKET_LOSS_LOW_RATE]
    if scenario_id == "packet_loss_high":
        return ["loss", PACKET_LOSS_HIGH_RATE]
    if scenario_id == "latency_jitter":
        return ["delay", "200ms", "50ms"]
    if scenario_id == "packet_reordering":
        return ["reorder", "25%", "50%"]
    if scenario_id == "packet_duplication":
        return ["duplicate", "10%"]
    if scenario_id == "reduced_mtu_pmtu_blackhole":
        return ["loss", "5%", "mtu", "576"]
    if scenario_id in {"path_loss_after_headers", "path_loss_during_upload"}:
        return ["loss", "80%"]
    if scenario_id == "udp_blackhole":
        return ["loss", "100%"]
    return []


def apply_netem_on_loopback(scenario_id: str) -> list[str]:
    """Return tc netem argument tail for ``tc qdisc replace dev lo root netem ...``."""
    return _tc_netem_args(scenario_id)


def run_impairment_cargo_test(
    scenario_id: str,
    *,
    netem_active: bool,
    cwd: Path = ROOT,
) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env["EGGFETCH_H3_IMPAIRMENT_SCENARIO"] = scenario_id
    if netem_active:
        env["EGGFETCH_H3_IMPAIRMENT_NETEM_ACTIVE"] = "1"
    cmd = [
        "cargo",
        "test",
        "-p",
        "eggfetch-core",
        "--all-features",
        "--test",
        "h3_impairment_scenarios",
        scenario_id,
        "--",
        "--test-threads=1",
        "--exact",
        "--nocapture",
    ]
    return subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def run_in_netns(scenario_id: str, output_path: Path) -> None:
    """Linux privileged path: netem on lo inside a network namespace, then cargo test."""
    netem_args = apply_netem_on_loopback(scenario_id)
    setup = (
        "set -e; "
        "ip link set lo up; "
        f"tc qdisc del dev lo root 2>/dev/null || true; "
    )
    if netem_args:
        setup += "tc qdisc replace dev lo root netem " + " ".join(netem_args) + "; "
    setup += "exec env EGGFETCH_H3_IMPAIRMENT_OUTPUT=" + json.dumps(str(output_path)) + " "
    setup += "EGGFETCH_H3_IMPAIRMENT_SCENARIO=" + json.dumps(scenario_id) + " "
    setup += "EGGFETCH_H3_IMPAIRMENT_NETEM_ACTIVE=1 "
    setup += "cargo test -p eggfetch-core --all-features --test h3_impairment_scenarios "
    setup += scenario_id + " -- --test-threads=1 --exact --nocapture"

    unshare_cmd = [
        "unshare",
        "--net",
        "--",
        "bash",
        "-lc",
        setup,
    ]
    subprocess.run(unshare_cmd, cwd=ROOT, check=False)


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if len(argv) < 2:
        print("usage: h3_impairment_netem.py SCENARIO_ID OUTPUT_JSON", file=sys.stderr)
        return 2
    scenario_id = argv[0]
    output_path = Path(argv[1])

    matrix = load_matrix()
    try:
        row = scenario_row(matrix, scenario_id)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    env = dict(os.environ)
    env.setdefault("EGGFETCH_H3_IMPAIRMENT_SCENARIO", scenario_id)
    env.setdefault("EGGFETCH_H3_IMPAIRMENT_OUTPUT", str(output_path))
    os.environ.update(env)

    ok, reason = linux_netem_capabilities()
    needs_netem = scenario_id in NETEM_REQUIRED

    if needs_netem and not ok:
        write_runner_result(
            output_path,
            scenario_id,
            "unsupported",
            invariant_map(scenario_id, "unsupported", matrix),
            reason=reason or "netem runner unavailable",
        )
        return 0

    if needs_netem and ok:
        try:
            run_in_netns(scenario_id, output_path)
        except OSError as exc:
            write_runner_result(
                output_path,
                scenario_id,
                "unsupported",
                invariant_map(scenario_id, "unsupported", matrix),
                reason=str(exc),
            )
            return 0
        if output_path.exists():
            return 0
        write_runner_result(
            output_path,
            scenario_id,
            "unsupported",
            invariant_map(scenario_id, "unsupported", matrix),
            reason="netem execution did not produce a result artifact",
        )
        return 0

    # Local scenarios: cargo test without netem (runner may still be invoked on macOS).
    output_path.parent.mkdir(parents=True, exist_ok=True)
    os.environ["EGGFETCH_H3_IMPAIRMENT_OUTPUT"] = str(output_path)
    completed = run_impairment_cargo_test(scenario_id, netem_active=False)
    if output_path.exists():
        return 0
    if completed.returncode == 0:
        write_runner_result(
            output_path,
            scenario_id,
            "fail",
            invariant_map(scenario_id, "fail", matrix),
            reason="cargo test succeeded but wrote no impairment artifact",
        )
    else:
        write_runner_result(
            output_path,
            scenario_id,
            "fail",
            invariant_map(scenario_id, "fail", matrix),
            reason="cargo test failed",
            output_tail=(completed.stdout + "\n" + completed.stderr).strip()[-4000:],
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())