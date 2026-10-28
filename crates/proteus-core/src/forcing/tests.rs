// tests.rs checks the shared contract and enum dispatch against both backends

use super::*;
use crate::fetch::{BoxFuture, FetchError};
use half::f16;

struct FixtureFetcher {
    native: bool,
}

impl TileFetcher for FixtureFetcher {
    fn preloaded(&self, url: &str) -> Option<Vec<u8>> {
        if self.native {
            return Some(if url.ends_with("manifest.json") {
                include_bytes!("norkyst/fixtures/manifest.json").to_vec()
            } else if url.ends_with("004_005.bin") {
                include_bytes!("norkyst/fixtures/004_005.bin").to_vec()
            } else if url.ends_with("005_005.bin") {
                include_bytes!("norkyst/fixtures/005_005.bin").to_vec()
            } else {
                return None;
            });
        }
        let mut bytes = b"PRT3".to_vec();
        for origin in [0.0_f32; 4] {
            bytes.extend(origin.to_le_bytes());
        }
        for dimension in [2_u32, 2, 2, 1] {
            bytes.extend(dimension.to_le_bytes());
        }
        bytes.extend(0.0_f32.to_le_bytes());
        for value in [1.0, 2.0, 3.0, 4.0] {
            for _ in 0..4 {
                bytes.extend(f16::from_f32(value).to_le_bytes());
            }
        }
        for dimension in [2_u32, 2, 2] {
            bytes.extend(dimension.to_le_bytes());
        }
        for value in [0.0, 4.0, 280.0, 6.0, 10.0, 286.0] {
            for _ in 0..4 {
                bytes.extend(f16::from_f32(value).to_le_bytes());
            }
        }
        Some(bytes)
    }

    fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move { Err(FetchError::Network(format!("unexpected request {url}"))) })
    }
}

async fn sample_contract(
    source: &mut impl Forcing,
    positions: &[(f32, f32)],
) -> (Vec<(f32, f32, f32, f32)>, Vec<(f32, f32, f32)>, f32) {
    let flat = positions.iter().flat_map(|p| [p.0, p.1]).collect();
    source
        .prepare_forcing(flat, 20261023, 0.5, 0.25)
        .await
        .unwrap();
    source
        .validate_forcing_interval(20261023, 0.5, 0.25)
        .unwrap();
    let velocities = source.get_velocities_wind(positions.iter().copied(), 20261023, 0.5);
    assert_eq!(
        velocities,
        source.get_velocities_wind_slice(positions, 20261023, 0.5)
    );
    let currents = source.get_velocities(positions, 20261023, 0.5);
    for (current, velocity) in currents.iter().zip(&velocities) {
        assert_eq!(*current, (velocity.0, velocity.1));
        assert!([velocity.0, velocity.1, velocity.2, velocity.3]
            .iter()
            .all(|v| v.is_finite()));
    }
    assert!(source.get_velocities(&[], 20261023, 0.5).is_empty());
    assert!(source
        .validate_forcing_interval(20261023, f32::NAN, 0.25)
        .is_err());
    (
        velocities,
        source.get_wind_sst(positions.iter().copied(), 20261023, 0.5),
        source.cell_area_m2(positions[0].1),
    )
}

#[tokio::test]
async fn global_backend_and_enum_share_the_same_contract() {
    let mut backend = GlobalForcing::new("fixture", 0.0, 0.0, FixtureFetcher { native: false });
    let mut selected = ForcingKind::global("fixture", 0.0, 0.0, FixtureFetcher { native: false });
    let positions = [(0.02, 0.02), (0.04, 0.03)];
    let expected = sample_contract(&mut backend, &positions).await;
    assert_eq!(sample_contract(&mut selected, &positions).await, expected);
    assert_eq!(expected.1[0], (0.5, 4.5, 280.5));
    assert!(selected
        .validate_forcing_interval(20261023, 0.5, 0.6)
        .is_err());
}

#[tokio::test]
async fn norkyst_backend_and_enum_share_the_same_contract() {
    let mut backend = NorkystForcing::new("fixture/", FixtureFetcher { native: true });
    let mut selected = ForcingKind::norkyst("fixture/", FixtureFetcher { native: true });
    let reference: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("norkyst/fixtures/reference.json")).unwrap();
    let positions: Vec<_> = reference
        .iter()
        .rev()
        .map(|p| {
            (
                p["lon"].as_f64().unwrap() as f32,
                p["lat"].as_f64().unwrap() as f32,
            )
        })
        .collect();
    let expected = sample_contract(&mut backend, &positions).await;
    assert_eq!(sample_contract(&mut selected, &positions).await, expected);
    assert_eq!(expected.1[0], (0.0, 4.0, 280.5));
    assert!(selected
        .validate_forcing_interval(20261023, 1.5, 0.6)
        .is_err());
}
