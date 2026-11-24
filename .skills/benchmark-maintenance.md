# Benchmark Maintenance Skill

Use this skill when extending, re-running or publishing eggfetch's performance
benchmarks, and when making a performance claim in the README or the docs.

## Workflow

1. Read `docs/architecture/benchmarks.md` for the Criterion suites, the
   BenchServer harness and the performance campaign.
2. Read `docs/architecture/free-threading-performance.md` for the
   GIL-versus-free-threaded evidence and the reading rules it established.
3. Pick the instrument that can answer the question (see the table below).
4. Extend the harness first if it cannot express the measurement. Do not
   measure something the existing instrument cannot represent.
5. Run the full matrix on every relevant build, at least twice, sequentially.
6. Verify every number programmatically against the raw output before it is
   published.
7. Write the detailed report, then the user-facing snapshot, then the
   qualification-ledger entry if the harness changed.

## Choosing an instrument

| Question | Instrument |
|---|---|
| Engine throughput, allocation and streaming shape | `crates/eggfetch-bench` (Criterion: `microbench`, `e2e`, `resources`) |
| Absolute peak RSS growth | `crates/eggfetch-bench` `resource_monitor` binary |
| eggfetch against direct Hyper on the native path | `crates/eggfetch-bench` `native_streaming_tail` binary |
| Python-level cost of the public API | `scripts/performance_benchmark.py` |
| eggfetch against HTTPX latency ceilings | `compat/httpx/0.28.1/performance-budgets.toml` |
| Whether the interpreter's GIL state changes anything | `scripts/performance_benchmark.py --concurrency` |

The Criterion suites measure `eggfetch-core` in Rust, so they cannot see a
Python-level question. A GIL-versus-free-threaded question is a Python-level
question and needs the `--concurrency` group.

## Running the Python harness

```sh
source .venv/bin/activate
python scripts/performance_benchmark.py --repeats 5 --concurrency --threads 1,2,4,8
```

The group reports `gil_enabled` for the running interpreter, so each result
file identifies its own build. `--workloads` selects from `io`, `io_cpu` and
`json`; an unknown name is rejected rather than ignored.

For a free-threaded comparison, run the same command with the same repeat and
thread settings on a GIL-enabled interpreter and on a free-threaded one, and
keep the raw JSON for both. Free-threaded interpreters are not packaged with
the system Python. The qualified set came from `python-build-standalone`
releases, unpacked outside the repository:

```sh
gh release download 20261123 --repo astral-sh/python-build-standalone \
  --pattern 'cpython-3.15.2+*-apple-darwin-freethreaded-install_only.tar.gz'
uv venv --python <unpacked>/bin/python3.15t .venv-ft3152
uv pip install --python .venv-ft3152/bin/python -r scripts/ci-requirements.txt
```

`uv` venvs have no `pip`. Install it explicitly when a step needs it.

## Rules that make a claim defensible

- **Compare the scaling factor within one run, not absolute throughput
  between runs.** Absolute throughput is host- and load-dependent. The
  scaling factor is relative to the first thread count of the same run, so a
  load difference between runs does not contaminate it.
- **Take at least two independent runs and quote ranges.** The ordering of
  the builds is the durable finding; a single magnitude is not. Say which
  build was faster, and by how much across the range.
- **Verify every published number against the raw output.** Write a scratch
  script that loads the result JSON and prints the exact cell you are about to
  type. Transcription errors are easy to make and hard to see. Two figures in
  the first draft of the free-threading report were wrong and were caught this
  way, and one caveat overstated what a noisy run showed.
- **Record the host load.** `uptime` before and after. Host load from
  unrelated work is usually the largest source of variance, and it explains
  most of the spread between runs.
- **Rule out a server ceiling.** A ceiling would appear as the same plateau
  in every run. A build that out-scales another build is evidence that the
  ceiling was the client, not the fixture.
- **Keep the fixture cheap.** The server's per-request Python cost must not
  be the thing being measured. The concurrency fixture runs in a separate
  process with a minimal raw-socket handler for this reason.
- **Never turn timing into a gate.** `scripts/performance_benchmark.py` stays
  outside routine CI and outside the release gates. Do not add a CI job, a
  matrix row or a threshold to make a measurement enforceable. Timing is
  evidence, and it is compared only when the commit, toolchains, target,
  feature set, fixture and repeat settings are identical.
- **Do not duplicate numbers across documents.** The detailed report holds the
  numbers and the reasoning; the README holds a snapshot and a link; the
  investigation record points at the report. One source of truth per figure.
- **The README section is a snapshot, not a report.** A short lead, the
  measurement table, one sentence on when the effect does and does not apply,
  and a link. No em dashes, no prose walls.
- **State what the claim is not.** Name the case that sees no benefit: a
  single caller, or an application that does no Python work between
  responses. A claim that only lists the good case is not defensible.
- **Changing the harness is an executable-input change.** It invalidates the
  exact-SHA binding under `.skills/verification-qualification.md`, so record
  it in `plans/httpx-parity-correction-status.md`.

## Where the output goes

| Output | Location |
|---|---|
| Detailed method, per-build numbers, interpretation, threats to validity | `docs/architecture/free-threading-performance.md` |
| Harness reference and reading rules | `docs/architecture/benchmarks.md` |
| User-facing snapshot | the README section, linking to the report |
| Decision and qualification context | `plans/python-free-threaded-support-investigation.md` |
| Executable-input change | `plans/httpx-parity-correction-status.md` |

New deep dives are indexed in `docs/architecture/overview.md` (Deep-Dive
Index), and cross-referenced from the README's Documentation section.

## Architecture References

- Benchmarks: `docs/architecture/benchmarks.md`
- Free-threading performance evidence: `docs/architecture/free-threading-performance.md`
- Verification policy and the exact-SHA rule: `.skills/verification-qualification.md`
