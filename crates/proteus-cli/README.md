run Proteus with a JSON config

```sh
cargo run --release -p proteus-cli -- config.json
```

select native NorKyst forcing with `"forcing_model": "norkyst"` or override the config selection with `--forcing-model norkyst`. the default model remains `global`.

```json
{
  "start_date": "2026-10-23 12:00",
  "forcing_model": "norkyst",
  "tracer_type": "generic",
  "tracer_json": { "wind_factor": 0.02, "wind_deflection": 0 },
  "releases": [{
    "lon": 4.5, "lat": 60, "radius": 0,
    "schedule": [{ "amount": 1000, "duration": 0 }]
  }],
  "particles": 1000,
  "time_step_minutes": 15,
  "steps": 96,
  "output": "centroids.json"
}
```

choose a start date within the current forcing window. start times are UTC and include the configured hour and minute. native forcing errors, including positions outside the grid and advances past the last forecast sample, stop the run.

without `tile_url`, the forcing roots are `https://tiles.driftmap2d.com/tiles` for global and `https://tiles.driftmap2d.com/norkyst` for NorKyst. NorKyst reads `manifest.json` directly under that root and daily tiles under `YYYY/MM/DD/XXX_YYY.bin`.

an explicit `tile_url` always wins, including when the model is overridden on the command line. use a complete `https://` or `file://` forcing root, or a CDN prefix such as `norkyst` or `validation-tiles`. no extra model prefix is appended. a local native root must contain both the manifest and the daily tile directories.

generate local catalogs once in an OpenDrift environment

```sh
python scripts/build_catalogs.py
```

select an oil by ID and override only the properties you need

```json
"tracer_type": "oil",
"tracer_json": {
  "oil_id": "GN00006",
  "overrides": { "wind_factor": 0.02 }
}
```

the oil defaults are 3.5 percent windage, Samuels wind deflection, and the catalog's emulsification onset and water limits. wind_factor is a fraction, wind_deflection is degrees or null for Samuels, and bulltime is seconds.

select a leeway object by code or exact name

```json
"tracer_type": "sar",
"tracer_json": {
  "object_id": "PIW-1",
  "overrides": { "random_orientation": false }
}
```

SAR defaults are jibe_probability 0.04, random_orientation true, capsizing false, capsize_threshold 30, capsize_fraction 0.4 and capsize_sigma 5. probabilities and capsize_fraction use fractions from zero to one. coefficients retain the units used by OpenDrift's OBJECTPROP.DAT.

catalogs are discovered in `catalogs/` near the config or executable, independently of the working directory. set `"catalog_dir": "../catalogs"` to specify a directory relative to the config. use `--oil-source` and `--object-source` on the generator to select NOAA records or the existing frontend object catalog. generated JSON is ignored by Git and no catalog downloads happen during simulation.

full property objects and JSON strings remain supported for existing configs and validation. unknown IDs, selector mismatches, unknown override keys and invalid property types return errors before simulation starts.

## particle snapshots

centroid output remains unchanged. enable optional full-particle output with

```powershell
proteus config.json --particle-output particles.jsonl --snapshot-every 2
```

the same options can be set in config as `particle_output` and `snapshot_every`.
command-line options override config values. paths resolve relative to the
config directory. the interval must be positive and defaults to every step.
initial and final states are always included, even if the final step is not an
interval multiple.

each JSONL line contains `step`, UTC `time`, and `particles`. every released
particle has `id`, `lon`, `lat`, and `stranded`. IDs are stable zero-based release
indices, including stranded particles. unreleased particles appear only after
release. coordinates are geographic degrees and nonfinite coordinates encode
as JSON null. snapshots stream directly to disk without retaining full particle
history in memory. completed lines remain useful if a later step fails.

100,000 particles across 49 snapshots can produce several hundred megabytes of
JSONL. increase the snapshot interval when full temporal resolution is unneeded.

for local coastline validation, `landmask_url` can override the production
landmask root with a file URL. the root must contain raster tiles and matching
`polygons/landmask_XXX_YYY.bin.zst` files. see `validation/landmask/README.md` for
generation and upload instructions. both assets are required by the new loader.
