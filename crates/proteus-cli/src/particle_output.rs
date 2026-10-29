// particle_output.rs streams stable particle IDs and positions one snapshot at a time

use chrono::NaiveDateTime;
use proteus_core::basemodel::Particles;
use serde::{Serialize, Serializer};
use std::io::Write;

#[derive(Serialize)]
struct Particle {
    id: usize,
    lon: f32,
    lat: f32,
    stranded: bool,
}

struct Positions<'a> {
    lons: &'a [f32],
    lats: &'a [f32],
    stranded: &'a [bool],
}

impl Serialize for Positions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // indices stay stable because released particles are appended and never removed
        serializer.collect_seq((0..self.lons.len()).map(|id| Particle {
            id,
            lon: self.lons[id],
            lat: self.lats[id],
            stranded: self.stranded[id],
        }))
    }
}

#[derive(Serialize)]
struct Snapshot<'a> {
    step: u32,
    time: String,
    particles: Positions<'a>,
}

pub fn should_snapshot(step: u32, final_step: u32, interval: u32) -> bool {
    step == 0 || step == final_step || step % interval == 0
}

pub fn write_snapshot(
    writer: &mut impl Write,
    step: u32,
    time: NaiveDateTime,
    particles: &Particles,
) -> anyhow::Result<()> {
    let snapshot = Snapshot {
        step,
        time: time.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        particles: Positions {
            lons: &particles.lons[..particles.len],
            lats: &particles.lats[..particles.len],
            stranded: &particles.stranded[..particles.len],
        },
    };
    serde_json::to_writer(&mut *writer, &snapshot)?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_keep_ids_stranding_and_empty_releases() {
        let mut bytes = Vec::new();
        for count in [0, 2] {
            let snapshot = Snapshot {
                step: count as u32,
                time: "2026-10-28T00:00:00Z".into(),
                particles: Positions {
                    lons: &[3.0, 4.0][..count],
                    lats: &[58.0, 59.0][..count],
                    stranded: &[false, true][..count],
                },
            };
            serde_json::to_writer(&mut bytes, &snapshot).unwrap();
            bytes.push(b'\n');
        }
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows[0]["particles"].as_array().unwrap().len(), 0);
        assert_eq!(rows[1]["particles"][1]["id"], 1);
        assert_eq!(rows[1]["particles"][1]["lon"], 4.0);
        assert_eq!(rows[1]["particles"][1]["stranded"], true);
    }

    #[test]
    fn interval_includes_initial_and_nonmultiple_final_steps() {
        let steps: Vec<_> = (0..=5)
            .filter(|&step| should_snapshot(step, 5, 3))
            .collect();
        assert_eq!(steps, vec![0, 3, 5]);
    }
}
