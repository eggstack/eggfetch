# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in eggfetch, please report it privately. Do not open a public GitHub issue.

Email: **security@eggstack.dev**

Note: this email is a placeholder until the project has a permanent hosting location with a confirmed security contact. Deliverability is not yet guaranteed. If you do not receive a response within a reasonable window, please open a non-security issue on GitHub referencing the report.

## Supported Versions

At the skeleton stage of this project, only the latest commit on the `main` branch is considered supported. There are no tagged releases yet. No backports or point releases are planned until a formal release is cut.

## Networking and TLS

eggfetch is an HTTP client engine. The core crate performs TLS negotiation, DNS resolution, and parsing of untrusted network input. A vulnerability in this code path can compromise confidentiality, integrity, or availability of connections made through the library.

Audit posture for networking and TLS code is treated as a serious concern. The project prefers Rustls over native TLS (OpenSSL, Secure Transport, SChannel) for portability and because Rustls has a smaller, memory-safe attack surface. All TLS, DNS, and body-parsing code should be reviewed carefully before release.

## Dependency Audit Posture

eggfetch intends to use cargo-deny and cargo-audit for dependency auditing. These tools are planned but not yet wired into CI. Hardening the dependency pipeline is part of the project's pre-release work.

Every dependency in the workspace must have an explicit reason documented in code or review. The project follows a conservative dependency policy: small transitive trees, no unnecessary proc-macro crates, and features kept optional unless essential.

## MSRV and Supply Chain

The minimum supported Rust version is 1.80. This is a conservative pin that avoids pulling in unstable compiler features. The toolchain is pinned via `rust-toolchain.toml` on the stable channel.

The project does not vendor dependencies or pin lockfiles at this stage. Dependency verification will be added as the project matures.
