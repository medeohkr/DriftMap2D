# visualization and simulation checks

run the array conversion, projection, buffer reuse, and resource lifecycle tests with Node 24 or newer

```sh
npm run test:particles
```

run `npm run dev` and open `http://localhost:8080/tests/particles.html` for real WebGL checks. the page checks colors, rotation, pitch, high zoom precision, repeated worlds, visibility, reset, and context recovery. it draws all 100,000 particles and compares update-to-idle latency against a GeoJSON circle layer on the same blank Mercator map. results appear below the map. the browser requires access to the same MapLibre CDN used by the app.

the benchmark includes projection, buffer upload, drawing, and frame scheduling. the GeoJSON case includes feature construction and source processing. it does not measure physics, heatmap generation, or full application performance. headless results can differ from an interactive browser.

heatmap caching and shared particle frames have separate checks

```sh
npm run test:heatmap
npm run test:simulation
```

the simulation checks cover animation-frame scheduling, pending-step pause and resume, cancellation, errors, final rendering, queued-frame cancellation, and sharing owned frames without changing earlier snapshots. Rust heatmap tests compare direct particle binning, smoothing, probability thresholds, and contours with the previous exported-array pipeline. run them with `cargo test -p proteus-wasm heatmap` from the repository root.
