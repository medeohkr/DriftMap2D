run the catalog-based oil comparison in an OpenDrift environment

```sh
python scripts/build_catalogs.py
cargo build --release -p proteus-cli
python validation/oil/run.py
python validation/oil/analyze.py
```

the default compares `GN00007` (Generic Medium Crude) over 64 release locations for 48 hours. Proteus receives only an oil ID and windage overrides and reads the local catalog through the CLI resolver. OpenDrift computes its own properties from the raw NOAA record identified by the catalog's `source_file`.

using the same raw record matters because OpenDrift's bundled database can contain an older edition under the same oil ID. catalogs built from OpenDrift's bundled archive use that bundled reference directly. `--reference-oil-file` selects another raw NOAA JSON file and must match the catalog oil ID.

`--catalog-dir` selects another local catalog directory. `--oil-type` selects another oil name present in both the catalog and OpenDrift. `--runs-dir` selects another output directory. `--check-config` prints the OpenDrift configuration and CLI catalog selection without running the simulations. defaults are relative to the suite and project directories, so the runner works from any directory.

the forcing inputs are the SMOC NetCDF and ECMWF GRIB files in `validation/forcing/`. `--tile-url` selects the matching CLI tiles and defaults to the `validation-tiles` CDN prefix.

`reference.py` contains the independent OpenDrift property extractor used by catalog regression tests. the simulation runner does not pass those extracted properties to Proteus.
