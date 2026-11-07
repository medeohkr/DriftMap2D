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

native results alone do not establish browser or WASM SIMD speedups. a separate browser engine timing appears below and does not demonstrate SIMD use. CPU stack sampling was attempted with `samply`, but Windows Performance Toolkit (`xperf`) is not installed. these results therefore combine direct phase timings, allocation instrumentation, and compiler IR inspection rather than a sampled flamegraph.

local logs, build configuration overrides, and LLVM IR used for this analysis are under ignored `data/engine_profile/`.

## first optimization pass

the native `cli` profile uses optimization level 3. the standard `release` profile now also uses level 3 for WASM, with `wasm-opt -O3` post-processing. validation runners build and use `target/cli/proteus.exe`. zero constant or Smagorinsky diffusion returns the existing advection buffer without random draws or forcing probes. completed releases return an unallocated empty vector.

the same 100,000 particle benchmark after those changes gave

| workload | original size build | CLI profile and fast paths | allocation calls | requested bytes per update |
| --- | ---: | ---: | ---: | ---: |
| generic Euler | 72.65 ms | 55.61 ms | 116 | 32,171,664 |
| leeway Euler | 73.39 ms | 55.55 ms | 116 | 32,171,664 |
| generic RK4 | 199.67 ms | 159.28 ms | 254 | 77,737,812 |

the empty release check now makes zero allocation requests. combined with the reused diffusion buffer, this removes 2 MB of allocation traffic per step at 100,000 particles. runtime gains are primarily from the speed profile. differences of a few percent between separate timing runs should be treated as noise, not evidence of an isolated fast-path speedup.

short deterministic Euler and RK4 runs were also compared through the actual CLI before and after optimization. all 17 snapshots in each case were exactly equal. stochastic realizations can change because disabled diffusion no longer consumes random draws, but the configured stochastic processes retain their probability distributions. tests cover positive diffusion, zero-buffer reuse, completed releases, and delayed scheduled releases.

## particle visualization

live rendering and playback now share an instanced WebGL circle layer with reusable buffers. positions stay in `Float32Array` snapshots instead of per-particle GeoJSON features. 100,000 coordinate pairs require 800,000 bytes per snapshot, excluding heatmap geometry and statistics. scenario files retain their existing MultiPoint format and convert only during import or export. playback updates only the visible view.

the browser harness at `web/tests/particles.html` compares 100,000 particles on a blank Mercator basemap. a headless Chrome run on November 6, 2026 measured median update-to-idle latency of 16.9 ms for the custom layer and 533.6 ms for rebuilt GeoJSON, over eight updates per renderer. these numbers include frame scheduling and are not measurements of simulation runtime or guaranteed interactive frame rates. the harness also checks rotation, pitch, high zoom, antimeridian crossings, repeated worlds, visibility, reset, and WebGL context recovery. see `web/tests/README.md` for commands.

## merged heatmap contours

`crates/proteus-wasm/src/heatmap/contours.rs` stitches marching-squares boundaries with grid vertex identities. internal cell edges are omitted, reversed edges cancel, and touching boundaries split into simple closed rings. straight intermediate vertices are removed. clockwise holes attach to their smallest containing shell. each concentration or probability level exports one MultiPolygon feature, retaining disconnected patches and the previous level order and interpolation.

the native geometry tests compare covered area and sample points against the old cell polygons. they cover all 16 marching-squares cases, all 19,683 three-by-three threshold configurations, larger plateaus, missing values, grid borders, holes, and nested islands. run them with `cargo test -p proteus-wasm heatmap`.

a headless Chrome comparison of the previous and rebuilt release WASM on November 6, 2026 gave

| example | old rings | merged rings | old GeoJSON bytes | merged GeoJSON bytes |
| --- | ---: | ---: | ---: | ---: |
| filled 100 by 100 grid, four levels | 39,204 | 4 | 7,145,069 | 773 |
| region with a hole and an island | 74 | 3 | 12,227 | 469 |
| smoothed two-cloud SAR example, four levels | 1,490 | 7 | 268,388 | 10,632 |

the filled grid's median contour generation and serialization time fell from 85.5 ms to 0.8 ms over seven calls. this is a synthetic dense case, not a simulation speedup measurement. MapLibre pixel comparisons for the hole and SAR examples found no changed pixels, and SAR density thresholds were identical. local comparison inputs, previous WASM, and results are under ignored `data/heatmap_union/` and `web/data/heatmap_union/`.

## shared sampling and reusable buffers

the start of each physics update now requests currents and wind together. oil includes temperature in that same request. tracer state changes still run before windage is calculated from the shared samples. RK2 and RK4 sample each intermediate position and time separately. wind that remains missing retains the second retry previously provided by the separate wind queries, without resampling currents.

the simulation retains active indices, environment samples, velocities, stage positions, and final positions between steps. both forcing backends retain their tile groups, fractional coordinates, sampled values, temporal values, missing indices, and dilation scratch. diffusion modifies the position buffer in place and retains its Smagorinsky probes. oil retains its weathering buffers and evaluates vapor pressures without allocating a vector per particle. buffers grow as needed and retain their largest capacity. the allocating forcing and integrator convenience methods remain available.

the leeway capsizing update also now indexes the compact wind samples by active position rather than particle ID. this prevents incorrect indexing after particles strand.

the following native comparison used the previous CLI binary and the updated engine with identical local NorKyst and landmask files, November 5 forcing, 100,000 particles, five warmup updates, and 30 timed repetitions

| workload | before median | after median | allocation calls before / after | requested bytes before / after |
| --- | ---: | ---: | ---: | ---: |
| generic Euler | 60.83 ms | 30.41 ms | 116 / 2 | 32,171,664 / 24 |
| leeway Euler | 55.58 ms | 30.56 ms | 116 / 2 | 32,171,664 / 24 |
| generic RK4 | 154.17 ms | 122.37 ms | 254 / 8 | 77,737,812 / 96 |

these allocations describe a warmed physics update, excluding release, forcing preparation, and downloads. date lookup formatting still makes small allocations per query. cold buffers, new tiles, changing tile groups, and larger releases can allocate. retained scratch capacity is separate from allocation traffic.

an engine-only headless Chrome comparison used 100,000 colocated generic particles with zero diffusion, Euler, 15 minute steps, and identical preloaded forcing. median WASM `step()` time fell from 121.1 ms to 81.1 ms over ten timed steps after four warmup steps. this includes cached forcing and landmask preparation, but excludes rendering, heatmap generation, and downloads. it is not an application frame-rate measurement.

deterministic generic, leeway, and oil runs were compared through both the CLI and WASM with Euler, RK2, and RK4. all 17 snapshots in each of the nine combinations matched exactly, including positions, oil diagnostics, and a midnight crossing. tests cover query counts and stage times, tracer changes before windage, shrinking active sets, missing wind retries, field clearing, and buffer reuse. comparison binaries, frozen forcing, logs, and browser results are under ignored `data/engine_profile/sharing/` and `web/data/engine_sharing/`.

## global and NorKyst comparison

`crates/proteus-cli/examples/compare_forcing.rs` measures both backends on the same fixed offshore cloud at 59 N, 3 E. it uses the current shared sampling and buffer changes above. this benchmark does not modify production engine code.

```sh
cargo run --offline --profile cli -p proteus-cli --example compare_forcing -- GLOBAL_ROOT NORKYST_ROOT LANDMASK_ROOT 20261105 30 1000,10000,100000
```

roots are tile directories expressed as URLs. the NorKyst root must be a local `file:///` URL because the projection diagnostic also reads its frozen manifest. arguments after the three roots are date, repetitions, and comma-separated particle counts. all cases currently use the fixed offshore location. use compatible archived tiles or an available date.

the November 6, 2026 comparison used optimization level 3 on the same machine as the earlier tests. local fixtures contained global tile `018_013`, NorKyst tile `003_005`, and the same raster and polygon landmask. both sources use November 5 forcing. file reads, decompression, parsing, and initial scratch allocation are excluded from timed phases. native inputs form a roughly 460 by 440 m square, restored outside each timed physics update. warmup sampling checks that all five fields are finite. physics cases also check finite final positions and no stranding.

native medians over 30 repetitions after five warmups, in milliseconds per 100,000 particles

| phase | global | NorKyst |
| --- | ---: | ---: |
| cached forcing preparation, including input copy | 3.84 | 10.00 |
| current and wind sampling at noon | 8.49 | 24.42 |
| current and wind sampling at 12.25 UTC | 11.79 | 28.62 |
| Euler physics, zero diffusion | 14.99 | 31.17 |
| Euler physics, constant diffusion 10 | 16.69 | 33.29 |
| RK4 physics, zero diffusion | 59.22 | 124.42 |
| Euler physics, Smagorinsky coefficient 0.1 | 42.70 | 134.74 |
| grid cell area queries alone | 0.34 | 7.85 |

the standalone phases overlap with work inside physics and must not be added to it as independent costs. native physics excludes forcing preparation and landmask preparation. the primary gap persists with warm caches and finite samples. both exact-hour and fractional-hour queries show it. results also scale roughly linearly from 10,000 to 100,000 particles. zero-diffusion Euler physics at 10,000 particles took 1.44 ms for global and 3.02 ms for NorKyst.

the NorKyst sampling path projects each input twice. `key()` converts longitude and latitude to projected grid coordinates for grouping, then `sample_into()` repeats that conversion for interpolation. `prepare_forcing()` projects every particle again. the current projection routine also evaluates eccentricity and standard-parallel expressions inside each call. a separate batch using that actual projection source took 7.14 ms for one pass over 100,000 positions. this is consistent with projection accounting for most of the native sampling gap, although it is not a sampled CPU attribution or an isolated speedup measurement.

an Euler step therefore performs three projection passes per particle across preparation and sampling. RK4 performs nine. Smagorinsky adds four current probes per particle, each following the same double-projection sampling path, plus a projection-dependent cell area calculation. global uses direct longitude and latitude arithmetic for its grid coordinates and a simpler cell area formula. the shared bilinear interpolation algorithm is the same for both sources.

a headless Chrome 156 comparison used the current release WASM, 100,000 initially colocated generic particles, 2 percent windage, Euler, 15 minute steps, and constant diffusion 10. twenty advances were timed after five warmups, with every engine tile and landmask already cached. the main combined test was repeated with the model order reversed and gave the following medians

| browser work | global | NorKyst |
| --- | ---: | ---: |
| WASM step in the combined test | 36.1 ms | 82.2 ms |
| scheduled JavaScript preloader work after that step | 0.0 ms | 31.9 ms |
| step and scheduled preloading together | 36.1 ms | 114.1 ms |

the preloader is a second significant difference. `simulationStep()` calls it every NorKyst step, while global calls it once per simulation day, every 96 steps at this timestep. even with cached downloads, the NorKyst preloader projects all particle positions and repeatedly builds string keys and tile objects for its 48 km margin. it runs on the browser's main thread even though the application does not await its promise. the combined benchmark awaits it to account for that work. it excludes rendering, heatmaps, history capture, and frame scheduling, so these are not full application frame-rate measurements.

in standalone JavaScript scans of the fixed square cloud, cached preloading took approximately 49 to 52 ms for NorKyst and 7 to 8 ms for global at 100,000 particles. the native envelope selected four preload tiles and the global envelope one. these calls used cached placeholder entries for additional prefetch targets, while engine sampling always used the real tile bytes. position export across the WASM boundary took about 0.4 ms. standalone timings vary with browser optimization and must not be added to the combined medians above.

the next optimization priorities are to avoid rescanning the full particle set for regional preloading every step, retain projected coordinates between grouping and interpolation, and cache the projection constants. a less frequent preload must still preserve the regional safety margin as particles move. Smagorinsky also warrants a cheaper cell area calculation. these are proposals, with no claimed speedups until implemented and checked against the existing trajectory comparisons.

raw native results, browser records, and the local browser driver are under ignored `data/engine_profile/forcing_compare/`. browser inputs and the local harness are under `web/data/forcing_compare/`. the browser harness finished with finite positions, all particles active, and no uncached engine requests or browser exceptions.

## Rust preloading and projection reuse

preload planning and background downloads now live in Rust. the TypeScript preloader and its global byte-cache hook are removed. marker edits call the WASM `preload_forcing` function with release coordinates. each subsequent engine step derives its background requests from the spatial plan already built during `prepare_forcing`, with no particle-position export or additional JavaScript scan. reset clears queued plans and cached tiles while retaining recently fetched metadata.

NorKyst projects each prepared particle once and retains its longitude, latitude, and double-precision grid coordinates. preparation accumulates bounds per occupied native tile, then expands those small envelopes by 12 km for required data and 48 km for background requests. the envelopes conservatively cover every particle margin and can include extra corner tiles. projected bounds are clipped to the actual grid, including its partial edge tiles. global reuses its required tile set and reuses that set across midnight rather than rescanning positions for the next date.

sampling reuses prepared coordinates only when both input coordinates match exactly. changed RK stages, reordered batches, and diffusion probes project their own positions. grouping stores the fractional native coordinates so interpolation never projects them a second time. projection constants are calculated once and reused, preserving the original arithmetic order. the retained coordinate and grouping buffers add memory proportional to the largest particle batch.

the WASM fetcher shares pending requests between background preloading and demand reads. up to four background downloads run concurrently. cached compressed bytes are parsed only when the engine needs the tile. plans cover the current and next date, skip already decoded tiles, and respect NorKyst date availability. changing the plan discards unneeded cached tiles and prevents stale downloads from repopulating the cache. failed preloads can still be retried on demand. metadata expires after five minutes. revision checks protect overlapping marker changes, simulation starts, and resets.

the same native comparison at 100,000 particles, 30 repetitions after five warmups, gave

| NorKyst phase | before | after |
| --- | ---: | ---: |
| one projection pass | 7.14 ms | 3.83 ms |
| cached forcing preparation | 10.00 ms | 6.04 ms |
| prepared current and wind sampling at noon | 24.42 ms | 8.83 ms |
| Euler physics, constant diffusion 10 | 33.29 ms | 17.25 ms |
| RK4 physics, zero diffusion | 124.42 ms | 79.80 ms |
| Euler physics, Smagorinsky coefficient 0.1 | 134.74 ms | 75.51 ms |

global Euler physics with constant diffusion 10 measured 16.61 ms in the updated run. the prepared-sampling row uses the same coordinates supplied to preparation. arbitrary changed coordinates still require one projection per query. the native physics rows exclude preparation and downloads.

a fresh headless Chrome comparison used the previous WASM and TypeScript preloader alongside the updated WASM, with real frozen tiles covering the full preload envelope. both versions advanced 100,000 generic particles from 59 N, 3 E with 15 minute steps. medians cover 20 steps after five warmups. requests were served from local frozen bytes, and the test verified that no requested fixture was missing.

| browser engine and preload work | before | after |
| --- | ---: | ---: |
| Euler, constant diffusion 10 | 130.9 ms | 51.0 ms |
| RK4, zero diffusion | 275.8 ms | 132.2 ms |
| Euler, Smagorinsky coefficient 0.1 | 302.9 ms | 138.9 ms |

the previous measurements include its JavaScript preload scan. the updated measurements include Rust preload scheduling within the engine call. these are cached engine and preload timings, excluding rendering, heatmaps, history capture, and frame scheduling. background fixture responses may occur as a moving cloud changes its preload envelope. the table does not predict internet download times or full application frame rates.

all 17 snapshots matched exactly before and after for global and NorKyst, generic, leeway, and oil, and Euler, RK2, and RK4. both CLI and WASM comparisons checked all particle positions and oil diagnostics across midnight. positive-diffusion timing cases are stochastic workloads rather than bitwise trajectory comparisons. browser checks also confirmed an eight-tile native release preload, four concurrent background requests, cancellation before a queued marker plan starts, and rejection of stale overlapping plans. the workspace has 97 passing Rust tests, and the seven particle-rendering tests and web production build pass.

local comparison binaries, native results, and browser records are under ignored `data/engine_profile/rust_preloader/`. frozen browser inputs and the local harness are under `web/data/rust_preloader/`.

## direct heatmaps and shared results

the engine now owns reusable heatmap and smoothing buffers. `Proteus.generate_heatmap()` bins active particles directly from Rust storage, using generic mass per particle, current oil mass, or unit leeway weights. it retains the previous grid bounds and alignment, padding, longitude shift, smoothing arithmetic, and SAR normalization. the standalone `HeatmapGenerator` API remains available for tools and comparisons.

the frontend caches the parsed GeoJSON by engine identity, completed step, grid size, smoothing, and contour levels. snapshot capture and live display share the same result. settings changes, new engines, and reset invalidate it. each new result owns separate geometry so later grid reuse does not alter saved snapshots. playback continues using saved GeoJSON.

a headless Chrome comparison at 100,000 particles, a small offshore grid, two smoothing passes, and four levels measured

| heatmap work | previous bridge | direct Rust and cache |
| --- | ---: | ---: |
| one generic heatmap | 2.6 ms | 1.2 ms |
| generic snapshot and live display on the same step | 4.6 ms | 1.1 ms |
| one SAR heatmap | 2.4 ms | 1.2 ms |
| SAR snapshot and live display on the same step | 4.5 ms | 1.1 ms |

these medians cover 20 calls after five warmups, including GeoJSON parsing. the previous bridge explicitly frees its temporary generator in the comparison. they are heatmap timings rather than full simulation or rendering speedups. large or fragmented grids can spend much more time generating contours.

all 14 browser comparisons produced exactly matching GeoJSON for generic, oil, and SAR examples, including fractional smoothing and antimeridian releases. native tests also compare every grid value and probability threshold, varying oil masses, stranding, invalid weights, empty and degenerate clouds, and reuse after moving or shrinking grids. app checks confirm a single generation for snapshot capture and display, immutable earlier snapshots, settings invalidation, saved playback, and reset invalidation. results and local harnesses are under ignored `data/direct_heatmap/` and `web/data/direct_heatmap/`.

## shared frames and simulation scheduling

the WASM step collects active coordinates once into a retained buffer. forcing and landmask preparation borrow that same slice. the CLI uses the same borrowed preparation paths. owned convenience APIs remain available for existing callers, while the production backends avoid the adapter allocation.

`Proteus.get_particle_frame()` collects active and stranded positions, bounds, counts, floating mass, and oil diagnostics in one pass. the frontend caches this owned frame by engine and completed step. rendering, statistics, bounds, and snapshot capture share its arrays. typed arrays copy out of Rust storage so subsequent buffer reuse cannot change saved snapshots. the standalone coordinate and diagnostic getters retain their existing behavior.

physics advances once per `requestAnimationFrame`, with live rendering after each completed step. snapshot intervals remain measured in physics steps, and the final state is always saved. the experimental batched scheduler and its separate drawing cadence were reverted.

pause and resume share a pending step rather than starting a second call into the same WASM object. reset cancels the driver and prevents old asynchronous results from publishing. view switching during a pending step uses the last completed cached frame or heatmap rather than borrowing the busy engine. queued animation frames are cancelled when a run is reset.

all saved trajectories, statistics, and heatmaps matched exactly during the scheduling comparison before batching was reverted. the 18 CLI and WASM combinations of forcing, tracer, and integrator also matched their previous trajectories across midnight. the new frame export matched all standalone getters, and exported frames remained unchanged after later engine updates. unit and app checks cover pending-step pause and resume, reset, busy-view switching, missing final snapshots, and cleanup. local records are under ignored `data/shared_frames/` and `web/data/shared_frames/`.
