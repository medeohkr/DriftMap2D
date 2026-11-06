# engine profiling

`crates/proteus-cli/examples/profile_engine.rs` benchmarks the actual Proteus core with cached NorKyst fields and the polygon landmask. it does not render, generate heatmaps, or write particle histories. production engine code and build profiles are unchanged by this benchmark.

```sh
cargo run --profile cli -p proteus-cli --example profile_engine -- 100000 30 sar euler
```

arguments are particle count, repetitions, tracer (`generic` or `sar`), integrator (`euler`, `rk2`, or `rk4`), forcing root, landmask root, and date as `YYYYMMDD`. trailing arguments are optional. the default date is November 5, 2026. replace it with an available date when those tiles expire.

the cloud starts at 59 N, 3 E with a 500 m Gaussian radius. measurements use noon on the selected date and a 15 minute timestep. generic windage is 2 percent. leeway uses PIW-1 coefficients, alternating initial orientation, zero jibing, and no capsizing. diffusion is zero. five warmup updates run before measurement. coordinates and stranding flags are restored outside the timed update so each repetition samples the same small offshore region. this is a hot-cache throughput test, not a full trajectory simulation or coastal worst-case benchmark.

the allocation counter records allocation and reallocation requests during a separate update outside the timing repetitions. requested bytes are cumulative allocation traffic, not peak or retained memory. timed runs retain the counter's inactive thread-local check, so results include a small instrumentation overhead. empty releases and cached forcing and landmask preparation are measured separately from physics.

## measured results

measured on an AMD Ryzen 7 8845HS running Windows with Rust 1.94.0 and LLVM 21.1.8. 30 repetitions per case, real NorKyst forcing, no downloads inside the timed physics update. both builds use LTO and one codegen unit, with debug symbols for inspection. only optimization level differs. values are median milliseconds per physics update.

| workload | original size profile `z` | speed profile `3` | allocation calls | requested bytes per update |
| --- | ---: | ---: | ---: | ---: |
| generic Euler, 10,000 particles | 7.29 | 5.69 | 99 | 3,821,584 |
| generic Euler, 100,000 particles | 72.65 | 56.95 | 117 | 32,971,664 |
| leeway Euler, 100,000 particles | 73.39 | 57.19 | 117 | 32,971,664 |
| generic RK4, 100,000 particles | 199.67 | 156.56 | 255 | 78,537,812 |

allocation counts and requested bytes were identical across the two optimization levels. an empty release call requests another 1,200,000 bytes at 100,000 particles despite releasing nothing.

for the current size build at 100,000 leeway particles, standalone wind/temperature sampling averaged 28.50 ms and currents/wind sampling averaged 33.22 ms. these measurements are separate from the full-step timing and must not be treated as exact nested percentages. cached forcing preparation averaged 15.33 ms and cached landmask preparation 6.96 ms. both preparation stages still scan particle coordinates even with no new downloads.

## SIMD findings

LLVM IR confirms that a dense `position += dt * velocity` slice loop produces four-wide floating-point multiply and add operations at optimization level 3. the same loop stays scalar at `z`. an otherwise similar loop addressing positions through an index array stays scalar at both levels. in the full benchmark executable, the 100,000-element dense loop took about 8 microseconds at level 3 versus 48–51 microseconds for the indexed loop.

these arithmetic loops are microbenchmarks rather than the entire integrator. their cost is much smaller than field sampling, so vectorizing only the final addition will not explain or remove most engine time. the existing separate longitude and latitude arrays are a good starting point. indirect active-particle indices, hash-based grouping, branches, and scalar geographic transforms make the full update less straightforward to vectorize.

the first optimization candidates are shared initial field sampling for tracer updates and advection, precomputed projection constants, reusable interpolation and integration buffers, an empty-release fast path, and a zero-diffusion fast path. dense batches and prepared interpolation coordinates can then expose more useful SIMD work. any active-particle compaction must preserve tracer state and stable exported particle IDs.

native results do not establish browser or WASM SIMD speedups. the web build needs a separate measurement with its own compiler and WASM target settings. CPU stack sampling was attempted with `samply`, but Windows Performance Toolkit (`xperf`) is not installed. these results therefore combine direct phase timings, allocation instrumentation, and compiler IR inspection rather than a sampled flamegraph.

local logs, build configuration overrides, and LLVM IR used for this analysis are under ignored `data/engine_profile/`.

## first optimization pass

the native `cli` profile uses optimization level 3. the standard `release` profile now also uses level 3 for WASM, with `wasm-opt -O3` post-processing. browser performance has not yet been benchmarked. validation runners build and use `target/cli/proteus.exe`. zero constant or Smagorinsky diffusion returns the existing advection buffer without random draws or forcing probes. completed releases return an unallocated empty vector.

the same 100,000 particle benchmark after those changes gave

| workload | original size build | CLI profile and fast paths | allocation calls | requested bytes per update |
| --- | ---: | ---: | ---: | ---: |
| generic Euler | 72.65 ms | 55.61 ms | 116 | 32,171,664 |
| leeway Euler | 73.39 ms | 55.55 ms | 116 | 32,171,664 |
| generic RK4 | 199.67 ms | 159.28 ms | 254 | 77,737,812 |

the empty release check now makes zero allocation requests. combined with the reused diffusion buffer, this removes 2 MB of allocation traffic per step at 100,000 particles. runtime gains are primarily from the speed profile. differences of a few percent between separate timing runs should be treated as noise, not evidence of an isolated fast-path speedup.

short deterministic Euler and RK4 runs were also compared through the actual CLI before and after optimization. all 17 snapshots in each case were exactly equal. stochastic realizations can change because disabled diffusion no longer consumes random draws, but the configured stochastic processes retain their probability distributions. tests cover positive diffusion, zero-buffer reuse, completed releases, and delayed scheduled releases.
