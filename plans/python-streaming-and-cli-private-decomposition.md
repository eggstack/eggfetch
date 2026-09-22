# Python Streaming and CLI Private Decomposition

Planning baseline: `c53eebc47569279b61c0611d4523c5132bdbcaeb`
Parent program: `plans/api-preserving-private-architecture-containment-program.md`
Prerequisite: `plans/core-client-proxy-private-decomposition-second-pass.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Reduce private implementation concentration in the Python streaming adapter
and CLI without changing any Python API/signature/typing/lifecycle behavior or
any CLI command syntax/default/output/exit-code behavior.

This plan is adapter decomposition only. It must not introduce a second HTTP
implementation, alter `eggfetch-core` semantics, or use code generation to
hide the public contracts that the repository now explicitly snapshots.

## Part A — preserve Python public signatures literally

Do not change or macro-generate the reviewed public PyO3 signatures for:

- `Client`;
- `AsyncClient`;
- top-level `request/get/post/put/patch/delete/head/options`;
- `StreamingResponse`;
- sync streaming iterators;
- async streaming iterators;
- network-stream wrappers;
- response properties/methods.

The visible sync/async duplication in the public method layer is intentional
and auditable. Semantic normalization remains centralized in
`request_preparation.rs`.

Do not create a second request-preparation or response-construction path.

## Part B — split Python streaming private state/lifecycle ownership

At baseline, `crates/eggfetch-python/src/streaming.rs` combines several
distinct private responsibilities in one large module.

Create private child modules only where responsibility boundaries are clear.
A reasonable target layout is:

```text
src/streaming.rs                  # public PyO3 class declarations / thin wiring
src/streaming/state.rs            # response consumption state + body ownership
src/streaming/sync_bridge.rs      # bounded sync backpressure bridge
src/streaming/decoding.rs         # incremental text decoding + line splitting
src/streaming/sync_iterators.rs   # private implementation for sync iterators
src/streaming/async_iterators.rs  # private implementation for async iterators
```

Exact names are not normative. Keep fewer modules if that yields clearer
ownership.

### State/lifecycle invariants

The following behavior must remain identical:

- state transitions among streaming/buffered/consumed/closed;
- single-consumption semantics;
- `StreamConsumed`, `StreamClosed`, and `ResponseNotRead` timing;
- buffered `read()/text()/aread()` cache behavior;
- pool/response-body ownership and release timing;
- `RuntimeLease` lifetime for sync streaming;
- close/drop behavior;
- sync and async context-manager behavior;
- response `network_stream` exposure and ownership;
- URL redaction in `__repr__`;
- callback/error propagation.

Do not change state representation merely to make decomposition convenient.

## Part C — preserve sync streaming backpressure

The current sync bridge is deliberately bounded so a slow Python consumer does
not create an unbounded queue.

If moved into a child module:

- preserve the exact queue capacity;
- preserve producer wakeup/consumer blocking semantics;
- preserve close/cancellation behavior;
- do not block Tokio workers on Python-side backpressure;
- do not add an unbounded channel;
- do not add an additional full-body copy;
- retain current `Bytes` ownership/cursor behavior from the completed
  performance campaign.

Add focused tests only where moves expose previously implicit lifecycle
assumptions.

## Part D — preserve text decoding and line semantics

Move incremental decoder/line splitting helpers only if their behavior remains
exact.

Guard:

- encoding selection;
- incremental boundary handling across arbitrary chunk splits;
- final decoder flush;
- CR/LF/CRLF line behavior;
- trailing unterminated line behavior;
- empty chunk behavior;
- bounded initial decode capacity;
- raw-vs-decoded stream distinction.

No "cleanup" may alter HTTPX-compatible text/line iteration semantics.

## Part E — split sync/async iterator implementation without changing classes

The existing Python class names and registration remain exactly unchanged:

- `StreamingBytesIterator`;
- `StreamingTextIterator`;
- `StreamingLinesIterator`;
- `StreamingRawBytesIterator`;
- `AsyncStreamingBytesIterator`;
- `AsyncStreamingTextIterator`;
- `AsyncStreamingLinesIterator`;
- `AsyncStreamingRawBytesIterator`.

Their PyO3 classes may remain declared in the parent module or be implemented
through private Rust submodules, but:

- Python module/name identity must not change;
- constructor visibility remains private;
- iterator/async-iterator protocol remains exact;
- cancellation/drop/close behavior remains exact;
- async `__anext__` continues to integrate through
  `pyo3-async-runtimes`;
- no new public Python helper/type is introduced.

## Part F — decompose CLI presentation/adaptation responsibilities

At baseline, `crates/eggfetch-cli/src/main.rs` owns clap schema, parsing,
request assembly, output/file handling, error mapping, streaming and tests.

Split only private adapter responsibilities. A reasonable target is:

```text
src/main.rs            # entry point + high-level orchestration
src/args.rs            # clap structs/enums and CLI-only validation
src/input.rs           # header/query/form/file parsing + body input
src/output.rs          # human/machine formatting + stream/file output
src/files.rs           # filename derivation/sanitization/open policy
src/errors.rs          # core/anyhow -> stable CLI exit codes
```

Exact names are not normative.

### CLI contract invariants

Preserve exactly:

- command/option names, aliases and value syntax;
- clap defaults and environment behavior;
- implicit method selection;
- body/form/file precedence and validation;
- default headers/user agent behavior;
- timeout/redirect/retry/proxy/TLS/protocol mapping into core;
- machine-readable output schema;
- human-readable stdout/stderr routing;
- verbose output;
- binary-terminal warning behavior;
- secret-header redaction;
- `--check-status` behavior;
- exit codes 0/2/3/4/5/6/7 and their classification;
- output filename derivation, path stripping and reserved-name rejection;
- no-clobber semantics;
- streaming writes/flush behavior and destructor-safe error paths.

Do not add subcommands or flags.

## Part G — avoid cross-adapter utility extraction without evidence

Some tiny utilities may look reusable across CLI/Python/FFI, but this plan must
not create a shared adapter-utilities crate or move presentation semantics
into `eggfetch-core`.

Examples that should remain adapter-local unless exact semantics already share
an owner:

- CLI Base64 formatting for machine output;
- Python incremental text decoding;
- Python exception mapping;
- CLI filename sanitization;
- CLI error-to-exit-code mapping.

Duplication across language/UI boundaries is preferable to inappropriate
shared policy.

## Tests and validation

Before implementation, capture:

- native Python API manifest result;
- Python typing-surface result;
- focused streaming tests;
- CLI test result and help snapshot/behavior if existing tests cover it.

During implementation, add or move tests with ownership.

Required validation:

```sh
./scripts/check.sh
./scripts/check.sh extended
```

Focused Python checks:

```sh
python scripts/check_native_python_api.py --self-test
python scripts/check_python_typing_surface.py --self-test
python scripts/check_python_typing.py
python -m pytest crates/eggfetch-python/tests/test_streaming.py -q
python -m pytest crates/eggfetch-python/tests/test_close_races.py -q
python -m pytest crates/eggfetch-python/tests/test_async.py crates/eggfetch-python/tests/test_sync.py -q
```

Run directly affected compatibility streaming/response/lifecycle tests.

Focused CLI checks:

```sh
cargo test -p eggfetch-cli --all-features
cargo clippy -p eggfetch-cli --all-targets --all-features -- -D warnings
```

Also run package validation if source-layout changes affect wheel/sdist or
binary package-content checks.

## Acceptance criteria

- [x] Native Python API manifest has zero difference.
- [x] Python PEP 561 typing-surface check has zero difference.
- [x] No public PyO3 signature or `eggfetch.__all__` entry changes.
- [x] No Python exception hierarchy or accepted/rejected input changes.
- [x] Streaming state/close/drop/cancellation semantics are unchanged.
- [x] Sync streaming remains bounded and does not introduce worker blocking or
      extra full-body copies.
- [x] Text/line/raw iteration semantics remain unchanged across arbitrary
      chunk boundaries.
- [x] CLI `--help`/argument syntax/defaults and all documented behaviors are
      unchanged.
- [x] CLI exit codes, redaction and machine-output schema remain unchanged.
- [x] Python and CLI source responsibilities are materially easier to audit
      without introducing shared cross-adapter policy.
- [x] Tier 1 and extended validation pass.
- [x] No compatibility waiver is added.

## Stop conditions

Stop and split a separate corrective if decomposition requires:

- changing a Python public signature/type/name;
- changing sync vs async runtime ownership;
- changing queue capacity/backpressure or response-body lifetime;
- adding a new public Python class/helper;
- changing CLI syntax/output/exit behavior;
- moving language/UI policy into core;
- adding a new dependency solely to split modules;
- accepting behavioral drift as "equivalent."

The preferred result is a smaller coherent refactor rather than maximum file
fragmentation.

## Implementation record

Completed in the current qualification candidate:

- Python streaming private state, bridge, decoding, sync-iterator, and
  async-iterator responsibilities are split under
  `crates/eggfetch-python/src/streaming/`, while PyO3 classes and wiring remain
  in `streaming.rs`;
- CLI private argument, error, input, output, and file responsibilities are
  split into sibling modules while `main.rs` retains orchestration;
- public Python names/signatures, iterator lifecycle/backpressure/text/raw
  semantics, and CLI syntax/output/exit behavior remain unchanged.

The native API/typing checks, focused streaming/lifecycle tests, CLI clippy and
tests, Tier 1, and the extended qualification candidate passed. The final
freeze SHA and remote CI result are recorded by the closure plan.
