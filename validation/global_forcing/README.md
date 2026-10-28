# global forcing tile validation

`global_cron.py` is checked hourly at minute 17 UTC. the workflow first reads
CMEMS catalogue metadata and the published R2 manifest. expensive dependencies
and tile generation are skipped unless a new completed update is available.
`arco_updating_start_date` blocks in-progress updates. `arco_updated_date` must
belong to the current UTC date, and its version and timestamp must differ from
the published `source_update`. this is a publication marker, not a model
forecast reference time. the global manifest also records the ECMWF reference
time separately. data reads reject in-progress CMEMS updates.

workflow concurrency prevents overlapping generators. scheduled generation
passes `--check-for-update` to recheck before downloads or cleanup. omit that
flag when manually regenerating the latest completed update. source failures
remain workflow failures, while stale and unchanged sources are normal skips.

run `python -m unittest discover -s validation/global_forcing` to check hourly
sample validation and the actual cron writer using synthetic data and a fake
uploader. no downloads or R2 writes occur.

new global tiles use `PRT3`, followed by four little-endian float32 grid origins
and a uint32 current sample count. the remaining grid and float16 field layout
matches `PRT2`. hourly samples start at midnight. full days have 25 samples and
the final day may have 2–24 samples. a 24-sample tile ends at 23:00 UTC.

the loader accepts legacy unversioned and `PRT2` tiles with their original
25-sample assumption. it rejects advances beyond the last available current or
wind sample and loads both dates for integration stages crossing midnight.

deploy the updated CLI and WASM reader before publishing `PRT3` tiles. existing
tiles remain readable without regeneration.

the cron reads the published SMOC timeline and requests six-hourly ECMWF samples
through the current endpoint, including the next wind sample when needed for
interpolation. ECMWF requests stop at the 00 UTC product's 360-hour horizon.
tiles extend through the last hour covered by both products, including a partial
final day. missing source hours or failed tile uploads stop publication.

`tiles/manifest.json` advertises the generated timestamps after all uploads
succeed. the web time picker uses its endpoint and retains the seven-day
hindcast window. until the first manifest is published, a 404 uses the previous
fixed forecast range.

run `node --test validation/global_forcing/forcing.test.mjs` to check published
availability, partial final days, and compatibility with older deployments.

run `node --test validation/global_forcing/simulationTime.test.mjs` to check
shared interval clipping, imports and resets, stale availability responses,
and preservation of the Norway release location when returning to global.
