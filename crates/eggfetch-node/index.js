// Experimental prototype entry point (see docs/architecture/ffi-and-node.md).
// API surface is unsupported and may change without notice. Expects the
// napi-rs build artifact copied to `./eggfetch.node` (e.g. via `npm run
// build` followed by copying the triple-suffixed `.node` file); Tier 1
// validation skips the JS surface when that artifact is absent.
const { EggfetchClient, EggfetchResponse } = require('./eggfetch.node');

module.exports = {
  EggfetchClient,
  EggfetchResponse,
  default: { EggfetchClient, EggfetchResponse },
};
