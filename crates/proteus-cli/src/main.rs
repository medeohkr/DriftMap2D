mod config;
mod fetch;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDateTime};
use clap::Parser;
use config::{Config, ReleasesSpec};
use fetch::ReqwestFetcher;
use proteus_core::basemodel::{DataLoader, LandMaskLoader, Simulation};
use std::path::PathBuf;

const MIN_LON: f32 = -180.0;
const MIN_LAT: f32 = -80.0;

#[derive(Parser)]
#[command(name = "proteus")]
struct Args {
    /// Path to the JSON config file
    config: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let config_path = args
        .config
        .canonicalize()
        .with_context(|| format!("config file not found: {}", args.config.display()))?;
    let config_dir = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));

    let config_text = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config: {}", config_path.display()))?;
    let config: Config = serde_json::from_str(&config_text)
        .with_context(|| format!("failed to parse config: {}", config_path.display()))?;

    run(config, config_dir).await
}

async fn run(config: Config, config_dir: &std::path::Path) -> Result<()> {
    let start_date = NaiveDateTime::parse_from_str(&config.start_date, "%Y-%m-%d %H:%M")
        .context("invalid start_date (expected YYYY-MM-DD HH:MM)")?;

    let steps_per_day = (1440.0 / config.time_step_minutes) as u32;
    if steps_per_day == 0 {
        anyhow::bail!("time_step_minutes too large (must produce at least 1 step/day)");
    }

    let releases_json = resolve_releases(&config.releases, config_dir)?;

    let mut simulation = Simulation::new(
        &config.tracer_type,
        &config.tracer_json,
        &releases_json,
        config.particles,
        steps_per_day,
        &config.advection,
        &config.diffusion,
        config.diffusion_coeffs,
    );

    let mut loader = DataLoader::new(
        &format!("{}", config.base_url),
        MIN_LON,
        MIN_LAT,
        ReqwestFetcher::new(),
    );
    let mut landmask = LandMaskLoader::new(
        &format!("{}/roaring_landmask", config.base_url),
        MIN_LON,
        -90.0,
        90.0,
        ReqwestFetcher::new(),
    );

    let mut centroids: Vec<(usize, f32, f32)> = Vec::with_capacity(config.steps as usize);
    let dt_days = 1.0 / steps_per_day as f32;

    for step_count in 0..config.steps {
        let day_offset = step_count / steps_per_day;
        let date = start_date.date() + chrono::Duration::days(day_offset as i64);
        let current_date_int =
            date.year() as usize * 10000 + date.month() as usize * 100 + date.day() as usize;
        let hour = (24.0 * step_count as f32 / steps_per_day as f32) % 24.0;

        simulation.release_particles(step_count);

        let positions = unstranded_flat(&simulation);
        loader
            .load_ocean_tiles(positions.clone(), current_date_int)
            .await;
        landmask.load_landmask_tiles(positions).await;

        simulation.update_particles_batch(dt_days, &loader, current_date_int, hour, &landmask);

        let (lon, lat) = centroid(&simulation);
        centroids.push((step_count as usize, lon, lat));

        log::info!(
            "step {}/{} centroid=({:.3}, {:.3})",
            step_count + 1,
            config.steps,
            lon,
            lat
        );
    }

    // Write output
    let out: Vec<serde_json::Value> = centroids
        .iter()
        .map(|(step, lon, lat)| {
            serde_json::json!({ "step": step, "lon": lon, "lat": lat })
        })
        .collect();

    let output_path = config_dir.join(&config.output);
    std::fs::write(&output_path, serde_json::to_string_pretty(&out)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    log::info!("wrote {} centroids to {}", centroids.len(), output_path.display());
    Ok(())
}

fn resolve_releases(spec: &ReleasesSpec, config_dir: &std::path::Path) -> Result<String> {
    match spec {
        ReleasesSpec::Inline(v) => Ok(serde_json::to_string(v)?),
        ReleasesSpec::Path(p) => {
            let path = if std::path::Path::new(p).is_absolute() {
                std::path::PathBuf::from(p)
            } else {
                config_dir.join(p)
            };
            std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read releases file: {}", path.display()))
        }
    }
}

fn unstranded_flat(simulation: &Simulation) -> Vec<f32> {
    let p = simulation.get_particles();
    let mut out = Vec::with_capacity(p.len * 2);
    for i in 0..p.len {
        if !p.stranded[i] {
            out.push(p.lons[i]);
            out.push(p.lats[i]);
        }
    }
    out
}

fn centroid(simulation: &Simulation) -> (f32, f32) {
    let p = simulation.get_particles();
    let mut sum_lon = 0.0_f64;
    let mut sum_lat = 0.0_f64;
    let mut count = 0_usize;

    for i in 0..p.len {
        if !p.stranded[i] {
            sum_lon += p.lons[i] as f64;
            sum_lat += p.lats[i] as f64;
            count += 1;
        }
    }

    if count == 0 {
        (0.0, 0.0)
    } else {
        ((sum_lon / count as f64) as f32, (sum_lat / count as f64) as f32)
    }
}