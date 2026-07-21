# HTTPX Drop-In Phase 1: Production Semantics and Lifecycle

Status: ready for implementation handoff

## Purpose

Close the production-grade correctness gaps that must be resolved before eggfetch can safely serve as a long-lived HTTP client or support a credible HTTPX compatibility layer.

This phase is deliberately prior to broad API expansion. Adding more HTTPX-shaped classes while connect deadlines are not independently enforced, Python resource limits are unavailable, and client closure does not deterministically tear down resources would increase surface area without fixing the operational foundation.

## Baseline

The current implementation already provides:

- Rust async networking over Tokio/hyper/rustls;
- logical pool permits and hyper-managed connection reuse;
- pool, read, write, total, and nominal connect timeout fields;
- sync and asyncio Python clients;
- cancellation-safe permit ownership in several existing paths;
- Python context managers;
- resource-monitor and benchmark infrastructure;
- multi-platform Python tests.

Known gaps to verify at phase start include:

- connect timeout configuration is accepted but not independently enforced for the complete DNS/TCP/TLS path;
- default timeout phases are disabled in native eggfetch;
- Python does not expose an HTTPX-compatible `Limits` object or constructor argument;
- default logical concurrency is unlimited;
- `close()` and `aclose()` mark clients closed without necessarily dropping all owned client/runtime resources immediately;
- shared sync-client thread behavior is not established against HTTPX's contract;
- environment proxy behavior is currently deliberately disabled rather than selected by `trust_env`;
- long-running descriptor, thread, task, and memory stabilization evidence is incomplete.

Record the exact baseline SHA, test counts, resource-monitor values, and open release-gate issues before implementation.

## Non-goals

- Implementing the complete HTTPX object model.
- Adding custom transports, mounts, event hooks, ASGI, WSGI, or mock transports.
- Adding full Python upload streaming beyond changes needed for lifecycle correctness.
- Enabling retries by default.
- Making HTTP/3 production-default or part of HTTPX compatibility.
- Optimizing benchmark numbers before correctness and boundedness are established.
- Relaxing TLS verification or timeout behavior to make tests easier.

## Deliverables

1. Independently enforced connect deadlines across DNS, TCP, proxy, and TLS phases.
2. Explicit native and compatibility timeout-default policies.
3. A core resource-limit model that can support HTTPX-compatible Python `Limits`.
4. Bounded compatibility defaults for connections and keep-alive behavior.
5. Deterministic sync and async client shutdown.
6. A defined and tested thread-sharing contract.
7. Complete cancellation and timeout cleanup tests.
8. Long-running resource-stability evidence.
9. Explicit environment-trust behavior.
10. A phase status file with acceptance evidence.

## Track A — Correct timeout phase enforcement

### A1. Map the actual request lifecycle

Document and instrument the request path into explicit phases:

1. pool acquisition;
2. DNS resolution;
3. TCP connect;
4. proxy TCP connect;
5. proxy CONNECT exchange;
6. TLS handshake to origin or over proxy tunnel;
7. request headers and body write;
8. response headers read;
9. response body read;
10. total request/redirect/retry envelope.

Do not treat a single opaque connector future as evidence that every configured phase is enforced correctly.

### A2. Enforce DNS and TCP deadlines

Introduce connector-layer deadline propagation so `Timeout.connect` bounds at least:

- hostname resolution;
- each address attempt;
- aggregate address-attempt behavior according to the selected policy;
- TCP connection establishment;
- direct TLS handshake;
- proxy connection establishment;
- TLS handshake over CONNECT.

The implementation must define whether a connect timeout is:

- one shared budget across DNS, address attempts, TCP, and TLS; or
- reset per subphase.

The HTTPX compatibility layer must match the reference behavior as measured by Phase 0 fixtures. The native Rust API may expose additional explicit subphase controls only if they compose predictably.

### A3. Deadline propagation and remaining budget

Use an absolute deadline or remaining-budget model. Redirects and retries must not accidentally reset a total timeout. Nested timeout wrappers must report the correct phase without swallowing cancellation.

Add tests where:

- DNS blocks;
- the first resolved address stalls and a later address succeeds;
- TCP SYN remains pending;
- TLS handshake stalls after TCP success;
- proxy TCP connect stalls;
- CONNECT response stalls;
- origin TLS over tunnel stalls;
- total timeout expires during a phase with a longer phase timeout.

### A4. Exception mapping

Ensure timeout failures preserve:

- timeout phase;
- elapsed or configured deadline information where policy permits;
- attached request in the Python compatibility layer;
- source error where useful internally;
- distinction between connect, pool, read, and write timeout classes.

A total timeout may map to the compatibility hierarchy according to the measured reference contract, but it must never be mislabeled as an unrelated network error.

## Track B — Define timeout defaults

### B1. Separate native and compatibility defaults

The project may retain a native eggfetch policy different from HTTPX, but the compatibility layer must use the target profile's defaults.

Define explicitly:

- native Rust default timeout;
- native Python `eggfetch` default timeout;
- HTTPX compatibility-layer default timeout;
- top-level helper default behavior;
- client constructor default behavior;
- `timeout=None` behavior;
- scalar timeout behavior;
- per-phase `Timeout` behavior;
- request-level override and merge behavior.

The compatibility profile should use HTTPX 0.28.1's documented default of five seconds of network inactivity unless differential observation identifies a more specific nuance.

### B2. Remove indefinite accidental behavior

No production-facing default should allow an ordinary stalled network operation to hang indefinitely unless the user explicitly supplies `timeout=None` or an equivalent native opt-out.

Add tests proving:

- default connect stall raises;
- default read inactivity raises;
- periodic chunks reset the read inactivity deadline as expected;
- explicit `None` disables timeouts;
- partial per-phase objects require or inherit defaults according to the reference constructor rules;
- zero and negative timeout validation matches the target surface.

## Track C — Resource limits and connection policy

### C1. Add a core limits model

Create or formalize a core resource-limit structure that distinguishes:

- maximum concurrent logical requests;
- maximum concurrent requests per origin;
- maximum physical connections where the transport can enforce it;
- maximum keep-alive connections;
- maximum keep-alive connections per origin where supported;
- keep-alive expiry;
- HTTP/2 stream concurrency interaction;
- HTTP/3 stream concurrency interaction without exposing it as HTTPX parity.

Avoid naming a logical semaphore `max_connections` if it cannot represent the HTTPX meaning. If the core needs both logical request limits and physical connection limits, model both explicitly.

### C2. Expose HTTPX-compatible `Limits`

The compatibility layer must support:

- `Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=5.0)` defaults for the pinned profile;
- `None` values where supported;
- constructor validation;
- stable repr and equality behavior where observed;
- propagation to sync and async clients;
- pool timeout interaction when maximum connections are exhausted.

The native eggfetch API may expose additional per-origin limits, but compatibility defaults must not silently map to a semantically different control.

### C3. Physical connection accounting

Where hyper or another transport owns pooling, configure the transport's actual pool limits rather than relying only on eggfetch's logical permit layer.

Add instrumentation or test-only observability sufficient to prove:

- idle connection caps;
- keep-alive expiry;
- physical connection reuse;
- HTTP/1.1 maximum connection behavior;
- HTTP/2 multiplexing without one-per-request connection inflation;
- pool timeout when capacity is unavailable;
- dropped or closed bodies return capacity.

Production APIs do not need to expose internal socket counters if the transport cannot do so safely, but tests need black-box evidence.

## Track D — Deterministic client lifecycle

### D1. Own resources explicitly

Refactor Python client wrappers so close operations can take and drop owned resources deterministically. Likely patterns include:

- `Option<eggfetch_core::Client>`;
- an explicitly owned runtime handle or shared runtime lease;
- a close-state object separate from object existence;
- idempotent teardown guarded against concurrent requests.

`close()` and `aclose()` must do more than set a boolean.

### D2. Define in-flight close behavior

Specify and test what happens when a client is closed while:

- a request is waiting for pool capacity;
- DNS is running;
- TCP or TLS connection is pending;
- a request body is being produced;
- response headers are pending;
- a response body is actively streaming;
- an iterator has been handed to Python;
- multiple threads or tasks are using the client.

Match HTTPX where this is part of the public contract. Where behavior is undefined, choose a safe deterministic policy and document it.

### D3. Interpreter shutdown and finalization

Add subprocess tests for:

- client not explicitly closed;
- streaming response abandoned;
- pending async request cancelled during interpreter exit;
- repeated construction and destruction;
- `atexit` ordering;
- object cycles containing a client;
- Windows interpreter shutdown.

The process must terminate without deadlock, panic, leaked non-daemon threads, or excessive shutdown delay.

### D4. Context-manager guarantees

Prove that exiting sync and async client contexts:

- rejects later requests with the expected exception;
- releases idle sockets within the documented bound;
- does not invalidate fully buffered response data;
- closes or detaches streaming bodies according to reference semantics;
- remains idempotent when close is called inside the context.

## Track E — Thread and task concurrency contract

### E1. Shared sync client

Establish whether a single `Client` instance can be used concurrently from multiple Python threads, as HTTPX documents.

The compatibility target requires:

- safe concurrent method calls;
- no PyO3 mutable-borrow conflict exposed to users;
- no use of one non-reentrant runtime in a way that serializes or panics unexpectedly;
- connection pooling shared across threads;
- close/request races handled deterministically.

Refactor the sync bridge if necessary. Candidate designs include:

- one process-wide or module-wide shared Tokio runtime;
- a shared runtime service with per-call futures;
- `Arc`-owned core client with minimal Python mutable state;
- explicit synchronization around close state only.

Do not create one runtime per request.

### E2. Async concurrency

Test one `AsyncClient` across many asyncio tasks for:

- connection reuse;
- cancellation isolation;
- no global serialization;
- correct pool backpressure;
- close races;
- task-group cancellation;
- no retained tasks after completion.

### E3. Fork behavior

Document and test supported behavior when a client exists across `fork()` on POSIX. A safe policy may require clients to be constructed after fork. The implementation must fail clearly or reset state rather than reuse invalid runtime and socket state silently.

## Track F — Environment trust policy

### F1. Implement explicit `trust_env`

The compatibility layer must accept `trust_env` with the HTTPX default and measured behavior.

When enabled, evaluate at least:

- `HTTP_PROXY`;
- `HTTPS_PROXY`;
- `ALL_PROXY`;
- `NO_PROXY`;
- lowercase variants according to reference behavior;
- `SSL_CERT_FILE`;
- `SSL_CERT_DIR` where supported;
- netrc behavior assigned to the authentication phase.

The native eggfetch surface may default to explicit-only configuration, but the compatibility layer cannot ignore `trust_env` silently.

### F2. Environment isolation and security

Add tests that:

- sanitize inherited CI proxy variables;
- verify `trust_env=False` ignores all relevant variables;
- prevent proxy credentials from appearing in reprs or logs;
- handle malformed environment URLs safely;
- apply `NO_PROXY` matching consistently;
- do not leak environment proxy settings into custom transports unexpectedly.

## Track G — Resource stability and fault injection

### G1. Metrics

Extend test and benchmark instrumentation to observe:

- file descriptors or handles;
- active TCP connections;
- Python threads;
- Tokio tasks where test instrumentation permits;
- resident memory;
- heap allocation trend;
- pool waiters and permits;
- DNS tasks;
- iterator bridge workers;
- client and response object counts in Python stress fixtures.

### G2. Repeated-failure tests

Run bounded high-iteration workloads for:

- connection refused;
- DNS failure;
- TLS verification failure;
- read timeout;
- write timeout;
- pool timeout;
- proxy rejection;
- cancellation at every phase;
- early response drop;
- abandoned streaming iterator;
- repeated client open/close.

Resource counts must return to a defined steady-state envelope.

### G3. Soak tests

Add scheduled or manually required soak profiles rather than placing all long tests in ordinary PR CI.

At minimum:

- sustained keep-alive requests;
- mixed origins;
- HTTP/2 multiplexing;
- slow streaming;
- repeated cancellation;
- proxy traffic;
- TLS session churn;
- client creation/destruction churn.

Store machine-readable reports with candidate SHA, platform, duration, workload, peak values, and final stabilization values.

## Expected files

Likely changes include:

- `crates/eggfetch-core/src/connector/`
- `crates/eggfetch-core/src/timeout.rs`
- `crates/eggfetch-core/src/pool.rs`
- `crates/eggfetch-core/src/client.rs`
- `crates/eggfetch-python/src/client.rs`
- `crates/eggfetch-python/src/async_client.rs`
- `crates/eggfetch-python/src/timeout.rs`
- `crates/eggfetch-python/src/limits.rs`
- `crates/eggfetch-python/tests/production/`
- `crates/eggfetch-bench/`
- `docs/concepts/timeouts.md`
- `docs/concepts/pooling.md`
- `docs/python/guide.md`
- `compat/httpx/0.28.1/allowed-differences.toml`
- `.github/workflows/ci.yml`
- `plans/httpx-drop-in-phase-1-status.md`

## Required tests

The implementation must add focused tests for:

- DNS timeout;
- TCP timeout;
- TLS handshake timeout;
- proxy and tunneled TLS timeout;
- phase-versus-total deadline precedence;
- default timeout behavior;
- `timeout=None`;
- pool capacity and pool timeout;
- keep-alive limits and expiry;
- deterministic close;
- close during every request phase;
- sync client use from many threads;
- async client use from many tasks;
- close/request races;
- interpreter shutdown;
- repeated failure stabilization;
- fork policy;
- environment proxy and certificate behavior;
- `trust_env=False` isolation.

## Validation commands

The status file must record exact commands. Expected command families include:

```bash
cargo test -p eggfetch-core timeout
cargo test -p eggfetch-core pool
cargo test -p eggfetch-core connector
pytest crates/eggfetch-python/tests/production -q
pytest crates/eggfetch-python/tests/compat -q
cargo run -p eggfetch-bench --bin resource_monitor
```

Long-running profiles should have explicit scripts and produce JSON reports.

## Acceptance criteria

This phase is complete only when:

- [ ] DNS resolution is bounded by the configured connect deadline.
- [ ] TCP connection establishment is bounded by the configured connect deadline.
- [ ] Direct TLS handshake is bounded by the configured connect deadline.
- [ ] Proxy TCP, CONNECT, and tunneled TLS phases are bounded and correctly classified.
- [ ] Total timeout is an absolute envelope across redirects and retries.
- [ ] Timeout exceptions map to the correct compatibility classes and retain request context.
- [ ] Compatibility defaults match HTTPX 0.28.1's measured timeout policy.
- [ ] Explicit `timeout=None` disables timeout enforcement as expected.
- [ ] A core limits model distinguishes logical request limits from physical pool limits.
- [ ] Python exposes HTTPX-compatible `Limits` construction and defaults.
- [ ] Default compatibility clients are bounded to the reference connection and keep-alive limits.
- [ ] Pool timeout behavior is proven under exhausted capacity.
- [ ] `close()` deterministically releases client and runtime-owned resources.
- [ ] `aclose()` deterministically releases client-owned resources.
- [ ] Context manager exit has tested socket and stream behavior.
- [ ] Shared sync-client use from multiple threads is safe and compatible.
- [ ] Shared async-client use from many tasks is safe and non-serialized except by configured limits.
- [ ] Close/request races do not panic, deadlock, or leak resources.
- [ ] Interpreter shutdown tests pass on Linux, macOS, and Windows.
- [ ] `trust_env` matches the compatibility profile and `trust_env=False` isolates the process environment.
- [ ] Repeated-failure resource tests stabilize within committed thresholds.
- [ ] Scheduled soak profiles produce retained machine-readable evidence.
- [ ] No acceptance criterion is satisfied solely by documentation or a unit test that bypasses the real connector.
- [ ] `plans/httpx-drop-in-phase-1-status.md` links a green required CI run and exact SHA.

## Handoff notes

Implementation should prefer architectural correction over adapter-level timeout wrappers. A Python timer around an opaque request does not establish independently correct connect, pool, read, and write semantics. Likewise, a logical semaphore is not an adequate substitute for actual transport pool limits. The status document must explain how each public control maps to the transport behavior it claims to govern.
