#!/usr/bin/env python3
"""Execute a qualification-only HTTP/3 impairment matrix.

The script is an evidence coordinator, not a packet manipulation tool. A
platform-specific runner (network namespaces/netem, an interop simulator, or
an equivalent controlled environment) is supplied by the qualifier and is
invoked once per matrix scenario. Missing host capabilities are recorded as
``unsupported`` rather than treated as passes.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MATRIX = ROOT / "qualification" / "http3" / "impairment-matrix.json"
STATUSES = {"pass", "fail", "unsupported"}


def load_matrix(path: Path) -> dict:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read impairment matrix {path}: {exc}") from exc
    if not isinstance(value, dict) or value.get("schema_version") != 1:
        raise ValueError("unsupported impairment matrix schema")
    scenarios = value.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise ValueError("impairment matrix must contain scenarios")
    seen = set()
    for scenario in scenarios:
        if not isinstance(scenario, dict) or not isinstance(scenario.get("id"), str):
            raise ValueError("each impairment scenario needs a string id")
        if scenario["id"] in seen:
            raise ValueError(f"duplicate impairment scenario {scenario['id']}")
        seen.add(scenario["id"])
        if not isinstance(scenario.get("invariants"), list) or not scenario["invariants"]:
            raise ValueError(f"scenario {scenario['id']} has no invariants")
    return value


def execute(runner: Path, scenario: dict, output_dir: Path) -> dict:
    scenario_id = scenario["id"]
    runner_output = output_dir / f"{scenario_id}.json"
    env = dict(os.environ)
    env["EGGFETCH_H3_IMPAIRMENT_SCENARIO"] = scenario_id
    env["EGGFETCH_H3_IMPAIRMENT_OUTPUT"] = str(runner_output)
    completed = subprocess.run(
        [str(runner), scenario_id, str(runner_output)],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if runner_output.exists():
        try:
            with runner_output.open(encoding="utf-8") as handle:
                observed = json.load(handle)
        except (OSError, json.JSONDecodeError) as exc:
            return {"scenario": scenario_id, "status": "fail", "error": str(exc)}
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
            observed["status"] = "fail"
            observed["error"] = "overall pass conflicts with invariant result"
        observed.setdefault("scenario", scenario_id)
        observed.setdefault("runner_return_code", completed.returncode)
        return observed
    return {
        "scenario": scenario_id,
        "status": "fail" if completed.returncode else "unsupported",
        "runner_return_code": completed.returncode,
        "output_tail": (completed.stdout + "\n" + completed.stderr).strip()[-8000:],
        "reason": "runner did not write a result artifact",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix", type=Path, default=MATRIX)
    parser.add_argument("--runner", type=Path, help="executable taking SCENARIO_ID OUTPUT_JSON")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scenario", action="append", help="run only this scenario; repeatable")
    args = parser.parse_args(argv)
    try:
        matrix = load_matrix(args.matrix)
        scenarios = matrix["scenarios"]
        selected = set(args.scenario or [scenario["id"] for scenario in scenarios])
        known = {scenario["id"] for scenario in scenarios}
        unknown = selected - known
        if unknown:
            raise ValueError(f"unknown impairment scenarios: {sorted(unknown)}")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        run_dir = args.output.parent / f"{args.output.stem}-scenarios"
        run_dir.mkdir(parents=True, exist_ok=True)
        results = []
        for scenario in scenarios:
            if scenario["id"] not in selected:
                results.append({"scenario": scenario["id"], "status": "unsupported", "reason": "not selected"})
            elif args.runner is None:
                results.append({"scenario": scenario["id"], "status": "unsupported", "reason": "no platform runner supplied"})
            else:
                results.append(execute(args.runner, scenario, run_dir))
        result = {
            "schema_version": 1,
            "program": "http3-independent-interop-and-impairment-qualification",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "matrix": str(args.matrix),
            "results": results,
            "status": "pass" if all(item["status"] == "pass" for item in results) else "blocked",
        }
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError) as exc:
        print(f"h3 impairment qualification failed closed: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
