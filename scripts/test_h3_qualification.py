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


if __name__ == "__main__":
    unittest.main()
