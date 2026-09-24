# Upstream components

The generated native code includes code from these projects:

- [youtubei.js](https://github.com/LuanRT/YouTube.js), MIT, LuanRT and contributors.
- [Hermes](https://github.com/facebook/hermes), MIT, Meta Platforms, Inc. and affiliates.
- Hermes's bundled [Boost.Context](https://www.boost.org/libraries/context/), Boost Software License 1.0.
- youtubei.js's browser bundle incorporates protobuf, fflate, and meriyah;
  their notices are distributed with the upstream package/source releases.

Rust dependency versions are recorded in `Cargo.lock`; JavaScript versions and
integrities are recorded in `nub.lock`. Building fetches upstream sources
through the explicit setup steps. Their source files retain their own licenses.
Any binary or vendored-source release must carry the applicable upstream
license notices in addition to this crate's MIT license.
