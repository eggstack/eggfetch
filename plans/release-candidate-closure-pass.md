# Release Candidate Closure Pass

## Goal

Prepare eggfetch for its first public release candidate (v0.1.0-rc1) by freezing public APIs, validating every supported interface, completing release automation, and eliminating remaining release blockers. This pass should avoid major feature work except for critical correctness defects.

## Track A — API freeze

- Freeze Rust public API.
- Freeze Python API and exception hierarchy.
- Freeze CLI flags, JSON output schema, and exit codes.
- Freeze C ABI handles and ownership semantics.
- Produce an explicit compatibility document listing stability guarantees and experimental surfaces (HTTP/3, Node bindings).

Acceptance:
- No planned breaking API changes remain before 0.1.0.

## Track B — Release automation

Implement and validate:
- crates.io dry-run and publish workflow.
- TestPyPI then PyPI publication.
- GitHub Release automation.
- Artifact checksums.
- SBOM generation.
- Provenance/signing where supported.
- Automated release notes from CHANGELOG.
- Tag-driven release pipeline.

Perform at least one end-to-end release rehearsal on a pre-release tag.

## Track C — Platform validation

Verify:
- Linux x86_64 and aarch64.
- macOS Intel and Apple Silicon.
- Windows x64.
- Python 3.10–3.13 wheels.
- Rust MSRV policy.
- Feature combinations (minimal/default/full/http3).

Document any unsupported combinations explicitly.

## Track D — Differential compatibility

Compare behavior against requests/httpx using identical integration tests:
- redirects
- cookies
- auth
- multipart
- decompression
- retry
- timeout
- proxy
- streaming

Document intentional semantic differences.

## Track E — Performance regression gate

Establish baselines for:
- latency
- throughput
- allocations
- memory
- connection reuse
- streaming
- multipart
- proxy

Create regression thresholds enforced in CI where practical.

## Track F — Security closure

Review:
- redaction
- TLS configuration
- unsafe blocks
- dependency audit
- fuzz coverage
- denial-of-service considerations
- decompression limits
- parser limits

Resolve all medium/high findings before RC.

## Track G — Documentation polish

Validate:
- every example builds or executes
- CLI help matches documentation
- migration guides remain accurate
- feature matrix reflects implementation
- release notes are complete

## Track H — Repository hygiene

- Prune oversized fuzz corpora to curated seeds.
- Remove dead code and stale feature flags.
- Verify license notices.
- Review dependency graph.

## Exit criteria

The release candidate is ready when:
- all CI jobs pass
- no known correctness issues remain
- API surface is frozen
- release automation succeeds in rehearsal
- documentation is complete
- security review is complete
- benchmark baselines are recorded
- outstanding issues are limited to deferred post-0.1 enhancements.