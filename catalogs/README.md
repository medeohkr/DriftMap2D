this directory holds generated catalogs for the Proteus CLI. JSON and gzip files are ignored by Git.

generate both catalogs locally in an OpenDrift environment

```sh
python scripts/build_catalogs.py
```

the default sources are OpenDrift's bundled ADIOS archive and `OBJECTPROP.DAT`. use `--oil-source path/to/noaa-oil-data/data/oil` for NOAA source records. use `--object-source web/src/lib/assets/object_catalog.json` to reuse the existing frontend asset instead. paths explicitly supplied to the generator are relative to the working directory. its default output directory is always this project directory.

object IDs generated from `OBJECTPROP.DAT` are stable codes such as `PIW-1`. catalogs generated from the asset retain its existing object names as IDs.

the CLI finds catalogs near its config file or executable, so it can run from any working directory. set `catalog_dir` in the simulation config to use another directory. that path is relative to the config file.

the oil builder also writes `oil_catalog.json.gz` for publication to R2. the CLI reads the uncompressed JSON and never downloads catalogs at runtime.
