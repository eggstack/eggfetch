from __future__ import annotations

from dataclasses import dataclass

from eggfetch import __version__ as _IMPL_VERSION


@dataclass(frozen=True, slots=True)
class CompatibilityInfo:
    provider: str
    implementation_version: str
    emulated_version: str
    compatibility_stage: str
    backend: str
    supported_python_versions: tuple[str, ...]
    profile_schema_version: str


COMPATIBILITY_INFO = CompatibilityInfo(
    provider="eggfetch",
    implementation_version=_IMPL_VERSION,
    emulated_version="0.28.1",
    compatibility_stage="stage-c-candidate",
    backend="rust-tokio",
    supported_python_versions=("3.10", "3.11", "3.12", "3.13"),
    profile_schema_version="1",
)


def get_compatibility_info() -> CompatibilityInfo:
    return COMPATIBILITY_INFO


def diagnostics_summary() -> str:
    info = COMPATIBILITY_INFO
    py_versions = ", ".join(info.supported_python_versions)
    return (
        f"eggfetch HTTPX compatibility layer\n"
        f"  Provider:          {info.provider}\n"
        f"  Implementation:    v{info.implementation_version}\n"
        f"  Emulated version:  HTTPX {info.emulated_version}\n"
        f"  Stage:             {info.compatibility_stage}\n"
        f"  Backend:           {info.backend}\n"
        f"  Supported Python:  {py_versions}\n"
        f"  Profile schema:    v{info.profile_schema_version}"
    )
