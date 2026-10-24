the catalog uses OpenDrift's ADIOS wrapper to generate GNOME pseudo-components and sample density and viscosity from -5 to 40 degrees Celsius at 0.5 degree intervals, matching `validation/oil/run.py`.

run these commands in an environment with OpenDrift and `adios_db` installed

```sh
python scripts/oil/test_build_oil_catalog.py
python scripts/oil/build_oil_catalog.py path/to/noaa-oil-data/data/oil -o data/oil_catalog.json
```

the builder writes JSON and a deterministic gzip copy. records rejected by OpenDrift or with invalid properties are listed under `skipped`. the catalog records the OpenDrift and ADIOS versions used.

`emulsion_water_fraction_max` is computed by GNOME during the build. `emulsion_water_fraction_max_by_temperature` stores Norwegian limits from OpenDrift's `max_water_fraction.json` as `[fraction, temperature_c]` pairs. Proteus interpolates those limits at the particle temperature and applies OpenOil's batch condition before replacing the GNOME maximum. a single point defines a constant limit. no water-fraction estimation is needed for catalog oils.

use `--water-fraction-limits path/to/max_water_fraction.json` to select another copy of OpenDrift's Norwegian limits. the spreadsheet helper `get_max_water_fraction_for_oils.py` is used by OpenDrift to maintain that JSON and is not needed to build this catalog.

the UI uses catalog onset properties by default. explicit time and evaporation overrides remain available and are preserved when importing scenarios. older tracer JSON without a water maximum still uses the legacy engine estimate.

upload `oil_catalog.json.gz` to the R2 key of the same name with content type `application/gzip` and no content encoding. the frontend decompresses the payload itself. retain the previous object as a backup before replacing it.
