use crate::fetch::TileFetcher;
use roaring::RoaringBitmap;
use std::collections::{HashMap, HashSet};
use super::normalize_lon;
use super::landmask_polygons::PolygonTile;

struct LandTile {
    bitmap: RoaringBitmap,
    polygons: PolygonTile,
}

pub struct LandMaskLoader<F: TileFetcher> {
    min_lon: f32,
    min_lat: f32,
    max_lat: f32,
    tile_size: f32,
    cells_per_tile: u32,
    base_url: String,
    fetcher: F,
    cache: HashMap<(usize, usize), LandTile>,
    loaded_tiles: HashSet<(usize, usize)>,
}

impl <F: TileFetcher>LandMaskLoader<F> {
    /// locate the first detected land crossing using the currently loaded mask
    pub fn shoreline_position(&self, start: (f32, f32), end: (f32, f32)) -> (f32, f32) {
        shoreline_crossing(start, end, |lon, lat| self.is_on_land(lon, lat))
    }

    pub fn new(base_url: &str, min_lon: f32, min_lat: f32, max_lat: f32, fetcher: F) -> Self {
        let tile_size = 10.0;
        let cells_per_tile = 2400; // Hardcoded

        Self {
            min_lon,
            min_lat,
            max_lat,
            tile_size,
            cells_per_tile,
            base_url: base_url.to_string(),
            fetcher,
            cache: HashMap::new(),
            loaded_tiles: HashSet::new()
        }
    }

    pub fn update_tiles(&mut self, positions: Vec<f32>) -> HashSet<(usize, usize)> {
        let mut needed = HashSet::new();
        let edge_threshold = 0.1;

        for position in positions.chunks_exact(2) {
            let lon = normalize_lon(position[0]) as f64;
            let lat = position[1] as f64;

            let lon_idx = ((lon - self.min_lon as f64) / self.tile_size as f64).floor() as i32;
            let lat_idx = ((lat - self.min_lat as f64) / self.tile_size as f64).floor() as i32;

            let mut add_tile = |lon_idx: i32, lat_idx: i32| {
                if lat_idx >= 0 && lat_idx < 18 {
                    needed.insert((lon_idx.rem_euclid(36) as usize, lat_idx as usize));
                }
            };

            add_tile(lon_idx, lat_idx);

            let lon_mod = ((lon % 10.0) + 10.0) % 10.0;
            let lat_mod = ((lat % 10.0) + 10.0) % 10.0;

            if lon_mod < edge_threshold {
                add_tile(lon_idx - 1, lat_idx);
            } else if lon_mod > 10.0 - edge_threshold {
                if lon_idx == 14 && lat_idx == 10 {
                }
                add_tile(lon_idx + 1, lat_idx);
            }

            if lat_mod < edge_threshold {
                add_tile(lon_idx, lat_idx - 1);
            } else if lat_mod > 10.0 - edge_threshold {
                add_tile(lon_idx, lat_idx + 1);
            }

            if (lon_mod < edge_threshold || lon_mod > 10.0 - edge_threshold) &&
            (lat_mod < edge_threshold || lat_mod > 10.0 - edge_threshold) {
                let lon_dir = if lon_mod < edge_threshold { -1 } else { 1 };
                let lat_dir = if lat_mod < edge_threshold { -1 } else { 1 };
                add_tile(lon_idx + lon_dir, lat_idx + lat_dir);
            }
        }
        needed.difference(&self.loaded_tiles).cloned().collect()
    }

    pub async fn load_tile(&mut self, lon_idx: usize, lat_idx: usize) -> Result<(), String> {
        let url = format!(
            "{}/landmask_{:03}_{:03}.bin",
            self.base_url, lon_idx, lat_idx
        );

        let bytes = self
            .fetcher
            .fetch_bytes(&url)
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if bytes.len() < 8 {
            return Err("File too short".to_string());
        }
        if u32::from_le_bytes(bytes[..4].try_into().unwrap()) != 2400
            || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2400 {
            return Err("invalid landmask raster dimensions".into());
        }

        let bitmap = RoaringBitmap::deserialize_from(&bytes[8..])
            .map_err(|e| format!("Failed to deserialize bitmap: {:?}", e))?;

        let url = format!("{}/polygons/landmask_{:03}_{:03}.bin.zst", self.base_url, lon_idx, lat_idx);
        let bytes = self.fetcher.fetch_bytes(&url).await
            .map_err(|e| format!("polygon tile unavailable {url}: {e}"))?;
        let polygons = PolygonTile::parse(&bytes, [lon_idx, lat_idx])?;
        self.cache.insert((lon_idx, lat_idx), LandTile { bitmap, polygons });
        self.loaded_tiles.insert((lon_idx, lat_idx));
        Ok(())
    }

    pub async fn load_landmask_tiles(&mut self, positions: Vec<f32>) -> Result<(), String> {
        let needed_landmask_tiles = self.update_tiles(positions);

        for (lon_idx, lat_idx) in needed_landmask_tiles {
            self.load_tile(lon_idx, lat_idx).await?;
        }
        Ok(())
    }
    pub fn is_on_land(&self, lon: f32, lat: f32) -> bool {
        if !lon.is_finite() || !lat.is_finite() { return false; }
        let lon = normalize_lon(lon);
        // Check bounds
        if lat < self.min_lat || lat > self.max_lat {
            return false;
        }

        let lon_idx_raw = (lon - self.min_lon) / self.tile_size;
        let lat_idx_raw = (lat - self.min_lat) / self.tile_size;
        let lon_idx = lon_idx_raw.floor() as i32;
        let lat_idx = lat_idx_raw.floor() as i32;

        if lon_idx < 0 || lat_idx < 0 {
            return false;
        }

        let lon_idx = lon_idx as usize;
        let lat_idx = lat_idx as usize;

        // GSHHG raster pixels are centred on their coordinates, with a half-cell offset
        let gx = ((lon as f64 - self.min_lon as f64) * 240.0 + 0.5).floor() as u32;
        let gy = ((lat as f64 - self.min_lat as f64) * 240.0 + 0.5).floor() as u32;
        if gy >= 18 * self.cells_per_tile { return false; }
        let gx = gx % (36 * self.cells_per_tile);
        let raster_key = ((gx / self.cells_per_tile) as usize, (gy / self.cells_per_tile) as usize);
        let Some(raster) = self.cache.get(&raster_key) else { return false; };
        let idx = (gy % self.cells_per_tile) * self.cells_per_tile + gx % self.cells_per_tile;
        if !raster.bitmap.contains(idx) { return false; }
        self.cache.get(&(lon_idx, lat_idx))
            .is_some_and(|tile| tile.polygons.contains(lon as f64, lat as f64))
    }
}

/// scan before refining so a later coastline cannot hide an earlier island
fn shoreline_crossing(
    start: (f32, f32),
    end: (f32, f32),
    on_land: impl Fn(f32, f32) -> bool,
) -> (f32, f32) {
    if ![start.0, start.1, end.0, end.1].iter().all(|v| v.is_finite()) {
        return end;
    }
    if on_land(start.0, start.1) {
        return start;
    }
    // follow the short segment across the dateline rather than through longitude zero
    let dx = (end.0 as f64 - start.0 as f64 + 180.0).rem_euclid(360.0) - 180.0;
    let dy = end.1 as f64 - start.1 as f64;
    let span = dx.abs().max(dy.abs());
    let position = |fraction: f64| {
        (normalize_lon((start.0 as f64 + dx * fraction) as f32),
         (start.1 as f64 + dy * fraction) as f32)
    };
    let samples = (span / 0.001).ceil().max(1.0) as usize;
    let mut water = 0.0;
    for i in 1..=samples {
        let mut land = i as f64 / samples as f64;
        let point = position(land);
        if on_land(point.0, point.1) {
            while (land - water) * span > 0.0001 {
                let middle = (water + land) * 0.5;
                let point = position(middle);
                if on_land(point.0, point.1) {
                    land = middle;
                } else {
                    water = middle;
                }
            }
            return position(land);
        }
        water = land;
    }
    end
}

#[cfg(test)]
mod shoreline_tests {
    use super::shoreline_crossing;

    #[test]
    fn stops_on_land_near_the_crossing_instead_of_the_old_position() {
        let point = shoreline_crossing((0.0, 0.0), (0.02, 0.01), |x, _| x >= 0.007);
        assert!(point.0 >= 0.007 && point.0 < 0.0071);
        assert!((point.1 - point.0 * 0.5).abs() < 0.00001);
    }

    #[test]
    fn detects_an_earlier_island_before_the_final_landmass() {
        let point = shoreline_crossing((0.0, 0.0), (0.02, 0.0),
            |x, _| (0.003..0.006).contains(&x) || x >= 0.015);
        assert!(point.0 >= 0.003 && point.0 < 0.0031);
    }

    #[test]
    fn crosses_the_dateline_without_visiting_the_other_side_of_the_world() {
        let point = shoreline_crossing((179.99, 1.0), (-179.99, 1.0), |x, _| x < -179.999);
        assert!(point.0 < -179.999 && point.0 >= -180.0);
        assert_eq!(point.1, 1.0);
    }

    #[test]
    fn keeps_existing_land_positions_and_handles_short_moves() {
        assert_eq!(shoreline_crossing((1.0, 1.0), (1.1, 1.0), |_, _| true), (1.0, 1.0));
        assert_eq!(shoreline_crossing((0.0, 0.0), (0.00001, 0.0), |x, _| x > 0.0), (0.00001, 0.0));
    }
}

#[cfg(test)]
mod polygon_loader_tests {
    use super::*;
    use crate::{BoxFuture, FetchError};

    struct MemoryFetcher { polygons: bool }

    impl TileFetcher for MemoryFetcher {
        fn fetch_bytes<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
            Box::pin(async move {
                if url.ends_with(".bin.zst") {
                    return if self.polygons {
                        Ok(include_bytes!("landmask_fixtures/polygons.bin.zst").to_vec())
                    } else { Err(FetchError::Http(404)) };
                }
                let mut bitmap = RoaringBitmap::new();
                for (x, y) in [(0.02f32, 0.02f32), (0.1, 0.1)] {
                    bitmap.insert((y as f64 * 240.0 + 0.5).floor() as u32 * 2400
                        + (x as f64 * 240.0 + 0.5).floor() as u32);
                }
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&2400u32.to_le_bytes());
                bytes.extend_from_slice(&2400u32.to_le_bytes());
                bitmap.serialize_into(&mut bytes).unwrap();
                Ok(bytes)
            })
        }
    }

    #[tokio::test]
    async fn raster_candidates_require_polygon_confirmation() {
        let mut mask = LandMaskLoader::new("memory", -180.0, -90.0, 90.0, MemoryFetcher { polygons: true });
        mask.load_tile(18, 9).await.unwrap();
        assert!(mask.is_on_land(0.02, 0.02));
        assert!(!mask.is_on_land(0.1, 0.1));
        assert!(!mask.is_on_land(0.3, 0.1));
    }

    #[tokio::test]
    async fn failed_polygon_download_never_marks_the_tile_loaded() {
        let mut mask = LandMaskLoader::new("memory", -180.0, -90.0, 90.0, MemoryFetcher { polygons: false });
        assert!(mask.load_landmask_tiles(vec![1.0, 1.0]).await.is_err());
        assert!(mask.cache.is_empty());
        assert!(mask.loaded_tiles.is_empty());
    }

    #[test]
    fn polar_and_dateline_neighbors_are_requested() {
        let mut mask = LandMaskLoader::new("memory", -180.0, -90.0, 90.0, MemoryFetcher { polygons: true });
        let keys = mask.update_tiles(vec![179.99, 85.0]);
        assert!(keys.contains(&(35, 17)));
        assert!(keys.contains(&(0, 17)));
    }
}
