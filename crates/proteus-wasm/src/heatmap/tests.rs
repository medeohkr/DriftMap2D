// tests.rs checks merged contours against the previous per-cell geometry

use super::{contours, EulerianGrid, Point2D};

fn grid(rows: &[&[f32]]) -> EulerianGrid {
    let mut grid = EulerianGrid::new(0.0, rows[0].len() as f64, 0.0, rows.len() as f64, 1.0);
    grid.grid = rows.iter().flat_map(|row| row.iter().copied()).collect();
    grid
}

/** retain the original cell polygons as an independent coverage reference */
fn legacy_cells(grid: &EulerianGrid, threshold: f32) -> Vec<Vec<Point2D>> {
    let mut cells = Vec::new();
    for y in 0..grid.ny - 1 {
        for x in 0..grid.nx - 1 {
            let values = [
                grid.grid[y * grid.nx + x],
                grid.grid[y * grid.nx + x + 1],
                grid.grid[(y + 1) * grid.nx + x + 1],
                grid.grid[(y + 1) * grid.nx + x],
            ];
            let config = values.iter().enumerate().fold(0, |bits, (i, &v)| {
                bits | if v.is_finite() && v >= threshold {
                    1 << i
                } else {
                    0
                }
            });
            let point = |dx, dy| Point2D {
                x: grid.lon_min + (x as f64 + dx) * grid.cell_size,
                y: grid.lat_min + (y as f64 + dy) * grid.cell_size,
            };
            let corners = [
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ];
            let interpolate = |a: usize, b: usize| {
                let difference = values[b] - values[a];
                let t = if !difference.is_finite() || difference.abs() < 1.0e-10 {
                    0.5
                } else {
                    ((threshold - values[a]) / difference).clamp(0.0, 1.0) as f64
                };
                Point2D {
                    x: corners[a].x + t * (corners[b].x - corners[a].x),
                    y: corners[a].y + t * (corners[b].y - corners[a].y),
                }
            };
            let [p0, p1, p2, p3] = corners;
            let [mb, mr, mt, ml] = [
                interpolate(0, 1),
                interpolate(1, 2),
                interpolate(3, 2),
                interpolate(0, 3),
            ];
            let mut polygon = match config {
                0 => continue,
                1 => vec![p0, mb, ml],
                2 => vec![p1, mr, mb],
                3 => vec![p0, p1, mr, ml],
                4 => vec![p2, mt, mr],
                5 => vec![p0, mb, mr, p2, mt, ml],
                6 => vec![p1, p2, mt, mb],
                7 => vec![p0, p1, p2, mt, ml],
                8 => vec![p3, ml, mt],
                9 => vec![p0, p3, mt, mb],
                10 => vec![mb, p1, mr, mt, p3, ml],
                11 => vec![p0, p1, mr, mt, p3],
                12 => vec![p2, p3, ml, mr],
                13 => vec![p0, mb, mr, p2, p3],
                14 => vec![mb, p1, p2, p3, ml],
                15 => vec![p0, p1, p2, p3],
                _ => unreachable!(),
            };
            polygon.push(polygon[0]);
            cells.push(polygon);
        }
    }
    cells
}

fn check_coverage(grid: &EulerianGrid, threshold: f32) {
    let old = legacy_cells(grid, threshold);
    let rings = grid.marching_squares(threshold);
    let old_area: f64 = old
        .iter()
        .map(|ring| contours::signed_area(ring).abs())
        .sum();
    let area: f64 = rings.iter().map(|ring| contours::signed_area(ring)).sum();
    assert!(
        (old_area - area).abs() < 1.0e-9,
        "area changed from {old_area} to {area} for {:?}",
        grid.grid
    );
    for ring in &rings {
        assert!(ring.len() >= 4);
        assert_eq!(ring.first(), ring.last());
        assert!(ring.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!(ring.windows(2).all(|p| p[0] != p[1]));
        let vertices: std::collections::HashSet<_> = ring[..ring.len() - 1]
            .iter()
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .collect();
        assert_eq!(
            vertices.len(),
            ring.len() - 1,
            "self-touching boundary for {:?}",
            grid.grid
        );
    }
    let polygons = contours::polygons(rings);
    for y in 0..grid.ny - 1 {
        for x in 0..grid.nx - 1 {
            for (dx, dy) in [(0.173, 0.319), (0.731, 0.827)] {
                let point = Point2D {
                    x: grid.lon_min + (x as f64 + dx) * grid.cell_size,
                    y: grid.lat_min + (y as f64 + dy) * grid.cell_size,
                };
                let expected = old.iter().any(|ring| contours::contains(ring, point));
                let actual = polygons.iter().any(|rings| {
                    contours::contains(&rings[0], point)
                        && !rings[1..]
                            .iter()
                            .any(|hole| contours::contains(hole, point))
                });
                assert_eq!(
                    actual, expected,
                    "coverage changed at {point:?} for {:?}",
                    grid.grid
                );
            }
        }
    }
}

#[test]
fn all_cell_cases_keep_the_original_coverage() {
    for config in 0..16 {
        let v = |bit| if config & (1 << bit) != 0 { 1.0 } else { 0.0 };
        check_coverage(&grid(&[&[v(0), v(1)], &[v(3), v(2)]]), 0.5);
    }
}

#[test]
fn threshold_plateaus_and_touching_boundaries_keep_the_original_coverage() {
    let mut grid = EulerianGrid::new(0.0, 3.0, 0.0, 3.0, 1.0);
    for mut pattern in 0..3_usize.pow(9) {
        for value in &mut grid.grid {
            *value = (pattern % 3) as f32;
            pattern /= 3;
        }
        check_coverage(&grid, 1.0);
    }
}

#[test]
fn interpolated_and_missing_values_keep_the_original_coverage() {
    let mut grid = EulerianGrid::new(170.0, 170.05, -40.0, -39.95, 0.01);
    let mut seed = 7_u32;
    for _ in 0..100 {
        for value in &mut grid.grid {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *value = if seed % 17 == 0 {
                f32::NAN
            } else {
                (seed % 1000) as f32 / 500.0
            };
        }
        for threshold in [0.25, 0.7, 1.5] {
            check_coverage(&grid, threshold);
        }
    }
}

#[test]
fn larger_threshold_plateaus_preserve_shell_and_hole_coverage() {
    let mut grid = EulerianGrid::new(0.0, 8.0, 0.0, 8.0, 1.0);
    let mut seed = 23_u32;
    for _ in 0..300 {
        for value in &mut grid.grid {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *value = if seed % 13 == 0 {
                f32::NAN
            } else {
                (seed % 3) as f32
            };
        }
        check_coverage(&grid, 1.0);
    }
}

#[test]
fn dense_regions_collapse_to_one_closed_rectangle() {
    let mut grid = EulerianGrid::new(0.0, 100.0, 0.0, 100.0, 1.0);
    grid.grid.fill(1.0);
    let old = legacy_cells(&grid, 0.5);
    assert_eq!(old.len(), 9801);
    let rings = grid.marching_squares(0.5);
    assert_eq!(rings.len(), 1);
    assert_eq!(rings[0].len(), 5);
    assert_eq!(contours::signed_area(&rings[0]), 9801.0);
    let json = grid.to_contour_geojson(&[0.5]);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["features"].as_array().unwrap().len(), 1);
    assert_eq!(value["features"][0]["geometry"]["type"], "MultiPolygon");
    assert!(json.len() < 300);
    println!(
        "dense grid cells={} merged_rings={} vertices={} geojson_bytes={}",
        old.len(),
        rings.len(),
        rings[0].len(),
        json.len()
    );
}

#[test]
fn holes_remain_empty_and_disconnected_patches_remain_separate() {
    let mut grid = EulerianGrid::new(0.0, 7.0, 0.0, 7.0, 1.0);
    grid.grid.fill(1.0);
    for y in 1..6 {
        for x in 1..6 {
            grid.grid[y * 7 + x] = 0.0;
        }
    }
    grid.grid[3 * 7 + 3] = 1.0;
    check_coverage(&grid, 0.5);
    let polygons = contours::polygons(grid.marching_squares(0.5));
    assert_eq!(polygons.len(), 2);
    assert_eq!(polygons.iter().map(Vec::len).sum::<usize>(), 3);
    assert_eq!(polygons.iter().filter(|p| p.len() == 2).count(), 1);
    let islands = self::grid(&[&[0.0; 5], &[0.0, 1.0, 0.0, 1.0, 0.0], &[0.0; 5]]);
    assert_eq!(contours::polygons(islands.marching_squares(0.5)).len(), 2);
}

#[test]
fn holes_touching_at_one_vertex_have_separate_simple_rings() {
    let mut grid = EulerianGrid::new(0.0, 5.0, 0.0, 5.0, 1.0);
    grid.grid.fill(2.0);
    grid.grid[11] = 0.0;
    grid.grid[12] = 1.0;
    grid.grid[13] = 0.0;
    check_coverage(&grid, 1.0);
    let polygons = contours::polygons(grid.marching_squares(1.0));
    assert_eq!(polygons.len(), 1);
    assert_eq!(polygons[0].len(), 3);
}

#[test]
fn concentration_and_probability_exports_keep_level_order_and_close_all_rings() {
    let mut grid = EulerianGrid::new(0.0, 4.0, 0.0, 4.0, 1.0);
    grid.grid.fill(1.0);
    grid.normalize_probability();
    for (json, levels) in [
        (
            grid.to_contour_geojson(&[0.01, 0.02, 0.03]),
            vec![0.01_f32, 0.02, 0.03],
        ),
        (
            grid.to_probability_contour_geojson(&[0.25, 0.5, 0.75, 0.95]),
            vec![0.95, 0.75, 0.5, 0.25],
        ),
    ] {
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        let features = json["features"].as_array().unwrap();
        assert_eq!(features.len(), levels.len());
        for (feature, level) in features.iter().zip(levels) {
            assert!(
                (feature["properties"]["concentration"].as_f64().unwrap() - level as f64).abs()
                    < 1.0e-7
            );
            for polygon in feature["geometry"]["coordinates"].as_array().unwrap() {
                for ring in polygon.as_array().unwrap() {
                    let ring = ring.as_array().unwrap();
                    assert!(ring.len() >= 4);
                    assert_eq!(ring.first(), ring.last());
                }
            }
        }
    }
}

#[test]
fn empty_and_invalid_samples_produce_no_geometry() {
    let grid = grid(&[&[0.0, f32::NAN], &[f32::INFINITY, 0.0]]);
    assert!(grid.generate_contours(&[0.5, f32::NAN]).is_empty());
    assert!(grid.generate_probability_contours(&[0.5, 0.95]).is_empty());
}
