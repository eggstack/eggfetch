"""Meta-tests that validate the downstream compatibility manifest structure.

Ensures all required fields are present, versions are pinned, no duplicate
package names exist, and the manifest conforms to schema-version 1.
"""

import tomllib
from pathlib import Path

import pytest

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

VALID_USAGE = {"public", "private"}


@pytest.fixture(scope="module")
def manifest():
    if not MANIFEST_PATH.exists():
        pytest.skip(f"Manifest not found: {MANIFEST_PATH}")
    with open(MANIFEST_PATH, "rb") as f:
        return tomllib.load(f)


class TestPortfolioMetadata:
    def test_schema_version_is_1(self, manifest):
        portfolio = manifest.get("portfolio", {})
        assert portfolio.get("schema-version") == "1"

    def test_status_is_phase5(self, manifest):
        portfolio = manifest.get("portfolio", {})
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
        errors = []
        for pkg in packages:
            missing = REQUIRED_PACKAGE_FIELDS - set(pkg.keys())
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
        """Public-usage packages with testable categories should have min-tests > 0.

        Excludes packages that are pytest plugins or libraries without
        installed test suites (e.g. pytest-httpx, anthropic, groq).
        """
        packages = manifest.get("package", [])
        # Packages that are test frameworks or SDK wrappers without shipped tests
        EXEMPT_PACKAGES = {"pytest-httpx", "anthropic", "groq", "anyio", "pydantic", "httpx"}
        errors = []
        for pkg in packages:
            if pkg.get("usage") != "public":
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
                    f"{pkg['name']}: public package in category '{cat}' "
                    f"should have min-tests > 0, got {min_t}"
                )
        assert not errors, "\n".join(errors)
