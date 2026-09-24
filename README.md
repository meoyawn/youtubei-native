# youtubei-native

**Experimental artifact.** The initial integration failed its performance goal
against QuickJS. After profiling and optimization, this sample produces a smaller
binary and modestly faster flat scans, but still uses more RAM and takes longer
to rebuild. See the [full comparison and limitations](BENCHMARKS.md). It has not
established an overall replacement for QuickJS.

An experimental safe Rust API over **youtubei.js 18.1.0 compiled to native C**
with Static Hermes. No QuickJS, Node subprocess, or handwritten C/C++ shim is
used at runtime. Application JavaScript is AOT compiled. Hermes's C++ runtime
remains linked, including an interpreter and embedded bytecode for its own
internal JavaScript built-ins in the tested configuration.

```rust,no_run
use youtubei_native::Youtube;

let mut youtube = Youtube::new()?;
assert_eq!(youtube.session_client_name()?, "WEB");
for video in youtube.playlist("PLAYLIST_ID")? {
    println!("{} {}", video.id, video.title);
}
# Ok::<(), youtubei_native::Error>(())
```

## Boundary and ownership

Following the [rusqlite](https://github.com/rusqlite/rusqlite) approach,
`src/ffi.rs` contains private raw C declarations. `Youtube` owns the runtime,
releases it in `Drop`, and returns owned Rust values and typed errors. Callers
never handle native pointers or `unsafe`. Mutating calls require `&mut self`.
The wrapper is neither `Send` nor `Sync`; a second live owner returns `Busy`
instead of violating the pinned runtime's native-unit ownership contract.

Rust calls generated `sh_export_youtubei_*` units through Hermes's guarded C
entry point. The driver unit is reused for subsequent calls. `js/host.js` and
`js/driver.js` provide native callback declarations and value transfer; Hermes
compiles both to C. They are not interpreted at runtime.

Host responses use the pinned runtime's native UTF-8 string-copy API. Rust keeps
the response buffer alive until that immediate copy completes; explicit lengths
preserve embedded NULs and ownership stays in Rust. The initial JS heap is 8 MiB
and grows with demand. Dropping the owner also releases retained host buffers and
its HTTP client.

Only required host APIs are adapted:

| API | Implementation |
| --- | --- |
| HTTP and headers | `reqwest` |
| URL parsing and query encoding | `url` |
| UTF-8 / UTF-16 conversion | Rust standard library |
| JSON boundary and typed results | `serde_json` / `serde` |
| Event listeners and request/response shape | Small JavaScript adapters compiled to C |

No browser-polyfill packages are installed. There are no timer, Intl, DOM,
streaming, WebCrypto, or storage polyfills. Missing functionality is outside
the supported API rather than silently simulated.

## Build

Currently validated on **macOS 26.4, Apple Silicon, Rust 1.96**. The linked
runtime was built for macOS 26; older deployment targets and other platforms
are unverified. Prerequisites: Rust, Apple Clang, CMake, and nub.
JavaScript dependencies use `nub.lock`; nub runs esbuild during the Cargo build.

```sh
nub install --ignore-scripts --frozen-lockfile
sh scripts/build-hermes.sh
MACOSX_DEPLOYMENT_TARGET=26.0 cargo test -- --nocapture
```

Hermes is pinned to `7508017ae267ecffe4c4df38656713034f35d9bf` from
[`static_h`](https://github.com/facebook/hermes/tree/static_h).
The script creates `.hermes/source` and `.hermes/build`. Setup accepts
`HERMES_SOURCE_DIR` and `HERMES_BUILD_DIR`; Cargo selects an existing compiler
and runtime through `HERMES_BUILD_DIR`.
The compiler, headers, runtime libraries, and release build model must match.

`build.rs` performs this pipeline automatically:

```text
youtubei.js/web.bundle + two small local modules
    → esbuild (one IIFE)
    → shermes -c (object files; C/Clang handled internally)
    → ar → libyoutubei.a
    → Rust + statically linked Hermes runtime
```

The package already ships a self-contained browser bundle. Esbuild reads
**three inputs**, not the scattered `dist/` module graph. It only links our
adapter; upstream already preserves names. We supply no target downgrade, custom plugins,
source patches, or bundle postprocessing. Upstream's shipped bundle already
contains its own transpilation helpers. Hermes enables standard block scoping
and async generators with `-Xes6-block-scoping -Xasync-generators`.
Direct compilation of the untouched upstream bundle fails at its ESM export
statement in this compiler, so one standard esbuild pass remains. No custom
export rewriting is used.

The two small native callback units are compiled separately in Hermes's typed
mode. Object files are generated inside Cargo's `OUT_DIR`; Hermes manages its
temporary C files, compiler flags, and include paths. Users of the Rust API do
not need a running JavaScript engine process.

## Verified scope and remaining work

The test uses a local HTTP server and **real reqwest requests**. It checks
UTF-8 and embedded NUL results, native exception propagation, recovery after an
error, exclusive ownership, destruction/reopening, local WEB session creation,
request context/headers, flat playlist parsing, and `getContinuation()` through
the final page. No live YouTube or media requests are made by these tests.

Current public methods cover text parsing, local sessions, classic
`PlaylistVideo` and modern `LockupView` listings. A live anonymous WEB scan of a
504-entry playlist matched every ID and its order against a complete flat
yt-dlp extraction, including four unavailable placeholders. Listing age labels
remain strings; no per-video lookups occur. See [measurements](BENCHMARKS.md).
Authentication, player-script execution, and other package surfaces remain
unverified. HTTP currently blocks the owning thread and reuses a reqwest client
across requests. Responses are buffered as UTF-8 text with an 8 MiB limit. Binary
response bodies are outside the supported listing API. This is a proof of the
native integration, not a complete or release-ready replacement for the package.

Promise jobs are drained through the pinned runtime's internal test hook.
Before publishing, replace that experimental hook with a supported embedding
API, finish the portable runtime/build distribution, validate the supported
YouTube operations, and declare a tested minimum Rust version. Crate/repository
name availability has not been checked. The crate has not been published to crates.io.

For development, `task lint` formats before running Clippy, and `task test`
runs the native integration test. See `THIRD_PARTY.md` for upstream licenses.

`cargo run --example benchmark` measures runtime creation, repeated text calls,
and offline session creation through the safe Rust API. The example validates
every result, including Unicode and embedded NUL text. It performs no network
requests. Native compilation uses Hermes `-O` with C optimization capped at
`-O1` (`-Wc,=-O1`), independently of the Rust profile. A clean native build with
these flags completed in 43.3 seconds on the test machine. Use Cargo's release
profile for runtime comparisons; the original debug measurements are retained
in the benchmark history.

`cargo run --example playlist_benchmark -- PLAYLIST_ID` performs two complete
live flat scans and emits their entries and timings as JSON. It includes no
cookies or authentication. The first measurement should include `startup_ms`
when comparing it with an integration that initializes its engine lazily.
