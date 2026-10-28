# NorKyst generic validation

compare the current Proteus CLI regional backend with OpenDrift OceanDrift from
2026-10-27 00:00 UTC through 2026-10-29 00:00 UTC.

the default 8 by 8 release grid spans 2–3° E and 58–60° N in the open North Sea,
well west of the Norwegian coast. each release has one surface particle, zero radius, hourly RK4 steps,
no windage, no diffusion, and no Stokes drift. both models retain their land
handling. these offshore trajectories primarily test currents and projected
interpolation, not windage or weathering.

OpenDrift uses `reader_netCDF_CF_generic`, as its NorKyst examples do. the
NorKyst v3 product is a regular projected CF grid, not native sigma-level ROMS
output. the reader handles projection and eastward/northward vectors. the source
timeline is sorted and duplicate timestamps retain the last source record,
matching the cron. every hour of the requested interval must be available.

the suite caches the surface currents for the release region with a 100 km
margin in `runs/source_surface.nc`. reads use one timestamp per request and
preserve the native grid and source precision. subsequent runs reuse this
snapshot. delete it to refresh the source. wind is set to zero in OpenDrift
because windage is zero in both models.

```powershell
conda activate opendrift
python validation/norkyst_generic/run.py --limit 1
python validation/norkyst_generic/run.py
python validation/norkyst_generic/analyze.py
```

paths resolve from the scripts, so these commands can run from any directory.
`run.py` builds the release CLI by default. `--skip-build` reuses the binary.
`--dry-run` writes configs without network access. `--source` accepts a local
NorKyst NetCDF snapshot or an alternative OPeNDAP URL. `--tile-url` accepts a
complete regional tile root, including the `norkyst` prefix, or a local file URL.

outputs under ignored `runs/` include configs, metadata, CLI logs, the published
manifest, OpenDrift NetCDF and paired centroid JSON files. incomplete comparisons
are excluded. rerunning replaces outputs for the selected run indices. use a
fresh runs directory when changing the grid or source. analysis writes mean,
standard deviation and 10–90th percentiles to relative and absolute CSVs and a
two-panel plot. relative error is separation divided by cumulative OpenDrift path
length, not endpoint displacement.

analysis currently includes only hours 0 through 24, inclusive. the underlying
48-hour trajectories remain available for diagnosis. this restricts comparisons
to the first day while the next day's published tiles and source may represent
different forecast revisions.

the default source and tile URLs are mutable. source forecasts may differ from
the forecast used to publish the tiles. save matching source data and tiles for
strict reproducibility. Proteus uses float16 tile fields while OpenDrift reads
the source precision. no result should be interpreted as a clean interpolation
comparison unless the forcing snapshots match. the source archive and published
tiles must still contain october 27–29 when the suite is run.

## standalone worst case

`reproduce_worst.py` recreates the offshore suite's largest final absolute error,
`run_039`, released at 59.142857142857146° N, 3° E. it uses the cached source
snapshot and the same single-particle, hourly RK4 setup. run it with the
OpenDrift conda environment activated so plotting libraries are available.

```powershell
conda activate opendrift
python validation/norkyst_generic/reproduce_worst.py
```

outputs go to `runs/worst_case/`. `opendrift_map.png` and
`opendrift_animation.gif` use OpenDrift's native plotting methods.
`comparison.png` overlays the original CLI trajectory with six-hour labels.
`opendrift_hourly.csv` provides exact UTC positions. import
`website_config.geojson` into the website to reproduce the settings, then run
the simulation. this imports configuration only, not precomputed snapshots.
the website uses currently published tiles, which may have changed since the
CLI reference was generated.
