//! compare_forcing.rs measures cached global and NorKyst work on the same particle cloud

#[path = "../src/fetch.rs"]
mod fetch;
#[allow(dead_code)]
#[path = "../../proteus-core/src/forcing/projection.rs"]
mod projection;

use anyhow::Result;
use proteus_core::{
    arrays::{FieldBuffers, VectorBuffer},
    basemodel::{LandMaskLoader, Simulation},
    forcing::{Forcing, ForcingFields, ForcingKind},
};
use serde_json::json;
use std::{hint::black_box, time::Instant};

/// report warmed median and p90 latency without including setup or input restoration
fn measure(repeats: usize, mut operation: impl FnMut()) -> serde_json::Value {
    for _ in 0..5 {
        operation();
    }
    let mut times = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let start = Instant::now();
        operation();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    summary(&mut times)
}

fn summary(times: &mut [f64]) -> serde_json::Value {
    times.sort_by(f64::total_cmp);
    json!({"median_ms": times[times.len() / 2], "p90_ms": times[times.len() * 9 / 10]})
}

/// a fixed offshore square avoids random seed differences and coastline work
fn positions(count: usize) -> Vec<(f32, f32)> {
    let side = (count as f64).sqrt().ceil() as usize;
    (0..count)
        .map(|i| {
            let x = (i % side) as f32 / side as f32 - 0.5;
            let y = (i / side) as f32 / side as f32 - 0.5;
            (3.0 + x * 0.008, 59.0 + y * 0.004)
        })
        .collect()
}

/// include the owned coordinate copy required by the current preparation API
async fn preparation(
    forcing: &mut ForcingKind<fetch::ReqwestFetcher>,
    points: &[(f32, f32)],
    date: usize,
    repeats: usize,
) -> Result<serde_json::Value> {
    let flat: Vec<f32> = points.iter().flat_map(|&(x, y)| [x, y]).collect();
    let mut times = Vec::with_capacity(repeats);
    for i in 0..repeats + 5 {
        let start = Instant::now();
        forcing
            .prepare_forcing(flat.clone(), date, 12.0, 0.25)
            .await?;
        if i >= 5 {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    Ok(summary(&mut times))
}

async fn compare(
    model: &str,
    root: &str,
    land_root: &str,
    date: usize,
    count: usize,
    repeats: usize,
) -> Result<()> {
    let points = positions(count);
    let flat: Vec<f32> = points.iter().flat_map(|&(x, y)| [x, y]).collect();
    let mut forcing = if model == "global" {
        ForcingKind::global(root, -180.0, -80.0, fetch::ReqwestFetcher::new())
    } else {
        ForcingKind::norkyst(root, fetch::ReqwestFetcher::new())
    };
    let mut landmask =
        LandMaskLoader::new(land_root, -180.0, -90.0, 90.0, fetch::ReqwestFetcher::new());
    // tile reads, decompression, parsing, and initial allocation stay outside every timer
    forcing
        .prepare_forcing(flat.clone(), date, 12.0, 0.25)
        .await?;
    landmask
        .load_landmask_tiles(flat.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let mut coordinates = VectorBuffer::default();
    coordinates.extend(points.iter().copied());
    let mut output = FieldBuffers::default();
    for hour in [12.0, 12.25] {
        forcing.sample_soa_into(
            coordinates.as_coordinates(),
            date,
            hour,
            ForcingFields::All,
            &mut output,
        );
        anyhow::ensure!(
            output.components.iter().flatten().all(|v| v.is_finite()),
            "missing {model} samples at {hour}"
        );
        for (name, fields) in [
            ("currents", ForcingFields::Currents),
            ("currents_wind", ForcingFields::CurrentsWind),
            ("all", ForcingFields::All),
        ] {
            let result = measure(repeats, || {
                forcing.sample_soa_into(
                    coordinates.as_coordinates(),
                    date,
                    hour,
                    fields,
                    &mut output,
                );
                black_box(&output);
            });
            println!(
                "{}",
                json!({"model":model,"particles":count,"phase":name,"hour":hour,"timing":result})
            );
        }
    }
    let result = preparation(&mut forcing, &points, date, repeats).await?;
    println!(
        "{}",
        json!({"model":model,"particles":count,"phase":"cached_prepare","timing":result})
    );
    let result = measure(repeats, || {
        let mut sum = 0.0;
        for &(_, lat) in black_box(&points) {
            sum += forcing.cell_area_m2(lat);
        }
        black_box(sum);
    });
    println!(
        "{}",
        json!({"model":model,"particles":count,"phase":"cell_area","timing":result})
    );

    for (scheme, diffusion, coefficients) in [
        ("euler", "constant", vec![0.0, 0.0]),
        ("euler", "constant", vec![10.0, 0.0]),
        ("rk4", "constant", vec![0.0, 0.0]),
        ("euler", "smagorinsky", vec![0.0, 0.1]),
    ] {
        let mut simulation = Simulation::new(
            "generic",
            r#"{"wind_factor":0.02,"wind_deflection":0}"#,
            r#"[{"lon":3,"lat":59,"radius":0,"schedule":[{"amount":1,"duration":0}]}]"#,
            count,
            96,
            scheme,
            diffusion,
            coefficients.clone(),
        );
        simulation.release_particles(0);
        let mut times = Vec::with_capacity(repeats);
        for i in 0..repeats + 5 {
            for (j, &(lon, lat)) in points.iter().enumerate() {
                simulation.particles.lons[j] = lon;
                simulation.particles.lats[j] = lat;
            }
            simulation.particles.stranded.fill(false);
            let start = Instant::now();
            simulation.update_particles_batch(1.0 / 96.0, &forcing, date, 12.0, &landmask);
            if i >= 5 {
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            black_box(&simulation.particles.lons);
            anyhow::ensure!(
                simulation
                    .particles
                    .lons
                    .iter()
                    .chain(&simulation.particles.lats)
                    .all(|v| v.is_finite()),
                "nonfinite {model} {scheme} result"
            );
            anyhow::ensure!(
                !simulation.particles.stranded.iter().any(|v| *v),
                "unexpected stranding"
            );
        }
        println!(
            "{}",
            json!({"model":model,"particles":count,"phase":"physics","scheme":scheme,"diffusion":diffusion,"coefficients":coefficients,"timing":summary(&mut times)})
        );
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 4, "usage: compare_forcing GLOBAL_ROOT NORKYST_ROOT LANDMASK_ROOT [YYYYMMDD] [REPEATS] [COUNTS comma separated]");
    let date = args
        .get(4)
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(20261105);
    let repeats = args.get(5).map(|v| v.parse()).transpose()?.unwrap_or(30);
    anyhow::ensure!(repeats > 0, "repeats must be positive");
    let counts: Vec<usize> = args
        .get(6)
        .map(String::as_str)
        .unwrap_or("1000,10000,100000")
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    anyhow::ensure!(counts.iter().all(|n| *n > 0), "counts must be positive");
    let manifest_path =
        url::Url::parse(&format!("{}/manifest.json", args[2].trim_end_matches('/')))?;
    // this diagnostic uses the actual projection source without changing the engine API
    let metadata: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(manifest_path.to_file_path().map_err(|_| {
            anyhow::anyhow!("NorKyst root must be a local file URL for projection timing")
        })?)
        .await?,
    )?;
    let projection: projection::PolarStereographic =
        serde_json::from_value(metadata["grid"]["projection"].clone())?;
    projection.validate().map_err(anyhow::Error::msg)?;
    for count in counts {
        let points = positions(count);
        let timing = measure(repeats, || {
            let mut sum = 0.0;
            let projection = black_box(&projection);
            for &(lon, lat) in black_box(&points) {
                let (x, y) = projection.project(lon as f64, lat as f64).unwrap();
                sum += x + y;
            }
            black_box(sum);
        });
        println!(
            "{}",
            json!({"model":"norkyst","particles":count,"phase":"projection_once","timing":timing})
        );
        for (model, root) in [("global", &args[1]), ("norkyst", &args[2])] {
            compare(model, root, &args[3], date, count, repeats).await?;
        }
    }
    Ok(())
}
