# NorKyst forcing draft

`norkyst_cron.py` reads the native 800 m polar stereographic grid from MET Norway.
It retains seven complete prior UTC dates, today, and every available forecast
hour. Each UTC day includes the following midnight when available. The final
forecast day may be partial. Missing hours in the selected window fail the run.
Older archive gaps outside that window are allowed.

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

## binary format

This is a new format, deliberately separate from the global longitude/latitude
tiles. The existing Proteus loader does not support it yet.

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
