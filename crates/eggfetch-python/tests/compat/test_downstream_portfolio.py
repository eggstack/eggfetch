"""Meta-tests that validate the downstream compatibility manifest structure.

Ensures all required fields are present, versions are pinned, no duplicate
package names exist, and the manifest conforms to schema-version 1.
"""

from pathlib import Path

import pytest

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

MANIFEST_PATH = Path(__file__).resolve().parents[4] / "compat" / "downstream" / "manifest.toml"

REQUIRED_PACKAGE_FIELDS = {
    "name",
    "version",
    "license",
    "category",
    "rationale",
    "usage",
    "test-subset",
    "expected-network-isolation",
    "optional-dependencies",
    "known-incompatibilities",
    "update-owner",
    "review-cadence",
    "test-command",
    "min-tests",
}

REQUIRED_V2_PACKAGE_FIELDS = {
    "source-type",
    "source-locator",
    "source-hash",
    "python-versions",
    "public-httpx-api",
    "install-command",
    "test-working-dir",
    "test-result-format",
    "min-collected",
    "max-skipped",
    "max-xfailed",
    "timeout",
    "network-policy",
    "category-ids",
    "release-blocking",
}

REQUIRED_PORTFOLIO_FIELDS = {
    "schema-version",
    "status",
    "created",
    "reference-profile",
    "description",
}

VALID_CATEGORIES = {
    "contract-tests",
    "mock-transport-user",
    "framework-test-client",
    "framework-asgi-transport",
    "sdk-async-client",
    "sdk-sync-client",
    "streaming-upload-download",
    "custom-transport-subclass",
    "async-testing-support",
    "custom-auth-flow",
    "event-hook-instrumentation",
    "heavy-config-user",
}

VALID_USAGE = {"public", "private", "required", "informational"}


@pytest.fixture(scope="module")
def manifest():
    if not MANIFEST_PATH.exists():
        pytest.skip(f"Manifest not found: {MANIFEST_PATH}")
    with open(MANIFEST_PATH, "rb") as f:
        return tomllib.load(f)


class TestPortfolioMetadata:
    def test_schema_version_is_valid(self, manifest):
        portfolio = manifest.get("portfolio", {})
        assert portfolio.get("schema-version") in ("1", "2")

    def test_status_matches_schema_version(self, manifest):
        portfolio = manifest.get("portfolio", {})
        sv = portfolio.get("schema-version", "1")
        if sv == "2":
            assert portfolio.get("status") == "phase-6"
        else:
            assert portfolio.get("status") == "phase-5"

    def test_all_portfolio_fields_present(self, manifest):
        portfolio = manifest.get("portfolio", {})
        missing = REQUIRED_PORTFOLIO_FIELDS - set(portfolio.keys())
        assert not missing, f"Missing portfolio fields: {missing}"

    def test_reference_profile_exists(self, manifest):
        portfolio = manifest.get("portfolio", {})
        ref = portfolio.get("reference-profile", "")
        ref_path = (MANIFEST_PATH.parent / ref).resolve()
        assert ref_path.exists(), f"Reference profile not found: {ref_path}"

    def test_v2_has_stage_c_categories(self, manifest):
        portfolio = manifest.get("portfolio", {})
        if portfolio.get("schema-version") == "2":
            cats = portfolio.get("stage-c-categories", [])
            assert len(cats) >= 8, f"Schema v2 should have >=8 stage-c-categories, got {len(cats)}"


class TestPackageEntries:
    def test_packages_list_exists(self, manifest):
        assert "package" in manifest, "No [[package]] entries found"
        assert isinstance(manifest["package"], list)
        assert len(manifest["package"]) > 0

    def test_no_duplicate_package_names(self, manifest):
        packages = manifest.get("package", [])
        names = [p["name"] for p in packages]
        duplicates = [n for n in names if names.count(n) > 1]
        assert not duplicates, f"Duplicate package names: {set(duplicates)}"

    def test_all_required_fields_present(self, manifest):
        packages = manifest.get("package", [])
        schema_version = manifest.get("portfolio", {}).get("schema-version", "1")
        base_fields = REQUIRED_PACKAGE_FIELDS
        if schema_version == "2":
            base_fields = base_fields | REQUIRED_V2_PACKAGE_FIELDS
        errors = []
        for pkg in packages:
            missing = base_fields - set(pkg.keys())
            if missing:
                errors.append(f"{pkg.get('name', '?')}: missing {missing}")
        assert not errors, "\n".join(errors)

    def test_versions_are_pinned(self, manifest):
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            version = pkg.get("version", "")
            if not version or version == "latest":
                errors.append(f"{pkg['name']}: version not pinned")
            elif not any(c.isdigit() for c in version):
                errors.append(f"{pkg['name']}: version '{version}' has no digits")
        assert not errors, "\n".join(errors)

    def test_categories_are_valid(self, manifest):
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            cat = pkg.get("category", "")
            if cat not in VALID_CATEGORIES:
                errors.append(f"{pkg['name']}: unknown category '{cat}'")
        assert not errors, "\n".join(errors)

    def test_usage_is_valid(self, manifest):
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            usage = pkg.get("usage", "")
            if usage not in VALID_USAGE:
                errors.append(f"{pkg['name']}: invalid usage '{usage}'")
        assert not errors, "\n".join(errors)

    def test_optional_dependencies_are_lists(self, manifest):
        packages = manifest.get("package", [])
        for pkg in packages:
            deps = pkg.get("optional-dependencies")
            assert isinstance(deps, list), (
                f"{pkg['name']}: optional-dependencies must be a list"
            )

    def test_known_incompatibilities_are_lists(self, manifest):
        packages = manifest.get("package", [])
        for pkg in packages:
            incompat = pkg.get("known-incompatibilities")
            assert isinstance(incompat, list), (
                f"{pkg['name']}: known-incompatibilities must be a list"
            )

    def test_expected_network_isolation_is_bool(self, manifest):
        packages = manifest.get("package", [])
        for pkg in packages:
            val = pkg.get("expected-network-isolation")
            assert isinstance(val, bool), (
                f"{pkg['name']}: expected-network-isolation must be bool, got {type(val)}"
            )

    def test_at_least_one_package_per_category(self, manifest):
        packages = manifest.get("package", [])
        covered = {pkg["category"] for pkg in packages}
        missing = VALID_CATEGORIES - covered
        assert not missing, f"Categories with no representative package: {missing}"

    def test_test_commands_are_strings(self, manifest):
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            cmd = pkg.get("test-command")
            if cmd is None:
                errors.append(f"{pkg['name']}: missing test-command")
            elif not isinstance(cmd, str):
                errors.append(f"{pkg['name']}: test-command must be a string")
            elif not cmd.strip():
                errors.append(f"{pkg['name']}: test-command is empty")
        assert not errors, "\n".join(errors)

    def test_min_tests_are_non_negative_integers(self, manifest):
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            min_t = pkg.get("min-tests")
            if min_t is None:
                errors.append(f"{pkg['name']}: missing min-tests")
            elif not isinstance(min_t, int) or min_t < 0:
                errors.append(f"{pkg['name']}: min-tests must be a non-negative integer, got {min_t}")
        assert not errors, "\n".join(errors)

    def test_required_packages_have_min_tests(self, manifest):
        """Required packages should have min-tests > 0.

        Excludes packages that are pytest plugins or SDK wrappers without
        installed test suites (e.g. pytest-httpx, anthropic, groq).
        """
        packages = manifest.get("package", [])
        # Packages exempted from min-tests requirement (informational or
        # SDK wrappers without installed test suites)
        EXEMPT_PACKAGES = {"anthropic", "groq", "anyio", "pydantic", "httpx"}
        errors = []
        for pkg in packages:
            if pkg.get("usage") != "required":
                continue
            if pkg["name"] in EXEMPT_PACKAGES:
                continue
            cat = pkg.get("category", "")
            min_t = pkg.get("min-tests", 0)
            # Categories that should have runnable tests
            testable = {
                "mock-transport-user", "framework-test-client",
                "framework-asgi-transport", "streaming-upload-download",
                "custom-transport-subclass", "custom-auth-flow",
                "event-hook-instrumentation",
            }
            if cat in testable and min_t == 0:
                errors.append(
                    f"{pkg['name']}: required package in category '{cat}' "
                    f"should have min-tests > 0, got {min_t}"
                )
        assert not errors, "\n".join(errors)

    def test_informational_entries_are_not_required(self, manifest):
        """Informational entries should not be classified as required."""
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            if pkg.get("usage") == "informational" and pkg.get("min-tests", 0) > 0:
                errors.append(
                    f"{pkg['name']}: informational entry should have min-tests=0, "
                    f"got {pkg.get('min-tests')}"
                )
        assert not errors, "\n".join(errors)


class TestSchemaV2Fields:
    def test_v2_required_entries_have_source_type(self, manifest):
        schema_version = manifest.get("portfolio", {}).get("schema-version", "1")
        if schema_version != "2":
            pytest.skip("Not schema v2")
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            if pkg.get("usage") == "required":
                if not pkg.get("source-type"):
                    errors.append(f"{pkg['name']}: missing source-type")
                if not pkg.get("source-locator"):
                    errors.append(f"{pkg['name']}: missing source-locator")
                if not isinstance(pkg.get("category-ids"), list):
                    errors.append(f"{pkg['name']}: missing category-ids")
        assert not errors, "\n".join(errors)

    def test_v2_required_entries_have_test_command(self, manifest):
        schema_version = manifest.get("portfolio", {}).get("schema-version", "1")
        if schema_version != "2":
            pytest.skip("Not schema v2")
        packages = manifest.get("package", [])
        errors = []
        for pkg in packages:
            if pkg.get("usage") == "required":
                cmd = pkg.get("test-command", "")
                if not cmd:
                    errors.append(f"{pkg['name']}: required package missing test-command")
        assert not errors, "\n".join(errors)

    def test_v2_behavioral_fixtures_dir_exists(self, manifest):
        schema_version = manifest.get("portfolio", {}).get("schema-version", "1")
        if schema_version != "2":
            pytest.skip("Not schema v2")
        fixtures_dir = MANIFEST_PATH.parent / "behavioral_fixtures"
        assert fixtures_dir.exists(), f"Schema v2 requires behavioral_fixtures: {fixtures_dir}"
        py_files = list(fixtures_dir.glob("test_*.py"))
        assert len(py_files) >= 5, f"Expected >=5 fixture files, found {len(py_files)}"
