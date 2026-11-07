//! preload.rs shares projected positions and tile bounds with sampling and preloading

use super::*;

pub(super) struct PreparedPosition {
    pub lon: f32,
    pub lat: f32,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

impl Bounds {
    fn tiles(self, margin: f64, shape: [usize; 2]) -> impl Iterator<Item = (usize, usize)> {
        let left = (self.left - margin).max(0.0) as usize / 120;
        let right = (self.right + margin).min((shape[1] - 1) as f64) as usize / 120;
        let top = (self.top - margin).max(0.0) as usize / 120;
        let bottom = (self.bottom + margin).min((shape[0] - 1) as f64) as usize / 120;
        (left..=right).flat_map(move |x| (top..=bottom).map(move |y| (x, y)))
    }
}

impl<F: TileFetcher> NorkystForcing<F> {
    pub(super) fn plan_positions(
        &mut self,
        positions: &[f32],
        date: usize,
        dates: &[usize],
    ) -> Result<HashSet<TileKey>, LoaderError> {
        if positions.len() % 2 != 0 {
            return Err(LoaderError::Parse("incomplete position pair".into()));
        }
        let grid = &self.manifest.as_ref().unwrap().grid;
        let columns = (grid.grid_shape[1] - 1) / 120 + 1;
        let rows = (grid.grid_shape[0] - 1) / 120 + 1;
        self.bounds.resize(columns * rows, None);
        self.bounds.fill(None);
        self.prepared.clear();
        self.preload.spatial.clear();
        self.preload.date = date;
        for pair in positions.chunks_exact(2) {
            let lon = normalize_lon(pair[0]);
            let lat = pair[1];
            let (x, y) = grid
                .position(lon, lat)
                .ok_or_else(|| LoaderError::NotFound("position outside NorKyst domain".into()))?;
            self.prepared.push(PreparedPosition { lon, lat, x, y });
            let slot = &mut self.bounds[y as usize / 120 * columns + x as usize / 120];
            if let Some(b) = slot {
                b.left = b.left.min(x);
                b.right = b.right.max(x);
                b.top = b.top.min(y);
                b.bottom = b.bottom.max(y);
            } else {
                *slot = Some(Bounds {
                    left: x,
                    right: x,
                    top: y,
                    bottom: y,
                });
            }
        }
        // expand one envelope per occupied tile instead of hashing tiles for every particle
        let mut needed = HashSet::new();
        for b in self.bounds.iter().flatten().copied() {
            for (lon_idx, lat_idx) in b.tiles(15.0, grid.grid_shape) {
                for &day in dates {
                    needed.insert(TileKey {
                        lon_idx,
                        lat_idx,
                        day,
                    });
                }
            }
            self.preload.spatial.extend(b.tiles(60.0, grid.grid_shape));
        }
        Ok(needed)
    }

    pub(in crate::forcing) async fn plan_preload(
        &mut self,
        positions: &[f32],
        date: usize,
    ) -> Result<(), LoaderError> {
        self.ensure_manifest().await?;
        self.interval_dates(date, 0.0, 0.0)?;
        self.plan_positions(positions, date, &[])?;
        Ok(())
    }

    pub(in crate::forcing) fn preload_urls(&self) -> Vec<String> {
        let Some(manifest) = &self.manifest else {
            return Vec::new();
        };
        self.preload.urls(
            &self.base_url,
            |day| manifest.day(day).is_some(),
            |key| self.tiles.contains_key(key),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margins_clip_to_partial_domain_tiles_and_cover_tile_crossings() {
        let first = Bounds {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        };
        assert_eq!(
            first.tiles(60.0, [1148, 2747]).collect::<Vec<_>>(),
            [(0, 0)]
        );
        let last = Bounds {
            left: 2746.0,
            right: 2746.0,
            top: 1147.0,
            bottom: 1147.0,
        };
        assert_eq!(
            last.tiles(60.0, [1148, 2747]).collect::<Vec<_>>(),
            [(22, 9)]
        );
        let crossing = Bounds {
            left: 119.5,
            right: 120.5,
            top: 119.5,
            bottom: 120.5,
        };
        assert_eq!(
            crossing.tiles(15.0, [1148, 2747]).collect::<Vec<_>>(),
            [(0, 0), (0, 1), (1, 0), (1, 1)]
        );
    }
}
