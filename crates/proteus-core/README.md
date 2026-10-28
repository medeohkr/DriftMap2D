# Proteus core

`basemodel/` owns particles, release schedules, advection, diffusion, and landmask
interaction. `tracers/` supplies generic, oil, and leeway behaviour through
`Tracer` and `TracerKind`. `forcing/` supplies environmental values through
`Forcing` and `ForcingKind`.

## forcing

```text
forcing/
    mod.rs             shared trait, field selection, errors, and enum dispatch
    cache.rs           surface grids shared by both backends
    format.rs          bounded decompression and checked binary readers
    global/            global cache, tile format, and geographic grid lookup
    norkyst/           native cache, manifest, tile format, and projected lookup
    interpolation.rs   shared bilinear sampling and missing-value expansion
    projection.rs      polar stereographic coordinate conversion
```

construct the selected source in the CLI or WASM binding, then import the trait
to prepare and sample it

```rust,no_run
use proteus_core::{TileFetcher, forcing::{Forcing, ForcingKind, LoaderError}};

async fn sample<F: TileFetcher>(fetcher: F) -> Result<(), LoaderError> {
    let mut forcing = ForcingKind::norkyst("https://tiles.driftmap2d.com/norkyst", fetcher);
    forcing.prepare_forcing(vec![4.5, 60.0], 20261023, 0.5, 0.25).await?;
    let values = forcing.get_velocities_wind([(4.5, 60.0)].into_iter(), 20261023, 0.5);
    Ok(())
}
```

the date is UTC `YYYYMMDD`. query positions are `(longitude, latitude)`.
preparation positions are flattened longitude-latitude pairs. queries preserve
input order. preparation loads the spatial margin and dates required by the
whole integration step, including intermediate stages crossing midnight.

the simulation and diffusion accept `&impl Forcing`, so they do not depend on
the enum, fetcher, grid type, or tile representation. the enum delegates to the
selected backend without trait objects or boxed forcing futures. asynchronous
preparation deliberately has no `Send` requirement for browser fetches.

each backend owns its fetcher, base URL, cache, metadata, and decoding rules.
both decoders use `Tile::parse`. global tiles accept legacy, `PRT2`, and `PRT3`
payloads, optionally compressed with zstd. NorKyst uses its own `NKT1` format
and manifest. global decoded payloads are limited to 64 MiB and NorKyst to
8,000,000 bytes. the refactor does
not require new tiles or changes to the JavaScript constructor or CLI config.

the shared query methods return currents in degrees per second, wind in metres
per second, and temperature in kelvin. `sample_fields` returns these as
`[current_u, current_v, wind_u, wind_v, temperature]`. unrequested fields and
unavailable samples remain NaN. `cell_area_m2` returns square metres for
Smagorinsky diffusion.

interpolation preserves finite zeros, retries missing samples after at most ten
expansion rounds, and retains expanded values in the backend cache.

particles, releases, integrators, diffusion probes, and forcing queries are
strictly two-dimensional. the global decoder still consumes historical layer
headers, selects the layer closest to zero depth, and skips other layers without
retaining them. no vertical coordinates or layer strides enter the runtime.

`GridData<N>` groups dimensions, sample count, and field arrays in time, row,
column order. a global tile contains `currents: GridData<2>` and optional
`atmosphere: GridData<3>`. a NorKyst tile contains one `GridData<5>` and its
explicit timestamps. origins remain backend metadata. missing atmospheric
fields are represented by `None` rather than zero dimensions and empty arrays.

to add a source, implement `Forcing`, add a `ForcingKind` variant and its four
delegations, then select it in the binding. reuse interpolation where it fits
the new grid rather than forcing every model to use the same format or cache.

## checks

```sh
cargo test -p proteus-core -p proteus-cli
cargo check -p proteus-wasm --target wasm32-unknown-unknown
```

backend tests cover decoding, projection, missing data, halos, and midnight
transitions. shared contract tests compare each concrete backend with enum
dispatch using synthetic global tiles and the independent NorKyst fixtures.
