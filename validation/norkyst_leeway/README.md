# NorKyst leeway validation

compare Proteus SAR and OpenDrift Leeway on the regional native grid from
october 28 to october 30, 2026 UTC. defaults match the existing leeway suite
with 100,000 particles, hourly Euler steps, zero release radius, no diffusion,
no jibing, and no capsizing. the nearshore grid spans 5.1–5.3° E and 59–59.2° N
west of Stavanger and Karmøy. starts on land are excluded. this region is chosen
to allow stranding within 48 hours. the previous offshore run is preserved in
`runs/offshore/`.

the CLI selects `PIW-1` from the project's local `catalogs/object_catalog.json`
using `tracer_json.object_id` and an explicit absolute `catalog_dir`. no leeway
coefficients are embedded in CLI configs. the suite verifies all nine catalog
coefficients against OpenDrift before starting. `--object-id` selects another
catalog ID. generate missing catalogs with `python scripts/build_catalogs.py`.

```powershell
conda activate opendrift
python validation/norkyst_leeway/run.py --dry-run
python validation/norkyst_leeway/run.py --limit 1
python validation/norkyst_leeway/run.py
python validation/norkyst_leeway/analyze.py
```

paths resolve independently of the working directory. `--particles` accepts a
positive even ensemble size, `--grid-size` changes the grid, `--skip-build`
reuses the release CLI, and `--source`, `--tile-url`, and `--catalog-dir` override
inputs. smaller ensembles are useful for smoke checks but increase sampling
noise. use a fresh runs directory when changing the object or ensemble size.

the shared NorKyst reader helper caches actual native currents and wind in
`runs/source_surface_20261028_wind.nc`. wind is not replaced with zero. output
includes configs, metadata, CLI logs, NetCDF trajectories, centroids, the object
reference record, and the published manifest. incomplete comparisons are
excluded. analysis produces relative and absolute CSVs and a two-panel plot
with means and 10–90th percentile bands.

OpenDrift and Proteus use independent random generators for coefficient
uncertainty. both alternate right and left orientations with jibing disabled.
OpenDrift uses a recorded per-run random seed. this compares ensemble centroids,
not identical particle paths. mutable source forecasts and published tiles may
still represent different revisions, as in the generic suite.

## visual comparison

```powershell
python validation/norkyst_leeway/visualize.py 0
python validation/norkyst_leeway/visualize.py run_039
```

each run gets a `visualization/` directory containing `side_by_side.png`,
`overlay.png`, and `side_by_side.gif`. both panels use identical extents and
physical aspect ratios. these tracks show ensemble centroids. six-hour labels
and final separation help compare timing and direction.

add `--rerun` to recreate both models from the selected run's config and recorded
OpenDrift seed. rerun outputs stay in `visualization/` and do not replace suite
results. use the activated OpenDrift environment for reruns. source caches are
reused and CLI forcing tiles come from the configured URL, so changed published
forecasts can change the reproduced result. `--skip-animation` exports only PNGs.

`--rerun` also enables streamed CLI particle output in `particles.jsonl` and
creates `particle_clouds.gif`. both models still simulate all 100,000 particles,
but the animation defaults to a fixed random sample of 1,000 IDs per model.
`--sample-particles 100000` displays the full clouds at greater rendering cost.
samples remain stable across frames and stranded particles appear in black.
the cloud GIF includes grey land and coastlines, and reports stranded counts
for the displayed sample. OpenDrift's last stranded positions are retained after
deactivation so both panels continue showing beached particles.
the random generators are independent, so individual particles are not paired.
saved runs with particle JSONL and OpenDrift NetCDF can render clouds without
rerunning. JSONL is read one snapshot at a time and only sampled tracks are kept
in memory. the full unsampled JSONL file can be several hundred megabytes.
