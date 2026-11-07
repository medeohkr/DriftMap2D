//! preload.rs builds small background request lists from prepared spatial tiles

use super::{next_date, TileKey};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct PreloadPlan {
    pub spatial: HashSet<(usize, usize)>,
    pub date: usize,
}

impl PreloadPlan {
    /// include the next available day without requesting tiles already decoded
    pub fn urls(
        &self,
        base: &str,
        available: impl Fn(usize) -> bool,
        loaded: impl Fn(&TileKey) -> bool,
    ) -> Vec<String> {
        let mut urls = Vec::new();
        for day in [Some(self.date), next_date(self.date).ok()]
            .into_iter()
            .flatten()
        {
            if !available(day) {
                continue;
            }
            for &(lon_idx, lat_idx) in &self.spatial {
                let key = TileKey {
                    lon_idx,
                    lat_idx,
                    day,
                };
                if !loaded(&key) {
                    urls.push(tile_url(base, &key));
                }
            }
        }
        urls.sort_unstable();
        urls
    }
}

pub(super) fn tile_url(base: &str, key: &TileKey) -> String {
    format!(
        "{}/{:04}/{:02}/{:02}/{:03}_{:03}.bin",
        base.trim_end_matches('/'),
        key.day / 10000,
        key.day / 100 % 100,
        key.day % 100,
        key.lon_idx,
        key.lat_idx
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_skip_decoded_tiles_and_roll_dates_across_years() {
        let plan = PreloadPlan {
            date: 20261231,
            spatial: [(3, 5)].into_iter().collect(),
        };
        assert_eq!(
            plan.urls("root/", |_| true, |key| key.day == 20261231),
            ["root/2027/01/01/003_005.bin"]
        );
        assert_eq!(
            plan.urls("root", |day| day == 20261231, |_| false),
            ["root/2026/12/31/003_005.bin"]
        );
        assert!(PreloadPlan::default()
            .urls("root", |_| true, |_| false)
            .is_empty());
    }
}
