# Performance and Footprint Roadmap

Status: closed; campaigns complete, fixtures repaired

Long-term references:

- `plans/000-long-term-specification.md` §5, §8
- `plans/001-terminology-and-domain-model.md` §9
- `plans/002-long-term-roadmap.md` Phase 0

Related ADRs:

- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

## 1. Purpose and ownership boundary

Owns measurement-driven efficiency: reproducible benchmarks, ownership/
allocation fast paths, streaming backpressure and copy reduction, cookie/pool
memory behavior, linked-footprint attribution, embedded-footprint evidence,
and bench-fixture determinism.

Consumes: engine internals. Must not own: public API changes (campaigns are
API-preserving by charter), custom pools, unbounded queues, routine CI timing
gates, production proxy behavior (bench fixtures are harness code).

## 2. Work classification

### Invariants

- Baseline first; no optimization without comparable before/after evidence.
- Public Rust/Python/C/CLI and HTTPX semantics frozen during campaigns.
- `eggfetch-bench` fixtures deterministic; unsupported framing reported
  explicitly, never left to read timeouts.
- Embedded verdicts reported as measured (including "not a footprint win").

### Capabilities

- None directly; this workstream serves engine efficiency, not user features.

### Infrastructure

- Benchmark guardrails, RSS regression monitor, footprint attribution.

### Polish

- Fixture repairs, classification corrections, benchmark hygiene.

## 3. Non-goals

- New dependencies, custom pools, unsafe buffer sharing, `ResponseBody`
  shape changes, H3 graduation, Node maturation, downstream-specific tuning.

## 4. Current state

First and second performance campaigns closed on frozen SHAs with
requalification; footprint program closed with the lean `standard-http1`
profile as the only measured improvement (+32 KiB vs aligned reqwest on
x86_64/thin-LTO; full compat +590 KiB); streaming-tail investigation closed
as not-reproduced/residual-unlocalized with fixture repaired. No production
optimization resulted from the tail work by design.

## 5. Target architecture

Efficiency work continues only as bounded, evidence-gated milestones. Cookie
watermark-style shortcuts require isolated large-jar evidence showing a
material win with unchanged semantics.

## 6. Dependency graph

```text
M001 benchmark baselines + guardrails (hard predecessor)
    |
    +--> M002 core ownership/allocation fast paths
    |
    +--> M003 Python streaming/buffered optimization
    |
    +--> M004 footprint boundary + attribution (soft; independent)
    |
    `--> M005 requalification + closure (hard after M002-M004)
```

Closed twice (first campaign, second pass) plus the footprint program; the
tail investigation was evidence-only with no M005 production change.

## 7. Milestones

### Milestone 1 — First performance campaign (API-preserving)

Class: polish + infrastructure. Status: closed.

Legacy: `plans/performance-optimization-no-api-regression-program.md`,
`plans/performance-benchmark-baseline-and-guardrails.md`,
`plans/core-hot-path-allocation-and-ownership-optimization.md`,
`plans/python-streaming-backpressure-and-copy-optimization.md`,
`plans/cookie-and-buffered-response-memory-optimization.md`,
`plans/post-performance-requalification-and-closure.md`,
`plans/performance-exact-sha-evidence-corrective-pass.md`.

Exit conditions: baselines frozen; ownership fast paths landed;
requalification + renewed binding. Met.

### Milestone 2 — Second-pass ownership optimization

Class: polish + infrastructure. Status: closed.

Legacy: `plans/second-pass-performance-ownership-optimization-program.md`,
`plans/second-pass-performance-benchmark-and-guardrails.md`,
`plans/high-level-h1-h2-request-ownership-fast-path.md`,
`plans/native-and-proxy-ownership-cleanup.md`,
`plans/python-buffered-response-and-adapter-ownership-optimization.md`,
`plans/benchmark-gated-cookie-and-body-buffer-tuning.md`,
`plans/second-pass-performance-requalification-and-closure.md`,
`plans/second-pass-performance-closure-corrective-pass.md`.

Exit conditions: header/move-based fast paths; lazy iterators; no-`Vec`
`aread()`; cookie/body tuning evidence-gated; requalification. Met.

### Milestone 3 — Footprint reduction and embedded evidence

Class: infrastructure + polish. Status: closed.

Legacy: `plans/linked-binary-footprint-reduction-program.md`,
`plans/linked-byte-baseline-and-attribution.md`,
`plans/conditional-tls-and-residual-dependency-footprint-tuning.md`,
`plans/embedded-consumer-footprint-qualification.md`,
`plans/post-footprint-reduction-requalification-and-closure.md`.

Exit conditions: lean profile measured; full-compat cost documented
honestly; no micro-feature proliferation. Met.

### Milestone 4 — Streaming-tail investigation and fixture corrective

Class: polish (evidence-only). Status: closed.

Legacy: `plans/native-concurrent-streaming-tail-investigation.md` (+ JSONL
evidence), `plans/native-streaming-classification-and-proxy-benchmark-corrective.md`.

Exit conditions: truthful terminal classification; deterministic fixture +
focused coverage; no production change without protocol-valid reproduction.
Met — result: not reproduced consistently / residual unlocalized.

## 8. Cross-cutting requirements

Perf-sensitive paths preserve chunking, cancellation, backpressure,
decoding, and public surface. `scripts/performance_benchmark.py` stays
non-CI timing evidence only.

## 9. Verification strategy

Comparable benchmark evidence before/after; focused non-Criterion tests for
fixture claims; Tier 1 + Tier 2 + exact-SHA renewal for executable changes;
manual `qualification/embedded` and H3-adjacent evidence never gates.

## 10. Risks and decision points

Risk: benchmark-driven scope creep into production proxies. Control: bench
fixtures are harness code; product-path proof required before any core
change (the rule the M004 corrective enforced).

## 11. Completion definition

Closed: campaigns requalified, footprint documented, tail classified, fixture
repaired. New optimization needs its own milestone plan with fresh baselines.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 first campaign | closed | legacy program §7 | legacy closures | — |
| M002 second pass | closed | legacy program §7 | legacy closures | — |
| M003 footprint | closed | legacy program §7 | legacy closure | — |
| M004 tail + fixture | closed | legacy plans §7 | legacy records | — |
