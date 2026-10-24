# NorKyst forcing draft

`norkyst_cron.py` reads the native 800 m polar stereographic grid from MET Norway.
It retains seven complete prior UTC dates, today, and every available forecast
hour. Each UTC day includes the following midnight when available. The final
forecast day may be partial. Missing hours in the selected window fail the run.
Older archive gaps outside that window are allowed.

THREDDS aggregation can return forecast records out of chronological order.
the cron orders timestamps while retaining their original data indices, keeping
the last source record when timestamps repeat. contiguous reads still use slices.
reordered days use explicit indices for every forcing field and coverage mask.
duplicates and backward transitions are logged, and genuine hourly gaps in the
selected window still fail the run.

The full grid currently has 2747 columns and 1148 rows, giving 23 by 10 tiles.
Each tile has a 120 by 120 core and one halo cell on each side. Tiles remain
122 by 122 at domain edges, with unavailable cells padded with NaN. Partial
edge cores are described by `core_shape`. The domain does not wrap.

Surface currents and wind remain eastward and northward in m/s. Surface potential
temperature is converted from Celsius to kelvin. Masks remain NaN and finite
zeros remain valid. Projection metadata supports converting longitude and
latitude into fractional native grid indices. No bathymetry, salinity, vertical
velocity, or diffusivity is downloaded.

## commands

```powershell
python norkyst_cron.py --inspect
python norkyst_cron.py --local-only
python norkyst_cron.py
python validation/norkyst/test_cron.py
```

Paths resolve relative to the script. Local output goes to `data/norkyst/`.
Publishing uses the existing `ACCOUNT_ID`, `ACCESS_KEY`, and `SECRET_KEY` secrets
and the `driftmap-tiles` bucket under `norkyst/YYYY/MM/DD/`. The separate workflow
runs daily at 10:00 UTC. Publishing is not enabled until the workflow is committed
and deployed to GitHub. A full first run backfills seven days and can transfer
many gigabytes. Subsequent runs reuse completed older hindcast days and refresh
yesterday onward. `--refresh-hindcast` forces their regeneration.

Day manifests are uploaded after all their tiles succeed. The root manifest is
uploaded after all days succeed, followed by expiry of older NorKyst dates.
The root manifest lists only the currently available window. Existing tile URLs
are mutable and use `Cache-Control: no-cache`.

## coverage

the cron records where both current components are finite at least once across
the published hindcast and forecast window. finite zeros remain valid. combining
the hourly masks avoids turning isolated missing samples into permanent gaps.
coverage allows one spatial expansion and then requires all four interpolation
corners to be supported.

the root manifest references a SHA-256 checked `coverage-<digest>.json` containing
row intervals on the native grid. the file is uploaded before the root manifest.
daily manifests retain their observed-current masks so later runs can reuse them.
the first run after this change reads currents for older cached days to add those
masks without regenerating their tiles.

unsupported ocean cells are identified with the GSHHG mask from `roaring_landmask`.
their polygons are drawn with diagonal hatching by helpers in
`web/src/lib/utils/forcingMask.ts` and `forcingCoverageLayers.ts`. the same native
coverage intervals reject release centers inside gaps such as the Gulf of Bothnia.
land checks retain their separate toast. temporary forcing availability at a
particular time remains separate from this structural coverage check.

the workflow installs `pyproj` and `roaring-landmask` for coverage generation.
run the updated cron before deploying the frontend changes. manifests without
coverage metadata retain the rectangular map outline but cannot pass the new
coverage validation.

## binary format

This is a new format, deliberately separate from the global longitude/latitude
tiles. Proteus reads it through `DataLoader::new_norkyst`. The wasm constructor
accepts a final `forcing_model` argument of `global` or `norkyst`, supplied by
the web forcing selector.

After zstd level 3 decompression, a tile contains:

1. four bytes `NKT1`
2. little endian uint32 JSON header length
3. UTF-8 JSON header
4. contiguous little endian float16 arrays in field, time, row, column order

Fields are `u`, `v`, `wind_u`, `wind_v`, `temperature_k`. The header specifies
tile index, actual core shape, padded origin in projected meters, timestamps in
Unix seconds, and array shape. Each day manifest lists its tiles, timestamps,
grid dimensions, axis origins and spacing, projection CF attributes and PROJ
string, units, surface depth, and attribution. Tile origins include the halo.
The root manifest links day manifests using paths relative to `norkyst/`.

Data attribution is MET Norway under CC BY 4.0, as stated by the source.

## sampling

The loader reads the root manifest once per simulation, projects particle
longitude and latitude into native x/y meters, and selects tiles using the
120-cell core. It reuses the bilinear interpolator and NaN expansion. Current
and wind components remain eastward and northward and need no grid rotation.
Particle integration remains geographic.

Sampling uses explicit timestamps for all five fields. Integration stages that
cross midnight preload the following day when available. Outside-domain starts,
missing tiles, invalid headers, and advances beyond forecast coverage return
errors. Native tile prefetch covers a 12 km margin around current positions.
Large steps that travel beyond that margin can still outrun the loaded tiles.
Smagorinsky diffusion uses native cell area corrected for projection scale.

The global loader retains its existing format and sampling. Wasm forcing times
now include the selected start time, including calendar changes at midnight.
Errors stop the web run and appear as a toast. Imported older scenarios default
to global forcing. The existing geographic tile preloader is skipped for NorKyst.

Rust tests use small fixtures generated by the Python cron encoder and projection
reference values from pyproj. Regenerate them with
`python validation/norkyst/generate_fixture.py` after installing pyproj. Run
`cargo test -p proteus-core` to check parsing, tile boundaries, temporal sampling,
midnight transitions, projection accuracy, and forecast limits. These tests do
not replace a full trajectory comparison using operational NorKyst tiles.
