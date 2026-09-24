# QuickJS comparison

**A scan means fetching a complete flat playlist listing**, including every
continuation page. It returns entry IDs and available listing metadata, without
fetching individual video details, resolving media URLs, or downloading media.

The workload is two complete, anonymous flat playlist listings of
**Tim Ventura Interviews**
(`PLipBN7O7_H3oq9oDRWagdZUoBOV81GCkT`): 504 entries across continuation pages,
including four unavailable placeholders. Every scan must match yt-dlp's complete
flat ID sequence. Both engines must also match title, duration, and availability.
Both use youtubei.js **18.1.0**. Each process makes fresh playlist requests for
the second listing; it does not just return the first listing's saved results.

Measurements use macOS 26.4 / Apple Silicon, Rust 1.96, fresh processes, alternating
engine order, and three runs each unless indicated otherwise. Each process is
limited to 60 seconds. Live network variation remains; small timing differences
are not evidence of an engine throughput advantage. Peak RSS includes both scans,
and CPU time covers the entire process. The standalone executables exclude the
desktop application and its media/database dependencies.

The original QuickJS integration uses rquickjs 0.11.0 and LLRT 0.8.1-beta, with its
existing Rust worker and JS bridge unchanged. Only its package dependency is
aligned to 18.1.0. It caches a remotely initialized session and uses async reqwest.
Hermes creates local WEB sessions without fetching config/player, uses a pooled
blocking reqwest client, and returns listing age labels without normalizing them
to calendar dates. Thus this compares complete integration choices, not just VM
instruction throughput. Making HTTP async would improve caller scheduling; it
cannot parallelize dependent continuation pages.

## Optimization history

The initial byte-array HTTP adapter took approximately 15.1 seconds per scan and
512 MiB peak RSS in a development build. Returning response text directly removed
the JSON number-array expansion: the three-run development medians became 4.72 /
4.73 seconds for first / second scan and 71.0 MiB RSS. QuickJS's corresponding
development medians were 3.98 / 3.34 seconds and 68.3 MiB RSS.

Subsequent changes reuse HTTP connections, release retained host buffers with the
runtime, optimize the generated code, and lower the initial heap from 32 to 8 MiB.
The heap remains growable. Profiling additionally identified per-character string
construction in the host adapter; it now copies UTF-8 through the existing Hermes
`_sh_asciiz_to_string` API. Explicit lengths and immediate native copying preserve
Unicode and buffer ownership without adding a handwritten C bridge.

Before that final string-copy change, three-run **release** medians were:

| Measurement | QuickJS | Hermes |
| --- | ---: | ---: |
| First complete flat playlist listing, including startup | 2.63 s | 2.68 s |
| Second complete flat playlist listing in the same process | 2.16 s | 2.40 s |
| Process CPU time, both listings | 0.76 s | 1.18 s |
| Peak RSS | 64.3 MiB | 69.1 MiB |
| Executable size | 16.96 MiB | 14.77 MiB |

Rust uses the release profile in both runners. Hermes uses `-O` for its IR and
`-Wc,=-O1` for native C; these are distinct optimization settings. The
[pinned compiler implementation](https://github.com/facebook/hermes/blob/7508017ae267ecffe4c4df38656713034f35d9bf/tools/shermes/compile.cpp)
appends the explicit C option after its default. A fresh esbuild → native units →
archive build took **43.34 seconds**; a warmed-dependency Cargo release rebuild
took **48.05 seconds**. The archive is **9.62 MiB**, excluding the runtime. The
earlier unoptimized pipeline took 14.68 seconds and produced a 15.46 MiB archive.
One-time compiler/runtime installation is excluded from package rebuild timings.

Raw summaries, including individual runs and binary hashes, are in
[benchmark-results.json](benchmark-results.json). The history distinguishes
single-run probes from repeated comparisons. No result here establishes a
runtime-free implementation: the tested Hermes configuration still includes its
C++ runtime, GC, interpreter, and bytecode for internal built-ins.

## Final comparison

After bulk UTF-8 transfer, the final release comparison is:

| Measurement | QuickJS | Hermes |
| --- | ---: | ---: |
| First complete flat playlist listing, including startup | 2.69 s | 2.29 s |
| Second complete flat playlist listing in the same process | 2.25 s | 2.07 s |
| Whole process wall time, both listings | 4.85 s | 4.66 s |
| Whole process CPU time, both listings | 0.86 s | 0.75 s |
| Peak RSS | 66.4 MiB | 74.8 MiB |
| Executable size | 16.96 MiB | 14.77 MiB |
| Forced bundle + release runner rebuild, cached dependencies | 4.24 s | 43.75 s |

Runtime timings and peak RSS are medians of three runs. Rebuild measurements
are individual forced builds and include the JS build and Rust runner. The
QuickJS measurement forces the original build script to rerun; it embeds JS source.
Hermes additionally produces native machine code. Compiler/runtime installation
and first-time Rust dependency compilation are excluded from both figures.

The initial Hermes integration failed its performance goal. Profiling and
optimization produced smaller binaries and modestly lower scan/CPU times in this
sample, while peak RAM and rebuild latency remain worse. This supports keeping
the experiment for reference; it does not establish an overall replacement for
QuickJS. Three live repetitions cannot isolate network effects or predict other
playlists, concurrent workloads, authentication, or player execution. The future
pure-Rust compiler must be measured on the same workload before judging it.

## Run the native workload

```sh
MACOSX_DEPLOYMENT_TARGET=26.0 cargo build --release --example playlist_benchmark
/usr/bin/time -l target/release/examples/playlist_benchmark PLipBN7O7_H3oq9oDRWagdZUoBOV81GCkT
yt-dlp --ignore-config --flat-playlist --skip-download --dump-single-json \
  'https://www.youtube.com/playlist?list=PLipBN7O7_H3oq9oDRWagdZUoBOV81GCkT'
```

The example emits entries and scan timings as JSON. Add `startup_ms` to the first
scan when comparing with an engine that initializes lazily. Use a fresh process
for each repetition and avoid overlapping benchmark or compiler processes.
