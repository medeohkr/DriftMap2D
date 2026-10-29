// landmask_polygons.rs refines raster candidates using bucketed coastline rings

use serde::Deserialize;
use std::io::Read;

type Ring = Vec<[f64; 2]>;
type Polygon = Vec<Ring>;

#[derive(Deserialize)]
pub(super) struct PolygonTile {
    version: u32,
    tile_index: [usize; 2],
    cell_size: f64,
    cells: Vec<Vec<Polygon>>,
}

impl PolygonTile {
    pub fn parse(bytes: &[u8], index: [usize; 2]) -> Result<Self, String> {
        const LIMIT: u64 = 64 * 1024 * 1024;
        let decoder = ruzstd::decoding::StreamingDecoder::new(bytes)
            .map_err(|e| format!("invalid polygon compression: {e}"))?;
        let mut raw = Vec::new();
        decoder
            .take(LIMIT + 1)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        if raw.len() as u64 > LIMIT || !raw.starts_with(b"LMP1") {
            return Err("invalid polygon payload or size".into());
        }
        let tile: Self = serde_json::from_slice(&raw[4..]).map_err(|e| e.to_string())?;
        if tile.version != 1
            || tile.tile_index != index
            || tile.cell_size != 0.25
            || tile.cells.len() != 1600
            || index[0] >= 36
            || index[1] >= 18
        {
            return Err("unsupported polygon tile layout".into());
        }
        let x = index[0] as f64 * 10.0 - 180.0;
        let y = index[1] as f64 * 10.0 - 90.0;
        for (cell, polygons) in tile.cells.iter().enumerate() {
            let west = x + (cell % 40) as f64 * 0.25;
            let south = y + (cell / 40) as f64 * 0.25;
            for polygon in polygons {
                if polygon.is_empty() {
                    return Err("empty polygon".into());
                }
                for ring in polygon {
                    if ring.len() < 4
                        || ring.first() != ring.last()
                        || ring.iter().any(|p| {
                            !p[0].is_finite()
                                || !p[1].is_finite()
                                || p[0] < west - 1e-8
                                || p[0] > west + 0.25 + 1e-8
                                || p[1] < south - 1e-8
                                || p[1] > south + 0.25 + 1e-8
                        })
                    {
                        return Err("invalid polygon ring or bucket bounds".into());
                    }
                }
            }
        }
        Ok(tile)
    }

    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        let x = (lon + 180.0 - self.tile_index[0] as f64 * 10.0) / self.cell_size;
        let y = (lat + 90.0 - self.tile_index[1] as f64 * 10.0) / self.cell_size;
        if !(0.0..40.0).contains(&x) || !(0.0..40.0).contains(&y) {
            return false;
        }
        self.cells[y.floor() as usize * 40 + x.floor() as usize]
            .iter()
            .any(|polygon| match in_ring(&polygon[0], lon, lat) {
                0 => false,
                2 => true,
                _ => {
                    for hole in &polygon[1..] {
                        match in_ring(hole, lon, lat) {
                            1 => return false,
                            2 => return true,
                            _ => {}
                        }
                    }
                    true
                }
            })
    }
}

// boundary points are land, including artificial clipping edges between buckets
fn in_ring(ring: &Ring, x: f64, y: f64) -> u8 {
    let mut inside = false;
    for segment in ring.windows(2) {
        let ([ax, ay], [bx, by]) = (segment[0], segment[1]);
        let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
        if cross.abs() <= 1e-12
            && x >= ax.min(bx) - 1e-12
            && x <= ax.max(bx) + 1e-12
            && y >= ay.min(by) - 1e-12
            && y <= ay.max(by) + 1e-12
        {
            return 2;
        }
        if (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay) {
            inside = !inside;
        }
    }
    u8::from(inside)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(a: f64, b: f64) -> Ring {
        vec![[a, a], [b, a], [b, b], [a, b], [a, a]]
    }

    #[test]
    fn holes_and_boundaries_are_distinguished() {
        let mut cells = vec![vec![]; 1600];
        cells[0] = vec![vec![square(0.0, 0.25), square(0.05, 0.15)]];
        let tile = PolygonTile {
            version: 1,
            tile_index: [18, 9],
            cell_size: 0.25,
            cells,
        };
        assert!(tile.contains(0.02, 0.02));
        assert!(!tile.contains(0.1, 0.1));
        assert!(tile.contains(0.05, 0.1));
        assert!(tile.contains(0.0, 0.1));
        assert!(!tile.contains(1.0, 1.0));
    }

    #[test]
    fn python_fixture_crosses_bucket_boundaries_and_preserves_holes() {
        let bytes = include_bytes!("landmask_fixtures/polygons.bin.zst");
        let tile = PolygonTile::parse(bytes, [18, 9]).unwrap();
        assert!(tile.contains(0.24999, 0.1));
        assert!(tile.contains(0.25, 0.1));
        assert!(tile.contains(0.25001, 0.1));
        assert!(!tile.contains(0.1, 0.1));
        assert!(PolygonTile::parse(bytes, [19, 9]).is_err());
        assert!(PolygonTile::parse(&bytes[..12], [18, 9]).is_err());
    }
}
