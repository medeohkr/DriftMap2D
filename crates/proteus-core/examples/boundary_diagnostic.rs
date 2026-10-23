//! run matching tiled and continuous forcing cases near 20 N 110 E

use proteus_core::basemodel::data_loader::{parse_tile_data, TileKey};
use proteus_core::basemodel::{
    meters_per_degree_lat, meters_per_degree_lon, DataLoader, LandMaskLoader, Simulation,
};
use proteus_core::{BoxFuture, FetchError, TileFetcher};
use std::{cell::RefCell, fs, io::Write, path::Path};

struct OfflineFetcher;
impl TileFetcher for OfflineFetcher {
    fn fetch_bytes<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async { Err(FetchError::Network("offline diagnostic".into())) })
    }
}

fn loader(root: &Path, continuous: bool) -> DataLoader<OfflineFetcher> {
    let mut loader = DataLoader::new("", -180.0, -80.0, OfflineFetcher);
    for day in [20260919, 20260920] {
        for x in [28, 29] {
            for y in [9, 10] {
                let filename = if continuous {
                    format!("continuous_{day}.bin")
                } else {
                    format!("{day}_{x:03}_{y:03}.bin")
                };
                let data =
                    parse_tile_data(&fs::read(root.join("tiles").join(filename)).unwrap()).unwrap();
                loader.cache.insert(
                    TileKey {
                        lon_idx: x,
                        lat_idx: y,
                        day,
                    },
                    RefCell::new(data),
                );
            }
        }
    }
    loader
}

fn velocity(loader: &DataLoader<OfflineFetcher>, lon: f32, lat: f32, hour: f32) -> (f32, f32) {
    let value = loader.get_velocities(&[(lon, lat, 0.0)], 20260919, hour)[0];
    (
        value.0 / meters_per_degree_lon(1.0, lat),
        value.1 / meters_per_degree_lat(1.0, lat),
    )
}

fn fresh_velocity(root: &Path, continuous: bool, lon: f32, lat: f32, hour: f32) -> (f32, f32) {
    let mut loader = DataLoader::new("", -180.0, -80.0, OfflineFetcher);
    let key = loader.get_tile_key(lon, lat, 20260919);
    let filename = if continuous {
        "continuous_20260919.bin".to_string()
    } else {
        format!("20260919_{:03}_{:03}.bin", key.lon_idx, key.lat_idx)
    };
    let tile = parse_tile_data(&fs::read(root.join("tiles").join(filename)).unwrap()).unwrap();
    loader.cache.insert(key, RefCell::new(tile));
    velocity(&loader, lon, lat, hour)
}

fn main() {
    let arg = std::env::args()
        .nth(1)
        .expect("pass the diagnostic run directory");
    let root = Path::new(&arg);
    let seeds: Vec<[f32; 2]> =
        serde_json::from_slice(&fs::read(root.join("seeds.json")).unwrap()).unwrap();
    let mut transects = fs::File::create(root.join("velocity_transects.csv")).unwrap();
    writeln!(transects, "mode,cache,hour,axis,fixed,coordinate,u,v").unwrap();
    for (mode, continuous) in [("tiled", false), ("continuous", true)] {
        for hour in [0.0, 6.0, 12.0, 18.0] {
            for axis in ["lon", "lat"] {
                for fixed in if axis == "lon" {
                    [19.99, 20.01, 20.1]
                } else {
                    [109.99, 110.01, 110.1]
                } {
                    let loader = loader(root, continuous);
                    let center = if axis == "lon" { 110.0 } else { 20.0 };
                    for i in 0..2001 {
                        let coordinate = center - 0.2 + i as f32 * 0.0002;
                        let (lon, lat) = if axis == "lon" {
                            (coordinate, fixed)
                        } else {
                            (fixed, coordinate)
                        };
                        // fresh caches distinguish geographic seams from previous query effects
                        if i % 100 == 0 || (coordinate - center).abs() < 0.002 {
                            let (u, v) = fresh_velocity(root, continuous, lon, lat, hour);
                            writeln!(
                                transects,
                                "{mode},fresh,{hour},{axis},{fixed},{coordinate},{u},{v}"
                            )
                            .unwrap();
                        }
                        let (u, v) = velocity(&loader, lon, lat, hour);
                        writeln!(
                            transects,
                            "{mode},sweep,{hour},{axis},{fixed},{coordinate},{u},{v}"
                        )
                        .unwrap();
                    }
                }
            }
        }
        let loader = loader(root, continuous);
        let landmask = LandMaskLoader::new("", -180.0, -90.0, 90.0, OfflineFetcher);
        let releases =
            r#"[{"lon":110,"lat":20,"radius":0,"schedule":[{"amount":1,"duration":0}]}]"#;
        let mut simulation = Simulation::new(
            "generic",
            r#"{"wind_factor":0,"wind_deflection":0}"#,
            releases,
            seeds.len(),
            96,
            "rk4",
            "constant",
            vec![0.0, 0.0],
        );
        simulation.release_particles(0);
        assert_eq!(simulation.particles.len, seeds.len());
        for (i, [lon, lat]) in seeds.iter().enumerate() {
            simulation.particles.lons[i] = *lon;
            simulation.particles.lats[i] = *lat;
        }
        let mut output = fs::File::create(root.join(format!("{mode}_particles.csv"))).unwrap();
        writeln!(output, "step,hours,particle,lon,lat").unwrap();
        for step in 0..=192 {
            if step % 4 == 0 {
                for i in 0..seeds.len() {
                    writeln!(
                        output,
                        "{step},{},{i},{},{}",
                        step as f32 / 4.0,
                        simulation.particles.lons[i],
                        simulation.particles.lats[i]
                    )
                    .unwrap();
                }
            }
            if step == 192 {
                break;
            }
            let day = if step < 96 { 20260919 } else { 20260920 };
            let hour = (step % 96) as f32 / 4.0;
            simulation.update_particles_batch(1.0 / 96.0, &loader, day, hour, &landmask);
        }
        println!("finished {mode} with {} particles", seeds.len());
    }
}
