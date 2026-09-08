# Node Binding Maturation

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`
Depends on: `plans/core-request-and-transport-consolidation.md`

## Objective

Resolve the mismatch between the repository's broad "additional bindings complete" positioning and the actual Node implementation. The Node crate must end this plan in one of two explicit states:

1. a deliberately experimental prototype with narrow documented guarantees; or
2. a supported binding that uses the Rust async engine directly, exposes practical body/stream/cancellation/error semantics, has generated TypeScript declarations, and is covered by the repository's existing validation workflow.

The preferred target is option 2 unless implementation constraints uncovered during the work make that unjustifiable.

## Current state

`eggfetch-node` currently:

- exposes async N-API methods but executes normal requests by calling the synchronous C ABI inside `spawn_blocking`;
- represents the client handle as a raw pointer cast to `usize` and relies on napi-rs object lifetime guarantees;
- accepts request bodies as `Option<String>` in the primary request methods;
- translates FFI failures into generic N-API error strings;
- does not expose the FFI streaming surface through normal Node response APIs;
- has an empty `index.d.ts`;
- has a JS test file but is not exercised by the normal `scripts/check.sh` Tier 1 path.

The current design is acceptable as a prototype but should not be the long-term supported binding architecture.

## 1. Decide and record the support contract

Before changing APIs, define the intended supported Node surface for the first stable binding.

Minimum supported contract if graduating from prototype:

- Node version floor compatible with chosen N-API level;
- async `Client` request methods;
- arbitrary HTTP methods;
- request headers;
- byte/string request bodies;
- response status/version/headers;
- buffered byte/text access;
- streaming response iteration;
- explicit close/cancellation behavior;
- structured error classes/codes;
- TypeScript declarations;
- no independent networking path outside `eggfetch-core`.

Optional capabilities such as full HTTPX-style configuration parity are not required initially.

Acceptance:

- [ ] One support contract is written in code/docs tests, not left implicit.
- [ ] Any intentionally unsupported capability is named explicitly.

## 2. Remove the blocking C ABI as the normal request execution path

The supported Node client should call `eggfetch-core` through a Rust-owned async client rather than execute `eggfetch_ffi::eggfetch_client_send` inside `spawn_blocking`.

Preferred structure:

- `EggfetchClient` owns or references a Rust async client/runtime integration suitable for napi-rs;
- N-API async methods await Rust futures directly through the napi runtime integration where supported;
- the C ABI remains independently useful for C consumers but is not the internal transport layer for Node;
- no new HTTP behavior is introduced in Node.

If direct async integration cannot be implemented safely with the current napi-rs version, document the blocker and keep Node explicitly experimental rather than pretending the blocking bridge is production-equivalent.

Acceptance:

- [ ] Supported Node requests do not call the blocking C ABI send function.
- [ ] No `spawn_blocking` is needed merely to perform ordinary network I/O.
- [ ] Client destruction cannot race in-flight requests into use-after-free.

## 3. Define safe client/request ownership

Replace raw-pointer lifetime reasoning in the supported path with Rust ownership where possible.

Requirements:

- client state is `Send + Sync` only where actually true;
- in-flight futures own/clone the state they need;
- dropping the JS wrapper does not invalidate active requests prematurely;
- explicit `close()` is idempotent if exposed;
- cancellation and GC/finalization have deterministic safe outcomes.

Keep `unsafe` constrained to unavoidable N-API/FFI boundaries.

Acceptance:

- [ ] No supported-path request future depends on a dangling raw client pointer.
- [ ] concurrent request + object-drop tests are green.

## 4. Expand request body support

Support at least:

- JavaScript string -> UTF-8 bytes;
- Node `Buffer` / `Uint8Array` -> bytes without lossy string conversion;
- empty body;
- streaming/async iterable body if napi-rs can bridge it safely without buffering the entire body.

For streaming request bodies:

- backpressure must be preserved;
- cancellation must release producer/consumer state;
- body replayability must be marked correctly for redirects/retries;
- no hidden eager buffering solely for API convenience.

If async-iterable upload support is deferred, document it explicitly and still provide byte bodies.

Acceptance:

- [ ] Binary request bodies round-trip without UTF-8 corruption.
- [ ] Known-length bodies set correct content length through the core.
- [ ] Streaming bodies, if implemented, are not replayed unsafely.

## 5. Add response streaming

Expose a Node-native streaming response API rather than only buffered response access.

Preferred interface may be an async iterator or a Node stream adapter, but must preserve:

- incremental core body consumption;
- backpressure;
- explicit/automatic close semantics;
- pool permit lifetime until body consumption/drop;
- cancellation propagation;
- correct raw/decoded behavior according to the selected Node API contract.

Do not implement a second buffering layer when the core already streams.

Acceptance:

- [ ] Large responses can be consumed incrementally with bounded memory.
- [ ] Partial consumption + close releases the core body/pool lease.
- [ ] cancellation stops further reads promptly.

## 6. Add practical request configuration

For a supported first release, expose the common configuration that maps directly to existing core primitives:

- headers;
- timeout;
- redirects;
- TLS verification/custom CA where straightforward;
- proxy;
- basic/bearer auth if already representable cleanly;
- HTTP/2 toggle/policy where supported.

Do not reproduce the Python HTTPX facade wholesale. Node should be idiomatic for Node while sharing core semantics.

Acceptance:

- [ ] Configuration is translated to core types rather than reimplemented.
- [ ] secret-bearing values are redacted in errors/debug output.

## 7. Structured errors

Replace generic `napi::Error::from_reason("eggfetch error [kind]: message")` as the only error contract.

Expose stable error codes/classes corresponding to useful core categories such as:

- invalid URL/request;
- connect;
- timeout with phase where available;
- TLS;
- proxy;
- protocol/H2/H3;
- body/decompression;
- cancellation/closed client.

Raw internal details may remain in message text, but consumers must not need to parse strings to classify failures.

Acceptance:

- [ ] Node tests can assert error category/code without string matching.
- [ ] secret redaction tests cover structured and string representations.

## 8. Generate and validate TypeScript declarations

`index.d.ts` must no longer be empty for a supported binding.

Use napi-rs declaration generation or another repository-local generation path consistent with the crate's build tooling. Do not hand-maintain a declaration file that can silently drift from Rust exports if generation is available.

Acceptance:

- [ ] declarations include Client, response, configuration and error surfaces;
- [ ] declaration generation is reproducible;
- [ ] a TypeScript compile smoke fixture imports and exercises the public API.

## 9. Node tests and existing validation integration

Expand `test.js` or migrate to a clearer local Node test layout covering:

- client construction/destruction;
- concurrent requests;
- all common methods;
- headers;
- binary body round trip;
- buffered response;
- streaming response;
- cancellation;
- timeout classification;
- invalid input;
- structured errors;
- close/GC races;
- TypeScript declaration smoke.

Integrate the Node checks into existing local validation conventions. The repository guide explicitly says not to add new CI jobs/matrices without specific need; prefer a Tier 1 or Tier 2 function in `scripts/check.sh` that runs when Node/npm prerequisites are available according to a clearly defined required/optional policy.

Because changing `scripts/check.sh` is qualification-sensitive, this executable plan intentionally precedes final HTTPX requalification.

Acceptance:

- [ ] normal repository validation no longer completely ignores the Node crate's JS surface.
- [ ] validation behavior when Node is unavailable is explicit and truthful.

## 10. Package/release readiness

If Node is promoted to supported:

- ensure package metadata is complete;
- verify native artifact loading on supported platforms;
- define whether publication is part of the current release process or remains manual/deferred;
- do not create a new automated publication pipeline unless separately requested.

If Node remains experimental:

- mark the crate/package/docs/roadmap clearly as experimental;
- do not claim additional-language-binding completion.

## Validation

At minimum:

```sh
cargo test -p eggfetch-node --all-features
./scripts/check.sh
```

Run the Node JS/TypeScript tests through the chosen existing validation hook. Run `./scripts/check.sh extended` at closure when environment prerequisites are available.

Do not renew HTTPX qualification here.

## Non-goals

- no Fetch API clone unless separately planned;
- no Axios compatibility facade;
- no browser/WebAssembly binding;
- no new language binding;
- no independent Node HTTP implementation;
- no automated npm release pipeline in this pass.

## Exit criteria

Either:

### Supported outcome

- [ ] direct Rust async-core dispatch;
- [ ] safe Rust-owned in-flight lifetime;
- [ ] binary body support;
- [ ] incremental response streaming;
- [ ] cancellation;
- [ ] structured errors;
- [ ] non-empty generated TypeScript declarations;
- [ ] Node tests integrated into validation;
- [ ] docs describe actual supported scope.

or:

### Experimental outcome

- [ ] prototype status is explicit everywhere;
- [ ] roadmap no longer classifies Node as completed supported binding;
- [ ] known blocking/string-body/streaming/declaration limitations are documented;
- [ ] no unsupported stability claims are made.
