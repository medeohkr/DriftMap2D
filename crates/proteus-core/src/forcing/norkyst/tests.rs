// tests.rs checks projected coordinates and native forcing samples

use super::*;
use crate::fetch::BoxFuture;

const MANIFEST: &[u8] = include_bytes!("fixtures/manifest.json");
const LEFT: &[u8] = include_bytes!("fixtures/004_005.bin");
const RIGHT: &[u8] = include_bytes!("fixtures/005_005.bin");

#[tokio::test]
async fn preload_envelopes_cover_every_particle_margin_and_only_available_dates() {
    let positions: Vec<_> = reference().iter().map(point).collect();
    let flat: Vec<f32> = positions.iter().flat_map(|&(x, y)| [x, y]).collect();
    let mut loader = NorkystForcing::new("fixture/", MockFetcher);
    loader.plan_preload(&flat, 20261023).await.unwrap();
    let grid = &loader.manifest.as_ref().unwrap().grid;
    let urls = loader.preload_urls();
    assert!(!urls.is_empty());
    assert!(loader.tiles.is_empty());
    assert!(urls
        .iter()
        .all(|url| url.starts_with("fixture/2026/10/23/")));
    for &(lon, lat) in &positions {
        let (x, y) = grid.position(normalize_lon(lon), lat).unwrap();
        let left = (x - 60.0).max(0.0) as usize / 120;
        let right = (x + 60.0).min((grid.grid_shape[1] - 1) as f64) as usize / 120;
        let top = (y - 60.0).max(0.0) as usize / 120;
        let bottom = (y + 60.0).min((grid.grid_shape[0] - 1) as f64) as usize / 120;
        for lon_idx in left..=right {
            for lat_idx in top..=bottom {
                assert!(urls.contains(&tile_url(
                    "fixture",
                    &TileKey {
                        lon_idx,
                        lat_idx,
                        day: 20261023
                    }
                )));
            }
        }
    }
    loader
        .prepare_forcing(
            positions.iter().flat_map(|&(x, y)| [x, y]).collect(),
            20261023,
            0.0,
            0.0,
        )
        .await
        .unwrap();
    let urls = loader.preload_urls();
    for key in loader.tiles.keys() {
        assert!(!urls.contains(&tile_url("fixture", key)));
    }
    loader.plan_preload(&[], 20261023).await.unwrap();
    assert!(loader.preload_urls().is_empty());
    assert!(loader.prepared.is_empty());
}

#[tokio::test]
async fn projected_samples_stay_correct_after_reordering_moving_and_shrinking_batches() {
    let positions: Vec<_> = reference().iter().map(point).collect();
    let mut loader = NorkystForcing::new("fixture", MockFetcher);
    loader
        .prepare_forcing(
            positions.iter().flat_map(|&(x, y)| [x, y]).collect(),
            20261023,
            0.5,
            0.0,
        )
        .await
        .unwrap();
    let expected =
        loader.sample_fields(positions.iter().copied(), 20261023, 0.5, ForcingFields::All);
    let reversed = loader.sample_fields(
        positions.iter().copied().rev(),
        20261023,
        0.5,
        ForcingFields::All,
    );
    assert_eq!(expected, reversed.into_iter().rev().collect::<Vec<_>>());
    let moved: Vec<_> = positions.iter().map(|&(x, y)| (x + 0.0001, y)).collect();
    let fresh = {
        let mut other = NorkystForcing::new("fixture", MockFetcher);
        other
            .prepare_forcing(
                moved.iter().flat_map(|&(x, y)| [x, y]).collect(),
                20261023,
                0.5,
                0.0,
            )
            .await
            .unwrap();
        other.sample_fields(moved.iter().copied(), 20261023, 0.5, ForcingFields::All)
    };
    assert_eq!(
        loader.sample_fields(moved.iter().copied(), 20261023, 0.5, ForcingFields::All),
        fresh
    );
    loader
        .prepare_forcing(vec![positions[0].0, positions[0].1], 20261023, 0.5, 0.0)
        .await
        .unwrap();
    assert_eq!(loader.prepared.len(), 1);
    assert_eq!(
        loader.sample_fields(positions.iter().copied(), 20261023, 0.5, ForcingFields::All),
        expected
    );
}

#[tokio::test]
async fn combined_samples_reuse_storage_and_clear_unrequested_fields() {
    let points: Vec<_> = reference().iter().take(3).map(point).collect();
    let mut loader = NorkystForcing::new("fixture", MockFetcher);
    loader
        .prepare_forcing(
            points.iter().flat_map(|p| [p.0, p.1]).collect(),
            20261023,
            0.5,
            0.0,
        )
        .await
        .unwrap();
    let velocities = loader.get_velocities_wind(points.iter().copied(), 20261023, 0.5);
    let temperature = loader.get_wind_sst(points.iter().copied(), 20261023, 0.5);
    let mut output = Vec::new();
    loader.sample_fields_into(
        points.iter().copied(),
        20261023,
        0.5,
        ForcingFields::All,
        &mut output,
    );
    for (i, v) in output.iter().enumerate() {
        assert_eq!((v[0], v[1], v[2], v[3]), velocities[i]);
        assert_eq!(v[4], temperature[i].2);
    }
    let pointers = {
        let scratch = loader.sampling.borrow();
        (
            output.as_ptr(),
            scratch.samples.as_ptr(),
            scratch.values.as_ptr(),
        )
    };
    loader.sample_fields_into(
        points.iter().copied(),
        20261023,
        0.5,
        ForcingFields::All,
        &mut output,
    );
    {
        let scratch = loader.sampling.borrow();
        assert_eq!(
            pointers,
            (
                output.as_ptr(),
                scratch.samples.as_ptr(),
                scratch.values.as_ptr()
            )
        );
    }
    loader.sample_fields_into(
        points[..1].iter().copied(),
        20261023,
        0.0,
        ForcingFields::CurrentsWind,
        &mut output,
    );
    assert_eq!(output.len(), 1);
    assert!(output[0][4].is_nan());
    loader.sample_fields_into(
        points.iter().copied(),
        19990101,
        0.0,
        ForcingFields::All,
        &mut output,
    );
    assert!(output.iter().flatten().all(|v| v.is_nan()));
}

struct MockFetcher;

impl TileFetcher for MockFetcher {
    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        let bytes = if url.ends_with("manifest.json") {
            MANIFEST
        } else if url.ends_with("004_005.bin") {
            LEFT
        } else if url.ends_with("005_005.bin") {
            RIGHT
        } else {
            return None;
        };
        Some(bytes.to_vec())
    }

    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move { Err(FetchError::Network(format!("unexpected request {url}"))) })
    }
}

fn reference() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("fixtures/reference.json")).unwrap()
}

fn point(reference: &serde_json::Value) -> (f32, f32) {
    (
        reference["lon"].as_f64().unwrap() as f32,
        reference["lat"].as_f64().unwrap() as f32,
    )
}

#[test]
fn projection_matches_independent_pyproj_coordinates() {
    let manifest = Manifest::parse(MANIFEST).unwrap();
    for (lon, lat, x, y) in [
        (5., 60., 472039.21688481513, 493645.21686379425),
        (10., 65., 1078162.6189692272, 521838.0112307118),
        (20., 70., 1757126.894991635, 491774.412304197),
        (70., 90., 3369600., 1844800.),
    ] {
        let projected = manifest.grid.projection.project(lon, lat).unwrap();
        assert!((projected.0 - x).abs() < 1e-6);
        assert!((projected.1 - y).abs() < 1e-6);
    }
    assert!(manifest.grid.position(0., 0.).is_none());
    assert!(manifest.grid.projection.project(0., -90.).is_none());
    for (lat, scale) in [
        (60., 0.9999999999574098),
        (65., 0.9788991056589346),
        (70., 0.9620675280136269),
    ] {
        assert!((manifest.grid.projection.scale(lat) - scale).abs() < 1e-8);
    }
}

#[tokio::test]
async fn integration_stages_cross_midnight_using_the_next_day_tile() {
    struct MidnightFetcher;
    impl TileFetcher for MidnightFetcher {
        fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
            if url.ends_with("manifest.json") {
                let mut value: serde_json::Value = serde_json::from_slice(MANIFEST).unwrap();
                let start = value["days"][0]["timestamps"][0].as_i64().unwrap();
                value["days"] = serde_json::json!([
                    {"date":"2026-10-23", "timestamps":(0..25).map(|i| start + i * 3600).collect::<Vec<_>>()},
                    {"date":"2026-10-24", "timestamps":(24..27).map(|i| start + i * 3600).collect::<Vec<_>>()}
                ]);
                return Some(serde_json::to_vec(&value).unwrap());
            }
            let next = url.contains("/2026/10/24/");
            let bytes: &[u8] = match (url.ends_with("004_005.bin"), next) {
                (true, false) => include_bytes!("fixtures/004_005_full.bin"),
                (true, true) => include_bytes!("fixtures/004_005_next.bin"),
                (false, false) => include_bytes!("fixtures/005_005_full.bin"),
                (false, true) => include_bytes!("fixtures/005_005_next.bin"),
            };
            Some(bytes.to_vec())
        }
        fn fetch_bytes<'a>(&'a self, _url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
            Box::pin(async { panic!("fixtures must be preloaded") })
        }
    }
    let mut loader = NorkystForcing::new("fixture", MidnightFetcher);
    let point = point(&reference()[0]);
    loader
        .prepare_forcing(vec![point.0, point.1], 20261023, 23.75, 0.5)
        .await
        .unwrap();
    for (hour, expected) in [(23.75, 303.75), (24.0, 304.0), (24.25, 304.25)] {
        assert_eq!(
            loader.get_wind_sst([point].into_iter(), 20261023, hour)[0].2,
            expected
        );
    }
    assert!(loader.cell_area_m2(point.1) < 700_000.0);
    assert!(loader
        .validate_forcing_interval(20261024, 1.5, 1.0)
        .is_err());
}

#[test]
fn python_tiles_preserve_fields_and_reject_invalid_headers() {
    let manifest = Manifest::parse(MANIFEST).unwrap();
    let times = &manifest.days[0].timestamps;
    let tile = Tile::parse(LEFT, &manifest.grid, [4, 5], times).unwrap();
    assert_eq!(tile.grid.fields[2][0], 0.0);
    assert_eq!(tile.grid.fields[3][0], 4.0);
    assert_eq!(tile.grid.fields[4][0], 280.0);
    assert!(tile.grid.fields[0][61 * 122 + 51].is_nan());
    assert!(Tile::parse(LEFT, &manifest.grid, [5, 5], times).is_err());
    assert!(Tile::parse(LEFT, &manifest.grid, [4, 5], &times[..2]).is_err());
    assert!(Tile::parse(&LEFT[..10], &manifest.grid, [4, 5], times).is_err());
    assert!(Tile::parse(b"PRT2", &manifest.grid, [4, 5], times).is_err());
}

#[tokio::test]
async fn projected_lookup_samples_both_sides_of_a_tile_boundary() {
    let mut loader = NorkystForcing::new("fixture", MockFetcher);
    let reference = reference();
    let positions: Vec<_> = reference.iter().map(point).collect();
    let flat = positions.iter().flat_map(|p| [p.0, p.1]).collect();
    loader
        .prepare_forcing(flat, 20261023, 0.0, 0.0)
        .await
        .unwrap();
    assert_eq!(
        loader
            .key(positions[0].0, positions[0].1, 20261023)
            .unwrap()
            .lon_idx,
        4
    );
    assert_eq!(
        loader
            .key(positions[1].0, positions[1].1, 20261023)
            .unwrap()
            .lon_idx,
        5
    );
    let current = loader.get_velocities_wind(positions.iter().copied(), 20261023, 0.5);
    let wind_sst = loader.get_wind_sst(positions.iter().copied(), 20261023, 0.5);
    for (i, value) in current.iter().enumerate() {
        let expected = 1.0
            + reference[i]["x"].as_f64().unwrap() * 0.001
            + reference[i]["y"].as_f64().unwrap() * 0.002
            + 0.25;
        let u = value.0 / meters_per_degree_lon(1.0, positions[i].1);
        assert!((u as f64 - expected).abs() < 0.002);
        assert!((value.1 / meters_per_degree_lat(1.0, positions[i].1) + 1.0).abs() < 1e-6);
        assert_eq!((value.2, value.3), (0.0, 4.0));
        assert_eq!(wind_sst[i], (0.0, 4.0, 280.5));
    }
    let end = loader.get_wind_sst(positions.iter().copied(), 20261023, 2.0);
    assert_eq!(end[0].2, 282.0);
    let missing = loader.get_wind_sst(positions.iter().copied(), 20261023, 2.5);
    assert!(missing[0].2.is_nan());
    loader
        .validate_forcing_interval(20261023, 1.5, 0.5)
        .unwrap();
    assert!(loader
        .validate_forcing_interval(20261023, 1.5, 0.6)
        .is_err());
    assert!(loader.validate_forcing_interval(20261024, 0., 1.).is_err());
}

#[tokio::test]
async fn invalid_domains_and_dates_return_errors() {
    let mut loader = NorkystForcing::new("fixture", MockFetcher);
    assert!(loader
        .prepare_forcing(vec![0., 0.], 20261023, 0.0, 0.0)
        .await
        .is_err());
    assert!(loader
        .prepare_forcing(vec![5., 60.], 20261024, 0.0, 0.0)
        .await
        .is_err());
    assert!(loader
        .prepare_forcing(vec![5.], 20261023, 0.0, 0.0)
        .await
        .is_err());
}

#[test]
fn metadata_rejects_unsupported_geometry_and_invalid_timestamps() {
    for (path, value) in [
        ("vector_basis", serde_json::json!("grid")),
        ("grid_spacing_m", serde_json::json!([1000, 1000])),
        ("dtype", serde_json::json!("<f4")),
    ] {
        let mut manifest: serde_json::Value = serde_json::from_slice(MANIFEST).unwrap();
        manifest["grid"][path] = value;
        assert!(Manifest::parse(&serde_json::to_vec(&manifest).unwrap()).is_err());
    }
    let mut manifest: serde_json::Value = serde_json::from_slice(MANIFEST).unwrap();
    manifest["days"][0]["timestamps"][1] = serde_json::json!(0);
    assert!(Manifest::parse(&serde_json::to_vec(&manifest).unwrap()).is_err());
}
