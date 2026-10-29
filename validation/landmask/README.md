# polygon landmask validation and deployment

Proteus uses the existing GSHHG raster as a conservative candidate filter and
then checks full-resolution coastline polygons. polygon coordinates are not
simplified. every 10-degree tile has 40 by 40 quarter-degree buckets, limiting
the geometry tested for each candidate. polygon holes remain water and clipping
boundaries remain continuous. exact boundary points are classified as land.

raster indexing includes GSHHG's half-cell offset. raster pixels near a tile
edge may belong to the neighboring tile, while polygon lookup uses the tile
containing the geographic point. preload includes neighboring tiles and wraps
longitude at the dateline.

generate all 648 tiles using the same GSHHG WKB source as OpenDrift

```powershell
python scripts/build_landmask_polygons.py
```

requires `roaring-landmask`, `shapely>=2`, and `zstandard`. generated assets live
in ignored `data/landmask_polygons/`. selected tiles can be generated with
`--tile 18 14 --tile 18 15`, but those partial sets cannot be uploaded by the
temporary uploader. the manifest records the source WKB SHA-256 and individual
tile sizes and hashes.

the ignored temporary `upload_landmask_to_r2.py` prompts for the account ID,
access key ID and secret key. credentials are never written to disk. run

```powershell
python upload_landmask_to_r2.py
```

it verifies the complete manifest and tile hashes before prompting, then uploads
to `driftmap-tiles/roaring_landmask/polygons/`. existing raster tiles are unchanged.
the manifest is uploaded last. verify a public URL such as
`https://tiles.driftmap2d.com/roaring_landmask/polygons/landmask_018_014.bin.zst`
before deploying the rebuilt WASM engine. missing polygon tiles stop loading and
simulation instead of treating the region as water. remove the uploader after
use if it is no longer needed. it is excluded from Git explicitly.

the new format is zstd level 3 containing `LMP1` and a UTF-8 JSON payload with
version, tile index, cell size and row-major cells. each cell contains polygons,
each polygon contains an exterior ring followed by holes, and each ring contains
closed longitude-latitude pairs. the decoder validates identity, bucket bounds,
ring closure, coordinates and a 64 MiB decoded size limit.

tests and local reference comparison

```powershell
python validation/landmask/test_polygons.py
cargo test -p proteus-core landmask
python validation/landmask/compare.py
cargo run -p proteus-core --example check_landmask -- data/landmask_local data/landmask_local/reference.csv
```

the example compares raster and polygon files in a local tile root against a
CSV with `lon,lat,land` columns. CSV coordinates should be float32-rounded before
the OpenDrift reference classification. `--polygon-only` isolates the polygon
check by replacing the raster with positive candidates. local CLI configs can
set `landmask_url` to a `file:///` root containing the existing raster files and
a `polygons/` subdirectory.

the cross-language fixture tests holes, bucket edges, identities and truncated
compression. loader tests ensure polygons refine positive raster cells and
failed polygon downloads are not cached as successful loads. the generated
Norway tiles matched OpenDrift's combined classifier at 50,000 sampled points.

source attribution is GSHHG by Wessel and Smith, distributed through the
`roaring_landmask` package. use the same source WKB revision when regenerating
the raster and polygon assets together.
