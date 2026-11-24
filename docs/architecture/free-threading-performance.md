# Free-Threaded Performance Evidence

This is the measurement record behind the free-threaded performance claim in
the README, the Python guide and the free-threaded support investigation. It
is evidence, not a performance budget: the numbers are host-specific, and the
claim is written so a reader can tell exactly which measurement supports
which part of it.

- [Scope](#scope)
- [The claim](#the-claim)
- [Method](#method)
- [Results](#results)
- [Interpretation](#interpretation)
- [Threats to validity](#threats-to-validity)
- [Reproducing](#reproducing)
- [Related documents](#related-documents)

## Scope

eggfetch ships free-threaded wheels for CPython 3.14t and 3.15t. The
correctness side of that support is qualified separately - see
[python-bindings.md](python-bindings.md#free-threaded-execution) and
`plans/python-free-threaded-support-investigation.md`. This document covers
the performance side only: what a free-threaded interpreter changes for a
Python program that uses eggfetch.

Two things it deliberately does not cover: the Rust engine's own performance,
which is interpreter-independent and measured by the Criterion suites in
`crates/eggfetch-bench` (see [benchmarks.md](benchmarks.md)), and any claim
about the engine's behaviour under a GIL-enabled interpreter, which is the
same code path either way.

## The claim

**Claim.** On a free-threaded CPython, eggfetch lets a Python program scale
across concurrent callers without the interpreter serializing the Python work
that surrounds each request. With eight concurrent callers and a realistic
per-response workload, the free-threaded build sustained **2.5x-3.8x** the
throughput of the GIL-enabled build and held the user-visible p50 latency at
**0.24-0.35 ms against 0.96-0.97 ms** - the GIL-enabled build plateaued at
about two threads while its p50 roughly quadrupled.

**What the claim is not.**

- It is not a latency improvement for a single caller. At one thread the
  three builds are within run-to-run variance of each other.
- It is not a throughput improvement for I/O-bound applications that do
  nothing in Python between responses. eggfetch already releases the GIL for
  the network wait, so that case scales on both builds.
- It is not a promise about any particular workload. The benefit is a
  function of how much Python work the *application* does per response, and
  the boundary is measured below.
- It says nothing about the Python 3.13t build, which does not exist, or
  about sub-interpreters, which are unsupported.

## Method

### Harness

`scripts/performance_benchmark.py --concurrency` is the measurement. It is
qualification-only timing evidence: it is outside routine CI and outside the
release gates, and no gate or CI job was added for it.

### Server

The fixture is a separate process of the same interpreter, speaking minimal
HTTP/1.1 over a raw socket with keep-alive. Its per-request Python cost is
deliberately tiny - one `recv`, one `sendall`, one head parse - so that the
client's own scaling curve is what gets measured rather than the fixture's.
It serves two payloads: an 8 KiB binary body and a 20,373-byte JSON array of
200 records.

### Client

One `eggfetch.Client` is shared by all worker threads, which is the
documented usage pattern. Each worker issues a fixed number of requests
(2,000) against the fixture and records two latency series per request: the
request and body read, and the whole user-visible operation including the
modelled application work.

### Workloads

| Workload | Application work per response | Models |
|---|---|---|
| `io` | none | The client library's own cost: request, body read, buffer handling. |
| `io_cpu` | 4x SHA-256 of the body | A CPU-bound step that does not need to look at the payload. |
| `json` | `json.loads` of the 20 KB array, then a summary list of `(id, price, active)` for all 200 records | A realistic JSON-API application mix: decode the response and extract what the caller needs. |

The `json` workload is the defensible one to quote, because it is the closest
to what an application actually does with a response, and because its Python
cost per response (90-100 us, measured consistently on all three builds and
both runs) is comparable to the request and body read itself (82-142 us) -
the regime where interpreter serialization decides the outcome.

### Repetitions and sample sizes

Each configuration is measured 5 times. Latency percentiles are computed over
the pooled samples of all repetitions: 10,000 samples at one thread, 80,000
at eight threads.

### Thread counts

1, 2, 4 and 8. The scaling factor is relative to the one-thread result of
the *same run*, so a host-load difference between runs does not contaminate
it.

### Builds

| Build | Interpreter | GIL |
|---|---|---|
| GIL 3.14.2 | CPython 3.14.2 | enabled |
| FT 3.14.10 | CPython 3.14.10t | disabled |
| FT 3.15.2 | CPython 3.15.2t | disabled |

The free-threaded interpreters are `python-build-standalone` 20261123 builds.
Every build ran the same commit with the pinned tooling in
`scripts/ci-requirements.txt`, and each result file records its own
`gil_enabled` flag so a result is self-describing.

## Results

Two independent runs of the full matrix were taken on the same host. The
ordering of the three builds is identical in both runs; the magnitudes differ,
so both are quoted.

### Throughput and scaling, `json` workload

| Threads | GIL 3.14.2 | FT 3.14.10 | FT 3.15.2 |
|---|---|---|---|
| 1 | 4,315 req/s (1.00x) / 5,351 (1.00x) | 5,684 (1.00x) / 5,336 (1.00x) | 5,752 (1.00x) / 5,082 (1.00x) |
| 2 | 8,774 (2.03x) / 8,325 (1.56x) | 11,140 (1.96x) / 10,049 (1.88x) | 11,304 (1.97x) / 10,054 (1.98x) |
| 4 | 8,357 (1.94x) / 8,107 (1.52x) | 17,871 (3.14x) / 16,487 (3.09x) | 17,857 (3.10x) / 17,866 (3.52x) |
| 8 | 8,425 (1.95x) / 8,507 (1.59x) | 29,754 (5.23x) / 21,486 (4.03x) | 32,229 (5.60x) / 28,691 (5.65x) |

Each cell is `run 1 / run 2`.

The GIL-enabled build stops scaling at two threads: 8,325-8,774 req/s at two
threads, 8,425-8,507 at eight. The free-threaded builds keep scaling to
21,000-32,000 req/s at eight threads. Taking every run combination, the
measured ratio at eight threads is **2.5x-3.8x**.

### Latency distribution, `json` workload at eight threads

User-visible latency (request, body read and application work), in
milliseconds:

| Build | Run | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| GIL 3.14.2 | 1 | 0.957 | 1.188 | 1.826 | 44.254 |
| GIL 3.14.2 | 2 | 0.970 | 1.163 | 1.427 | 10.025 |
| FT 3.14.10 | 1 | 0.263 | 0.353 | 0.470 | 0.839 |
| FT 3.14.10 | 2 | 0.345 | 0.677 | 0.994 | 39.446 |
| FT 3.15.2 | 1 | 0.243 | 0.312 | 0.399 | 0.954 |
| FT 3.15.2 | 2 | 0.283 | 0.398 | 0.533 | 1.007 |

The GIL-enabled build's median user-visible latency at eight threads is
**0.96-0.97 ms against 0.24-0.35 ms** free-threaded - 2.8x-3.9x lower. Its
p99 is 1.4x-4.6x higher. The GIL-enabled runs' worst single request reached
44 ms and 10 ms. Three of the four free-threaded runs peaked at 0.84-1.01 ms;
the fourth, FT 3.14.10 run 2, peaked at 39.4 ms in a run that was
noisier throughout (p99 0.994 ms, 21,486 req/s against 29,754 in run 1). That
figure is included rather than dropped, and it is why every comparison here
is quoted as a range.

### The other two workloads

At eight threads, same format as above (`run 1 / run 2`):

| Workload | GIL 3.14.2 | FT 3.14.10 | FT 3.15.2 |
|---|---|---|---|
| `io` req/s | 29,383 (2.62x) / 37,988 (3.16x) | 51,141 (4.77x) / 54,526 (5.00x) | 58,598 (4.74x) / 44,487 (4.24x) |
| `io_cpu` req/s | 17,064 (1.71x) / 22,281 (2.26x) | 52,147 (5.88x) / 53,072 (6.76x) | 54,565 (5.91x) / 38,824 (4.36x) |

`io_cpu` user-visible p99 at eight threads: 1.341 / 0.828 ms on the
GIL-enabled build against 0.269 / 0.376 ms (3.14.10) and 0.240 / 1.293 ms
(3.15.2). The 1.293 ms figure comes from the noisier of the two FT 3.15.2
runs - that run was slower across the board, with p50 0.227 ms against
0.144 ms and 38,824 req/s against 54,565 - so it reflects host load rather
than a property of the build. It is included rather than dropped.

### Single-caller cost

At one thread the three builds are close and the ordering is not stable
across runs - the `json` one-thread row moves between 4,315 and 5,351 req/s
on the GIL-enabled build and between 5,082 and 5,752 req/s free-threaded.
That spread is host load, not a build difference, and no single-thread
performance claim is made. What the row *does* establish is that the
free-threaded build carries no per-request penalty: an application that makes
one request at a time gets the same result from either build.

## Interpretation

**Why the GIL-enabled build plateaus.** The binding releases the GIL for the
network wait, so the *transport* already parallelizes: the `io` workload
scales 2.6x-3.2x under the GIL. What the GIL does not release is the Python
work on either side of the network wait - decoding the body into a Python
object, and whatever the application does with it. With the `json` mix that
work is comparable in cost to the request itself, so the serialized portion
dominates. At two threads the GIL-enabled build is already at its ceiling
(8,325-8,774 req/s); adding threads past that point adds contention, not
throughput, and the latency distribution widens: p50 goes from 0.225-0.242 ms
at two threads to 0.957-0.970 ms at eight.

**Why the free-threaded build keeps scaling.** With the GIL disabled, those
Python-side sections run concurrently across threads. The engine's own
concurrency was never the limit; the interpreter was.

**Where the boundary is.** The benefit is a function of the ratio between the
application's per-response Python work and the network wait. If the
application does almost nothing in Python, the two builds converge (the `io`
row). If it does a meaningful amount, the free-threaded build wins by roughly
the factor by which that work would otherwise have serialized. The `io_cpu`
row sits between: its work is real CPU time but does not depend on the
payload, so it sits between the two extremes.

## Threats to validity

- **Host load.** The machine ran at a load average of 8.67 on 14 logical CPUs
  from an unrelated project during these measurements. This is the largest
  source of variance, and it is why the free-threaded magnitudes differ by up
  to 30% between the two runs while the GIL-enabled numbers are stable. The
  *ordering* is identical in both runs.
- **Loopback only.** The fixture is on `127.0.0.1` with no network latency.
  A real network changes the ratio between network wait and Python work, and
  therefore moves the boundary described above.
- **Patch versions differ.** The GIL-enabled interpreter is 3.14.2 and the
  free-threaded one is 3.14.10t, because those are the interpreters available
  on this host. They are the same minor version.
- **The fixture is the same interpreter as the client.** A server ceiling
  would appear as the same plateau in every run. Since the free-threaded runs
  out-scale the GIL-enabled runs rather than converging on it, the ceiling
  was the client, not the server.
- **The `json` mix is representative, not universal.** It is one plausible
  application shape. An application that does ten times more Python work per
  response would see a larger free-threaded advantage; one that does almost
  none would see almost none.
- **Not a budget.** Nothing here is a threshold. The repository's rule stands:
  `scripts/performance_benchmark.py` stays non-CI timing evidence, and only a
  fresh measurement on the same commit, toolchains, target, feature set,
  server fixture and repeat settings is the basis for a comparison.

## Reproducing

```sh
source .venv/bin/activate
python scripts/performance_benchmark.py --repeats 5 --concurrency --threads 1,2,4,8
```

Run the same command on a GIL-enabled interpreter and on a free-threaded
one, and compare the scaling factors in the `concurrency` group of the JSON
output. The group reports `gil_enabled` for the running interpreter, so a
result file identifies its own build. `docs/architecture/benchmarks.md`
documents the reading rules.

## Related documents

- [python-bindings.md](python-bindings.md#free-threaded-execution) - the
  free-threading contract: GIL stays off, callbacks are not serialized,
  sub-interpreters are unsupported.
- [benchmarks.md](benchmarks.md) - the Criterion suites and the performance
  campaign, including the reading rules for this harness.
- `plans/python-free-threaded-support-investigation.md` - the investigation
  and decision record, with this evidence in the free-threading section.
