//! tests.rs compares direct binning with the previous exported-array pipeline

use super::*;
use proteus_core::tracers::{GenericTracer, LeewayTracer, OilTracer};

fn particles(kind: &str) -> Particles {
    let tracer = match kind {
        "sar" => TracerKind::Leeway(LeewayTracer::new(
            r#"{"downwind":[0.96,0,0],"right":[0.54,0,0],"left":[-0.54,0,0],"jibe_probability":0,"capsizing":false,"capsize_threshold":30,"capsize_fraction":0.4,"capsize_sigma":5}"#,
            5,
        )),
        "oil" => TracerKind::Oil(OilTracer::new(
            r#"{"wind_factor":0.02,"wind_deflection":0,"product_type":"test","api":30,"density_kgm3":[[900,15]],"dynamic_viscosity_cp":[[9,15]],"interfacial_tension_n_m":[[0.03,15]],"sara_mass_fractions":{"asphaltenes":0.03},"boiling_points_c":[100,400],"molecular_weights_kg_mol":[0.1,0.3],"component_mass_fractions":[0.5,0.5],"bullwinkle_fraction":0.05,"bulltime":0}"#,
            5,
            0.25,
        )),
        _ => TracerKind::Generic(GenericTracer::new(
            r#"{"wind_factor":0.02,"wind_deflection":0}"#,
            0.25,
        )),
    };
    let mut p = Particles::new(5, tracer);
    for (lon, lat) in [
        (5.0, 60.0),
        (5.007, 60.01),
        (5.018, 60.013),
        (5.023, 60.021),
        (20.0, 65.0),
    ] {
        p.add_particle(lon, lat);
    }
    p.stranded[4] = true;
    if let TracerKind::Oil(oil) = &mut p.tracer {
        oil.data
            .total_mass
            .copy_from_slice(&[0.1, 0.0, 2.5, f32::NAN, 100.0]);
    }
    p
}

fn legacy(p: &Particles, cell: f64, smoothing: f64) -> EulerianGrid {
    let b = p.bounding_box_array();
    let mut lons = Vec::new();
    let mut lats = Vec::new();
    let mut weights = Vec::new();
    for i in 0..p.len {
        if p.stranded[i] {
            continue;
        }
        lons.push(p.lons[i] as f64);
        lats.push(p.lats[i] as f64);
        weights.push(match &p.tracer {
            TracerKind::Generic(t) => t.data.mass_per_particle,
            TracerKind::Oil(t) => t.data.total_mass[i],
            TracerKind::Leeway(_) => 1.0,
        });
    }
    let (left, right) = if b[1] as f64 - b[0] as f64 > 180.0 {
        for lon in &mut lons {
            if *lon < 0.0 {
                *lon += 360.0;
            }
        }
        (
            lons.iter().copied().fold(f64::INFINITY, f64::min),
            lons.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    } else {
        (b[0] as f64, b[1] as f64)
    };
    let padding = cell * (smoothing + 1.0);
    let mut grid = EulerianGrid::new(
        left - padding,
        right + padding,
        b[2] as f64 - padding,
        b[3] as f64 + padding,
        cell,
    );
    grid.add_particles(&lons, &lats, Some(&weights));
    for _ in 0..smoothing as usize {
        legacy_smooth(&mut grid);
    }
    if matches!(&p.tracer, TracerKind::Leeway(_)) {
        grid.normalize_probability();
    }
    grid
}

fn legacy_smooth(grid: &mut EulerianGrid) {
    if grid.nx < 2 || grid.ny < 2 {
        return;
    }
    let kernel = [
        1.0 / 16.0,
        2.0 / 16.0,
        1.0 / 16.0,
        2.0 / 16.0,
        4.0 / 16.0,
        2.0 / 16.0,
        1.0 / 16.0,
        2.0 / 16.0,
        1.0 / 16.0,
    ];
    let mut output = vec![0.0; grid.grid.len()];
    for y in 0..grid.ny {
        for x in 0..grid.nx {
            let mut sum = 0.0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = (x as isize + dx).clamp(0, grid.nx as isize - 1) as usize;
                    let sy = (y as isize + dy).clamp(0, grid.ny as isize - 1) as usize;
                    sum += grid.grid[sy * grid.nx + sx] * kernel[((dy + 1) * 3 + dx + 1) as usize];
                }
            }
            output[y * grid.nx + x] = sum;
        }
    }
    grid.grid = output;
}

fn compare(p: &Particles, actual: &mut SimulationHeatmap, cell: f64, smoothing: f64) {
    let expected = legacy(p, cell, smoothing);
    actual.rebuild(p, cell, smoothing);
    assert_eq!(
        (
            actual.grid.nx,
            actual.grid.ny,
            actual.grid.lon_min,
            actual.grid.lat_min
        ),
        (expected.nx, expected.ny, expected.lon_min, expected.lat_min)
    );
    assert_eq!(actual.grid.grid, expected.grid);
    let levels = [0.00001, 0.01, 0.05, 0.1];
    assert_eq!(
        actual.grid.to_contour_geojson(&levels),
        expected.to_contour_geojson(&levels)
    );
    for probability in [0.25, 0.5, 0.75, 0.95] {
        assert_eq!(
            actual.grid.hdr_density_threshold(probability),
            expected.hdr_density_threshold(probability)
        );
    }
    let levels = [0.25, 0.5, 0.75, 0.95];
    assert_eq!(
        actual.grid.to_probability_contour_geojson(&levels),
        expected.to_probability_contour_geojson(&levels)
    );
}

#[test]
fn direct_binning_preserves_tracer_masses_smoothing_and_contours() {
    for kind in ["generic", "oil", "sar"] {
        let p = particles(kind);
        let mut direct = SimulationHeatmap::default();
        for (cell, smoothing) in [(0.005, 0.0), (0.005, 2.0), (0.005, 1.5), (0.002, 3.0)] {
            compare(&p, &mut direct, cell, smoothing);
        }
    }
}

#[test]
fn wrapping_empty_and_degenerate_clouds_match_the_previous_pipeline() {
    let mut p = particles("sar");
    let mut direct = SimulationHeatmap::default();
    for (i, lon) in [179.95, -179.96, 179.97, -179.98].into_iter().enumerate() {
        p.lons[i] = lon;
    }
    compare(&p, &mut direct, 0.01, 2.0);
    p.lons[..4].fill(5.0);
    p.lats[..4].fill(60.0);
    compare(&p, &mut direct, 0.01, 2.0);
    p.lons[0] = f32::NAN;
    compare(&p, &mut direct, 0.01, 2.0);
    p.stranded.fill(true);
    compare(&p, &mut direct, 0.01, 2.0);
    compare(&p, &mut direct, 0.0, 2.0);
}

#[test]
fn grid_and_smoothing_storage_survive_moving_and_shrinking_clouds() {
    let mut p = particles("generic");
    let mut direct = SimulationHeatmap::default();
    direct.rebuild(&p, 0.001, 2.0);
    let pointers = [direct.grid.grid.as_ptr(), direct.grid.smoothed.as_ptr()];
    compare(&p, &mut direct, 0.001, 2.0);
    assert_eq!(
        pointers,
        [direct.grid.grid.as_ptr(), direct.grid.smoothed.as_ptr()]
    );
    p.stranded[0] = true;
    p.stranded[3] = true;
    compare(&p, &mut direct, 0.01, 2.0);
    assert_eq!(
        pointers,
        [direct.grid.grid.as_ptr(), direct.grid.smoothed.as_ptr()]
    );
    p.stranded.fill(true);
    compare(&p, &mut direct, 0.01, 2.0);
    p.stranded[..4].fill(false);
    compare(&p, &mut direct, 0.005, 2.0);
    assert_eq!(
        pointers,
        [direct.grid.grid.as_ptr(), direct.grid.smoothed.as_ptr()]
    );
}
