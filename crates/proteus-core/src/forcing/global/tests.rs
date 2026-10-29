// tests.rs verifies tile parsing and interpolation behavior

use super::*;
use crate::fetch::BoxFuture;
use crate::forcing::{interpolation::interpolate_time_batch, GridData};
use half::f16;

fn sample(x: f32, y: f32) -> GridSample {
    GridSample {
        output_index: 0,
        x,
        y,
    }
}

#[test]
fn finite_values_and_zeros_do_not_expand_the_grid() {
    let mut grid = [2.0, -2.0, 2.0, -2.0];
    let mut scratch = Vec::new();
    let values = interpolate_batch(
        &mut grid,
        2,
        2,
        &[sample(0.5, 0.5), sample(0.0, 0.0)],
        &mut scratch,
    );
    assert_eq!(values, [0.0, 2.0]);
    assert_eq!(grid, [2.0, -2.0, 2.0, -2.0]);
    assert!(scratch.is_empty());
}

#[test]
fn dilation_uses_a_snapshot_and_the_mean_of_finite_neighbors() {
    let mut grid = [f32::NAN, -5.0, f32::INFINITY, -2.0];
    let values = interpolate_batch(&mut grid, 2, 2, &[sample(0.5, 0.5)], &mut Vec::new());
    assert_eq!(values, [-3.5]);
    assert_eq!(grid, [-3.5, -5.0, -3.5, -2.0]);
    let mut grid = [2.0, f32::NAN, f32::NAN, 0.0, f32::NAN, f32::NAN];
    expand_nan_neighbors(&mut grid, 3, 2, &mut Vec::new());
    assert_eq!(grid[1], 1.0);
    assert!(grid[2].is_nan());
    assert_eq!(grid[3], 0.0);
}

#[test]
fn dilation_is_symmetric_for_signed_fields_and_preserves_valid_cells() {
    let mut grid = [-6.0, f32::NAN, 2.0, 0.0, f32::NAN, 4.0];
    let mut reversed = grid.map(|value| -value);
    expand_nan_neighbors(&mut grid, 3, 2, &mut Vec::new());
    expand_nan_neighbors(&mut reversed, 3, 2, &mut Vec::new());
    assert_eq!(grid, [-6.0, 0.0, 2.0, 0.0, 0.0, 4.0]);
    assert_eq!(grid, reversed.map(|value| -value));
}

#[test]
fn dilation_accumulates_without_float32_overflow() {
    let mut grid = [f32::MAX, f32::NAN, f32::MAX, f32::MAX];
    expand_nan_neighbors(&mut grid, 2, 2, &mut Vec::new());
    assert_eq!(grid, [f32::MAX; 4]);
}

#[test]
fn batch_order_does_not_change_results_or_expanded_values() {
    let mut original = vec![f32::NAN; 8 * 2];
    original[0] = 2.0;
    original[8] = 2.0;
    original[7] = 8.0;
    original[15] = 8.0;
    let samples = [sample(0.25, 0.5), sample(3.5, 0.5), sample(6.75, 0.5)];
    let mut forward_grid = original.clone();
    let forward = interpolate_batch(&mut forward_grid, 8, 2, &samples, &mut Vec::new());
    let mut reverse_grid = original;
    let reversed: Vec<_> = samples.iter().copied().rev().collect();
    let reverse = interpolate_batch(&mut reverse_grid, 8, 2, &reversed, &mut Vec::new());
    assert_eq!(forward, reverse.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(forward_grid, reverse_grid);
    assert_eq!(forward, [2.0, 5.0, 8.0]);
}

#[test]
fn retries_stop_at_ten_and_later_batches_reuse_expansion() {
    let mut grid = vec![f32::NAN; 30 * 2];
    grid[0] = 4.0;
    grid[30] = 4.0;
    let samples = [sample(9.5, 0.5), sample(10.5, 0.5)];
    let values = interpolate_batch(&mut grid, 30, 2, &samples, &mut Vec::new());
    assert_eq!(values[0], 4.0);
    assert!(values[1].is_nan());
    assert!(grid[11].is_nan());
    let values = interpolate_batch(&mut grid, 30, 2, &samples, &mut Vec::new());
    assert_eq!(values, [4.0, 4.0]);
}

#[test]
fn missing_grids_stay_missing_and_retries_clamp_to_the_edge() {
    let mut missing = [f32::NAN; 4];
    let mut scratch = Vec::new();
    assert!(interpolate_batch(&mut missing, 2, 2, &[sample(0.5, 0.5)], &mut scratch)[0].is_nan());
    assert!(scratch.is_empty());
    let mut grid = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(
        interpolate_batch(&mut grid, 2, 2, &[sample(-1.0, 0.5)], &mut scratch),
        [2.0]
    );
}

#[test]
fn temporal_sampling_is_spatial_first() {
    let mut data = [f32::NAN, 2.0, 4.0, 6.0, 10.0, 10.0, 10.0, 10.0];
    let values = interpolate_time_batch(
        &mut data,
        2,
        2,
        2,
        0.5,
        &[sample(0.5, 0.5)],
        &mut Vec::new(),
    );
    assert_eq!(values, [7.0]);
    assert_eq!(data[0], 4.0);
    assert_eq!(&data[4..], &[10.0; 4]);
}

#[test]
fn legacy_layers_are_discarded_after_selecting_the_shallowest_surface() {
    let mut bytes = b"PRT3".to_vec();
    for origin in [0.0_f32; 4] {
        bytes.extend(origin.to_le_bytes());
    }
    for dimension in [2_u32, 2, 2, 2] {
        bytes.extend(dimension.to_le_bytes());
    }
    for depth in [10.0_f32, 0.5] {
        bytes.extend(depth.to_le_bytes());
    }
    for value in [99.0, 98.0, 1.0, 2.0, 97.0, 96.0, 3.0, 4.0] {
        for _ in 0..4 {
            bytes.extend(f16::from_f32(value).to_le_bytes());
        }
    }
    for dimension in [2_u32, 2, 1] {
        bytes.extend(dimension.to_le_bytes());
    }
    for value in [5.0, 6.0, 280.0] {
        for _ in 0..4 {
            bytes.extend(f16::from_f32(value).to_le_bytes());
        }
    }
    let mut tile = Tile::parse(&bytes).unwrap();
    assert_eq!(
        tile.currents.fields[0],
        [vec![1.0; 4], vec![3.0; 4]].concat()
    );
    assert_eq!(
        tile.currents.fields[1],
        [vec![2.0; 4], vec![4.0; 4]].concat()
    );
    assert_eq!(tile.atmosphere.as_ref().unwrap().fields[2], vec![280.0; 4]);
    assert_eq!(
        tile.currents
            .sample_field(0, 0.5, &[sample(0.5, 0.5)], &mut Vec::new()),
        [2.0]
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Tile::parse(&trailing).is_err());
    assert!(Tile::parse(b"PRT4").is_err());
    bytes[40..44].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(Tile::parse(&bytes).is_err());
}

#[test]
fn exact_and_final_timestamps_do_not_require_another_slice() {
    let mut data = [0.0, 0.0, 0.0, 0.0, f32::NAN, f32::NAN, f32::NAN, f32::NAN];
    assert_eq!(
        interpolate_time_batch(
            &mut data,
            2,
            2,
            2,
            0.0,
            &[sample(0.5, 0.5)],
            &mut Vec::new()
        ),
        [0.0]
    );
    let mut data = [f32::NAN, f32::NAN, f32::NAN, f32::NAN, 2.0, 2.0, 2.0, 2.0];
    assert_eq!(
        interpolate_time_batch(
            &mut data,
            2,
            2,
            2,
            24.0,
            &[sample(0.5, 0.5)],
            &mut Vec::new()
        ),
        [2.0]
    );
    assert!(data[0].is_nan());
}

#[test]
fn matches_scipy_reference_for_mixed_missing_values_and_edges() {
    let mut grid: Vec<_> = (0..30)
        .map(|i| {
            if i % 4 == 0 || i % 7 == 0 {
                f32::NAN
            } else {
                ((i * 7) % 19 - 9) as f32 / 4.0
            }
        })
        .collect();
    let samples = [
        sample(0.5, 0.5),
        sample(2.3, 1.7),
        sample(-0.4, 2.0),
        sample(5.2, 3.0),
        sample(5.0, 4.0),
        sample(0.0, 0.0),
        sample(4.7, 0.2),
        sample(2.1, 4.0),
        sample(3.0, 3.0),
    ];
    // expected values from the supplied algorithm using SciPy float32 arrays
    let expected = [
        -0.59375,
        -0.1681250035762787,
        -0.3125,
        0.0,
        1.0,
        -0.875,
        0.6759997606277466,
        0.6749998331069946,
        0.3125,
    ];
    let values = interpolate_batch(&mut grid, 6, 5, &samples, &mut Vec::new());
    for (value, expected) in values.iter().zip(expected) {
        assert!((value - expected).abs() < 1e-6);
    }
}

#[test]
fn padded_header_preserves_origins_nans_and_legacy_tiles() {
    let mut legacy = Vec::new();
    for value in [2u32, 2, 1] {
        legacy.extend(value.to_le_bytes());
    }
    legacy.extend(0.0f32.to_le_bytes());
    for _ in 0..25 {
        for value in [f32::NAN, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
            legacy.extend(f16::from_f32(value).to_bits().to_le_bytes());
        }
    }
    let old = Tile::parse(&legacy).unwrap();
    assert_eq!(old.current_origin, None);
    let mut padded = b"PRT2".to_vec();
    for value in [109.916664f32, 19.916666, 109.75, 19.75] {
        padded.extend(value.to_le_bytes());
    }
    padded.extend(legacy);
    let new = Tile::parse(&padded).unwrap();
    assert_eq!(new.current_origin, Some((109.916664, 19.916666)));
    assert_eq!(new.wind_origin, Some((109.75, 19.75)));
    assert!(new.currents.fields[0][0].is_nan());
    assert_eq!(new.currents.fields[1], old.currents.fields[1]);
    assert!(Tile::parse(b"PRT2").is_err());
}

#[test]
fn one_cell_halos_interpolate_continuously_across_tile_edges() {
    let mut loader = GlobalForcing::new("", -180.0, -80.0, UnusedFetcher);
    for lon_idx in [28, 29] {
        let origin = (
            -180.0 + lon_idx as f32 * 10.0 - 1.0 / 12.0,
            20.0 - 1.0 / 12.0,
        );
        let wind_origin = (-180.0 + lon_idx as f32 * 10.0 - 0.25, 19.75);
        let field = |width: usize, step: f32, origin: (f32, f32)| -> Vec<f32> {
            (0..width * width)
                .map(|i| {
                    (origin.0 + (i % width) as f32 * step) * 0.01
                        + (origin.1 + (i / width) as f32 * step) * 0.02
                })
                .collect()
        };
        let currents = field(122, 1.0 / 12.0, origin);
        let winds = field(42, 0.25, wind_origin);
        loader.cache.insert(
            TileKey {
                lon_idx,
                lat_idx: 10,
                day: 1,
            },
            RefCell::new(Tile {
                current_origin: Some(origin),
                wind_origin: Some(wind_origin),
                currents: GridData {
                    width: 122,
                    height: 122,
                    sample_count: 1,
                    fields: [currents.clone(), currents],
                },
                atmosphere: Some(GridData {
                    width: 42,
                    height: 42,
                    sample_count: 1,
                    fields: [winds.clone(), winds.clone(), winds],
                }),
            }),
        );
    }
    let positions = [(109.999, 20.01), (110.0, 20.01), (110.001, 20.01)];
    let values = loader.get_velocities_wind_slice(&positions, 1, 0.0);
    for ((u, v, wind_u, wind_v), (lon, lat)) in values.into_iter().zip(positions) {
        let expected = lon * 0.01 + lat * 0.02;
        assert!((u / meters_per_degree_lon(1.0, lat) - expected).abs() < 1e-6);
        assert!((v / meters_per_degree_lat(1.0, lat) - expected).abs() < 1e-6);
        assert!((wind_u - expected).abs() < 1e-6);
        assert!((wind_v - expected).abs() < 1e-6);
    }
}

#[test]
fn parses_python_zstd_tiles_without_changing_origins_nans_or_zeros() {
    // fixture compressed by Python zstandard at level 3
    let bytes: &[u8] = &[
        40, 181, 47, 253, 96, 180, 0, 181, 1, 0, 148, 2, 80, 82, 84, 50, 85, 213, 219, 66, 85, 85,
        159, 65, 0, 128, 219, 66, 0, 0, 158, 65, 2, 0, 0, 0, 1, 0, 126, 0, 0, 0, 192, 0, 66, 0, 68,
        0, 69, 0, 70, 0, 71, 3, 0, 125, 51, 85, 197, 121, 227, 213, 225, 1,
    ];
    let tile = Tile::parse(bytes).unwrap();
    assert_eq!(tile.current_origin, Some((109.916664, 19.916666)));
    assert_eq!(tile.wind_origin, Some((109.75, 19.75)));
    assert_eq!(tile.currents.sample_count, 25);
    assert!(tile.currents.fields[0][0].is_nan());
    assert_eq!(tile.currents.fields[0][1], 0.0);
    assert_eq!(tile.currents.fields[0][2], -2.0);
    assert_eq!(tile.currents.fields[1][0], 4.0);
    assert!(Tile::parse(&[0x28, 0xb5, 0x2f, 0xfd]).is_err());
    assert!(Tile::parse(&bytes[..bytes.len() - 3]).is_err());
}

#[test]
fn malformed_and_truncated_tiles_return_errors_without_panicking() {
    let mut bytes = Vec::new();
    for value in [2u32, 2, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0.0f32.to_le_bytes());
    bytes.extend(vec![0; 25 * 2 * 4 * 2]);
    let current_end = bytes.len();
    for value in [2u32, 2, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(vec![0; 3 * 4 * 2]);
    assert!(Tile::parse(&bytes).is_ok());
    for len in 0..bytes.len() {
        if len != current_end {
            assert!(
                Tile::parse(&bytes[..len]).is_err(),
                "accepted truncated length {len}"
            );
        }
    }
    for header in [[2u32, 2, u32::MAX], [u32::MAX, u32::MAX, 1], [0, 2, 1]] {
        let mut invalid: Vec<_> = header.into_iter().flat_map(u32::to_le_bytes).collect();
        invalid.extend(0.0f32.to_le_bytes());
        assert!(Tile::parse(&invalid).is_err());
    }
}

struct UnusedFetcher;
impl TileFetcher for UnusedFetcher {
    fn fetch_bytes<'a>(&'a self, _url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async { panic!("test tiles are already cached") })
    }
}

fn counted_global_tile(hours: u32, wind_steps: u32) -> Vec<u8> {
    let mut bytes = b"PRT3".to_vec();
    for _ in 0..4 {
        bytes.extend(0.0f32.to_le_bytes());
    }
    for value in [hours, 2, 2, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0.0f32.to_le_bytes());
    for hour in 0..hours {
        for value in [hour as f32, hour as f32 + 1.0] {
            for _ in 0..4 {
                bytes.extend(f16::from_f32(value).to_bits().to_le_bytes());
            }
        }
    }
    for value in [2u32, 2, wind_steps] {
        bytes.extend(value.to_le_bytes());
    }
    for step in 0..wind_steps {
        for value in [
            100.0 + step as f32,
            200.0 + step as f32,
            280.0 + step as f32,
        ] {
            for _ in 0..4 {
                bytes.extend(f16::from_f32(value).to_bits().to_le_bytes());
            }
        }
    }
    bytes
}

#[test]
fn counted_global_tiles_keep_current_and_wind_payloads_aligned() {
    for hours in [2, 24, 25] {
        let tile = Tile::parse(&counted_global_tile(hours, 5)).unwrap();
        assert_eq!(tile.currents.sample_count, hours as usize);
        assert_eq!(tile.currents.fields[0].len(), hours as usize * 4);
        assert_eq!(tile.currents.fields[0][0], 0.0);
        assert_eq!(*tile.currents.fields[0].last().unwrap(), (hours - 1) as f32);
        assert_eq!(*tile.currents.fields[1].last().unwrap(), hours as f32);
        assert_eq!(tile.atmosphere.as_ref().unwrap().fields[0][0], 100.0);
        assert_eq!(
            *tile.atmosphere.as_ref().unwrap().fields[1].last().unwrap(),
            204.0
        );
        assert_eq!(
            *tile.atmosphere.as_ref().unwrap().fields[2].last().unwrap(),
            284.0
        );
    }
}

#[test]
fn counted_global_tiles_reject_invalid_counts_and_truncation() {
    let bytes = counted_global_tile(24, 5);
    let currents_end = 40 + 24 * 4 * 4;
    for length in 0..bytes.len() {
        if length != currents_end {
            assert!(
                Tile::parse(&bytes[..length]).is_err(),
                "accepted truncated length {length}"
            );
        }
    }
    for hours in [0u32, 1, 26, u32::MAX] {
        let mut invalid = bytes.clone();
        invalid[20..24].copy_from_slice(&hours.to_le_bytes());
        assert!(Tile::parse(&invalid).is_err());
    }
}

#[test]
fn partial_global_tiles_stop_at_the_last_current_or_wind_sample() {
    let mut loader = GlobalForcing::new("", 0.0, 0.0, UnusedFetcher);
    let key = TileKey {
        lon_idx: 0,
        lat_idx: 0,
        day: 20261023,
    };
    loader.cache.insert(
        key.clone(),
        RefCell::new(Tile::parse(&counted_global_tile(24, 5)).unwrap()),
    );
    assert!(loader.validate_forcing_interval(key.day, 22.5, 0.5).is_ok());
    assert!(loader.validate_forcing_interval(key.day, 23.0, 0.0).is_ok());
    assert!(
        loader.get_velocities_wind_slice(&[(1.0 / 24.0, 1.0 / 24.0)], key.day, 23.000_002)[0]
            .0
            .is_finite()
    );
    assert!(loader
        .validate_forcing_interval(key.day, 22.5, 0.6)
        .is_err());
    assert!(loader
        .validate_forcing_interval(key.day, 23.0, 1.0)
        .is_err());
    let positions = [(1.0 / 24.0, 1.0 / 24.0)];
    assert!(
        loader.get_velocities_wind_slice(&positions, key.day, f32::NAN)[0]
            .0
            .is_nan()
    );
    let value = loader.get_velocities_wind_slice(&positions, key.day, 22.5)[0];
    assert!((value.0 / meters_per_degree_lon(1.0, positions[0].1) - 22.5).abs() < 1e-5);
    assert!(
        loader.get_velocities_wind_slice(&positions, key.day, 23.1)[0]
            .0
            .is_nan()
    );
    loader.cache.insert(
        key.clone(),
        RefCell::new(Tile::parse(&counted_global_tile(24, 4)).unwrap()),
    );
    assert!(loader.validate_forcing_interval(key.day, 17.5, 0.5).is_ok());
    assert!(loader
        .validate_forcing_interval(key.day, 18.0, 0.1)
        .is_err());
    let value = loader.get_velocities_wind_slice(&positions, key.day, 18.1)[0];
    assert!(value.0.is_finite());
    assert!(value.2.is_nan());
}

struct PartialForecastFetcher;
impl TileFetcher for PartialForecastFetcher {
    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move {
            if url.contains("/2026/10/23/") {
                Ok(counted_global_tile(25, 5))
            } else if url.contains("/2026/10/24/") {
                Ok(counted_global_tile(24, 5))
            } else {
                Err(FetchError::Http(404))
            }
        })
    }
}

#[tokio::test]
async fn global_midnight_advances_load_both_dates_and_sample_the_next_tile() {
    let mut loader = GlobalForcing::new(
        "https://example.test/tiles",
        0.0,
        0.0,
        PartialForecastFetcher,
    );
    let positions = vec![1.0 / 24.0, 1.0 / 24.0];
    loader
        .prepare_forcing(positions.clone(), 20261023, 23.75, 0.250_002)
        .await
        .unwrap();
    assert_eq!(loader.cache.len(), 1);
    loader
        .prepare_forcing(positions.clone(), 20261023, 23.75, 0.5)
        .await
        .unwrap();
    assert_eq!(loader.cache.len(), 2);
    let point = [(positions[0], positions[1])];
    let value = loader.get_velocities_wind_slice(&point, 20261023, 24.25)[0];
    assert!((value.0 / meters_per_degree_lon(1.0, positions[1]) - 0.25).abs() < 1e-5);
    assert!(loader
        .validate_forcing_interval(20261023, 23.75, 0.5)
        .is_ok());
    loader
        .prepare_forcing(positions, 20261024, 22.5, 0.5)
        .await
        .unwrap();
    assert_eq!(loader.cache.len(), 1);
    assert!(loader
        .validate_forcing_interval(20261024, 23.0, 0.1)
        .is_err());
    assert_eq!(next_date(20261231).unwrap(), 20270101);
}

#[test]
fn loader_batches_currents_wind_and_sst_into_output_order() {
    let mut loader = GlobalForcing::new("", 0.0, 0.0, UnusedFetcher);
    let field = || vec![f32::NAN, 2.0, 4.0, 6.0, 10.0, 10.0, 10.0, 10.0];
    for lon_idx in [0, 1] {
        loader.cache.insert(
            TileKey {
                lon_idx,
                lat_idx: 0,
                day: 1,
            },
            RefCell::new(Tile {
                current_origin: None,
                wind_origin: None,
                currents: GridData {
                    width: 2,
                    height: 2,
                    sample_count: 2,
                    fields: [field(), field()],
                },
                atmosphere: Some(GridData {
                    width: 2,
                    height: 2,
                    sample_count: 2,
                    fields: [field(), field(), field()],
                }),
            }),
        );
    }
    let positions = [(10.0 + 1.0 / 24.0, 1.0 / 24.0), (1.0 / 24.0, 1.0 / 24.0)];
    let values = loader.sample_groups(
        loader.group_positions_slice(&positions, 1),
        2,
        0.5,
        true,
        true,
        true,
    );
    let velocities = loader.get_velocities_wind(positions.iter().copied(), 1, 0.5);
    let wind_sst = loader.get_wind_sst(positions.iter().copied(), 1, 0.5);
    assert_eq!(
        velocities,
        loader.get_velocities_wind_slice(&positions, 1, 0.5)
    );
    for (i, value) in values.into_iter().enumerate() {
        assert_eq!(velocities[i], (value[0], value[1], value[2], value[3]));
        assert_eq!(wind_sst[i], (value[2], value[3], value[4]));
        assert!((value[0] - meters_per_degree_lon(7.0, positions[0].1)).abs() < 1e-8);
        assert!((value[1] - meters_per_degree_lat(7.0, positions[0].1)).abs() < 1e-8);
        assert!((value[2] - value[3]).abs() < 1e-6);
        assert_eq!(value[3], value[4]);
    }
    for tile in loader.cache.values() {
        let tile = tile.borrow();
        for data in [
            &tile.currents.fields[0],
            &tile.currents.fields[1],
            &tile.atmosphere.as_ref().unwrap().fields[0],
            &tile.atmosphere.as_ref().unwrap().fields[1],
            &tile.atmosphere.as_ref().unwrap().fields[2],
        ] {
            assert_eq!(data[0], 4.0);
        }
    }
}
