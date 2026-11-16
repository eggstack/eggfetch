#!/usr/bin/env python3
"""Unit tests for qualification-manifest safety checks."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("h3_qualification.py")
SPEC = importlib.util.spec_from_file_location("h3_qualification", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

IMPAIRMENT_SCRIPT = Path(__file__).with_name("h3_impairment.py")
IMPAIRMENT_SPEC = importlib.util.spec_from_file_location("h3_impairment", IMPAIRMENT_SCRIPT)
assert IMPAIRMENT_SPEC and IMPAIRMENT_SPEC.loader
IMPAIRMENT = importlib.util.module_from_spec(IMPAIRMENT_SPEC)
IMPAIRMENT_SPEC.loader.exec_module(IMPAIRMENT)

NETEM_SCRIPT = Path(__file__).with_name("h3_impairment_netem.py")
NETEM_SPEC = importlib.util.spec_from_file_location("h3_impairment_netem", NETEM_SCRIPT)
assert NETEM_SPEC and NETEM_SPEC.loader
NETEM = importlib.util.module_from_spec(NETEM_SPEC)
NETEM_SPEC.loader.exec_module(NETEM)


class QualificationManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        with MODULE.CORPUS.open(encoding="utf-8") as handle:
            self.corpus = json.load(handle)
        self.cases = MODULE.validate_corpus(self.corpus)

    def test_corpus_has_unique_cases(self) -> None:
        ids = [case["id"] for case in self.cases]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertIn("goaway_reconnect", ids)

    def test_endpoint_rejects_credentials_and_queries(self) -> None:
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_endpoint("https://user:password@example.test/")
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_endpoint("https://example.test/?token=secret")

    def test_server_identity_requires_immutable_reference(self) -> None:
        manifest = {
            "schema_version": 1,
            "servers": [
                {
                    "implementation": "test",
                    "version": "1",
                    "identity": {},
                    "endpoint": "https://example.test/",
                    "capabilities": ["get"],
                }
            ],
        }
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_server_manifest(manifest, self.cases)

    def test_example_manifest_shape_is_valid(self) -> None:
        example = Path(__file__).parents[1] / "qualification/http3/servers.example.json"
        with example.open(encoding="utf-8") as handle:
            manifest = json.load(handle)
        self.assertEqual(len(MODULE.validate_server_manifest(manifest, self.cases)), 3)

    def test_output_parent_can_be_created(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "nested" / "result.json"
            output.parent.mkdir(parents=True)
            output.write_text("{}\n", encoding="utf-8")
            self.assertTrue(output.exists())

    def test_control_urls_reject_credentials_and_queries(self) -> None:
        manifest = {
            "schema_version": 1,
            "servers": [
                {
                    "implementation": "test",
                    "version": "1",
                    "identity": {"source_revision": "abc"},
                    "endpoint": "https://example.test/",
                    "capabilities": ["goaway_reconnect"],
                    "controls": {
                        "goaway_reconnect": "https://user:pass@example.test/control",
                    },
                }
            ],
        }
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_server_manifest(manifest, self.cases)

    def test_server_controlled_capability_without_control_is_not_executable(self) -> None:
        server = {
            "implementation": "test",
            "version": "1",
            "identity": {"source_revision": "abc"},
            "endpoint": "https://example.test/",
            "capabilities": ["goaway_reconnect", "get"],
            "validated_controls": {},
        }
        attempted = MODULE.attempted_cases_for_server(server)
        self.assertIn("get", attempted)
        self.assertNotIn("goaway_reconnect", attempted)

    def test_unknown_control_key_is_rejected(self) -> None:
        manifest = {
            "schema_version": 1,
            "servers": [
                {
                    "implementation": "test",
                    "version": "1",
                    "identity": {"source_revision": "abc"},
                    "endpoint": "https://example.test/",
                    "capabilities": ["get"],
                    "controls": {"not_a_real_control": "https://example.test/x"},
                }
            ],
        }
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_server_manifest(manifest, self.cases)

    def test_control_without_capability_is_rejected(self) -> None:
        manifest = {
            "schema_version": 1,
            "servers": [
                {
                    "implementation": "test",
                    "version": "1",
                    "identity": {"source_revision": "abc"},
                    "endpoint": "https://example.test/",
                    "capabilities": ["get"],
                    "controls": {
                        "goaway_reconnect": "https://example.test/goaway",
                    },
                }
            ],
        }
        with self.assertRaises(MODULE.ManifestError):
            MODULE.validate_server_manifest(manifest, self.cases)

    def test_parse_corpus_case_lines(self) -> None:
        output = "\n".join(
            [
                "running 1 test",
                "H3_CORPUS_CASE get=pass endpoint=https://example.test/",
                "H3_CORPUS_CASE goaway_reconnect=pass endpoint=https://example.test/",
            ]
        )
        observed = MODULE.parse_corpus_case_lines(output)
        self.assertEqual(observed["get"], "pass")
        self.assertEqual(observed["goaway_reconnect"], "pass")


class H3ImpairmentContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = NETEM.load_matrix()

    def test_missing_privileges_writes_all_invariants_unsupported(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "packet_loss_low.json"
            NETEM.write_runner_result(
                output,
                "packet_loss_low",
                "unsupported",
                NETEM.invariant_map("packet_loss_low", "unsupported", self.matrix),
                reason="unit test: no netem",
                packet_loss_rate=NETEM.PACKET_LOSS_LOW_RATE,
            )
            with output.open(encoding="utf-8") as handle:
                observed = json.load(handle)
            row = NETEM.scenario_row(self.matrix, "packet_loss_low")
            normalized = NETEM.validate_runner_result(observed, row)
            self.assertEqual(normalized["status"], "unsupported")
            for name in row["invariants"]:
                self.assertEqual(observed["invariants"][name], "unsupported")

    def test_pass_with_failed_invariant_is_rejected(self) -> None:
        row = NETEM.scenario_row(self.matrix, "udp_blackhole")
        observed = {
            "status": "pass",
            "scenario": "udp_blackhole",
            "invariants": {
                "bounded_connect_or_total_deadline": "pass",
                "Http3Only_does_not_fallback": "fail",
                "resources_released": "pass",
            },
        }
        normalized = NETEM.validate_runner_result(observed, row)
        self.assertEqual(normalized["status"], "fail")
        self.assertIn("error", normalized)

    def test_unknown_scenario_id_rejected_by_matrix_load(self) -> None:
        with self.assertRaises(ValueError):
            NETEM.scenario_row(self.matrix, "not_a_real_scenario")

    def test_coordinator_rejects_incomplete_invariants(self) -> None:
        row = NETEM.scenario_row(self.matrix, "latency_jitter")
        observed = {
            "status": "pass",
            "scenario": "latency_jitter",
            "invariants": {"bounded_total_deadline": "pass"},
        }
        normalized = NETEM.validate_runner_result(observed, row)
        self.assertEqual(normalized["status"], "fail")


if __name__ == "__main__":
    unittest.main()
