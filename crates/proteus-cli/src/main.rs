mod config;
mod fetch;
mod tracer;
mod particle_output;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDateTime, Timelike};
use clap::Parser;
use config::{Config, ForcingModel, ReleasesSpec};
use fetch::ReqwestFetcher;
use proteus_core::basemodel::{LandMaskLoader, Simulation};
use proteus_core::forcing::{Forcing, ForcingKind};
use proteus_core::tracers::TracerKind;
use std::path::PathBuf;

const MIN_LON: f32 = -180.0;
const MIN_LAT: f32 = -80.0;

#[derive(Parser)]
#[command(name = "proteus")]
struct Args {
    /// Path to the JSON config file
    config: PathBuf,

    /// override the forcing model selected in the config
    #[arg(long, value_enum)]
    forcing_model: Option<ForcingModel>,

    /// stream all released particles to JSONL, relative to the config directory
    #[arg(long)]
    particle_output: Option<PathBuf>,

    /// particle snapshot interval in steps, always including initial and final states
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    snapshot_every: Option<u32>,
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
    let mut config: Config = serde_json::from_str(&config_text)
        .with_context(|| format!("failed to parse config: {}", config_path.display()))?;
    if let Some(model) = args.forcing_model {
        config.forcing_model = model;
    }
    if let Some(path) = args.particle_output { config.particle_output = Some(path); }
    if let Some(interval) = args.snapshot_every { config.snapshot_every = interval; }
    run(config, config_dir).await
}

async fn run(config: Config, config_dir: &std::path::Path) -> Result<()> {
    use std::io::{BufWriter, Write};
    anyhow::ensure!(config.snapshot_every > 0, "snapshot_every must be positive");
    let mut particle_file = if let Some(path) = &config.particle_output {
        let path = config_dir.join(path);
        anyhow::ensure!(path != config_dir.join(&config.output), "particle and centroid outputs must differ");
        Some(BufWriter::new(std::fs::File::create(&path)
            .with_context(|| format!("failed to create {}", path.display()))?))
    } else { None };
    let start_date = NaiveDateTime::parse_from_str(&config.start_date, "%Y-%m-%d %H:%M")
        .context("invalid start_date (expected YYYY-MM-DD HH:MM)")?;

    anyhow::ensure!(config.time_step_minutes.is_finite() && config.time_step_minutes > 0.0,
        "time_step_minutes must be finite and positive");
    let steps_per_day = (1440.0 / config.time_step_minutes) as u32;
    if steps_per_day == 0 {
        anyhow::bail!("time_step_minutes too large (must produce at least 1 step/day)");
    }

    let releases_json = resolve_releases(&config.releases, config_dir)?;
    let tracer_json = tracer::resolve(
        &config.tracer_type, &config.tracer_json, config_dir, config.catalog_dir.as_deref(),
    )?;

    let mut simulation = Simulation::new(
        &config.tracer_type,
        &tracer_json,
        &releases_json,
        config.particles,
        steps_per_day,
        &config.advection,
        &config.diffusion,
        config.diffusion_coeffs,
    );

    let base_url = resolve_tile_base_url(config.tile_url.as_deref(), config.forcing_model);
    let mut forcing = match config.forcing_model {
        ForcingModel::Global => ForcingKind::global(&base_url, MIN_LON, MIN_LAT, ReqwestFetcher::new()),
        ForcingModel::Norkyst => ForcingKind::norkyst(&base_url, ReqwestFetcher::new()),
    };
    log::info!("forcing model={:?} root={}", config.forcing_model, base_url);
    let mut landmask = LandMaskLoader::new(
        config.landmask_url.as_deref().unwrap_or("https://tiles.driftmap2d.com/roaring_landmask"),
        MIN_LON,
        -90.0,
        90.0,
        ReqwestFetcher::new(),
    );

    // One centroid per step, plus the initial seed position at step 0.
    let mut results: Vec<(usize, f32, f32, f32, f32)> =
        Vec::with_capacity(config.steps as usize + 1);
    let dt_days = 1.0 / steps_per_day as f32;

    for step_count in 0..=config.steps {
        // Step 0 is the seeding step: seed, record initial centroid, do not advance.
        if step_count == 0 {
            simulation.release_particles(0);
            if let Some(writer) = &mut particle_file {
                particle_output::write_snapshot(writer, 0, start_date, simulation.get_particles())?;
            }
            let (lon, lat) = centroid(&simulation);
            results.push((
                0,
                lon,
                lat,
                mass_weighted_evaporation(&simulation),
                mass_weighted_emulsification(&simulation),
            ));
            log::info!("step 0/{} centroid=({:.4}, {:.4})", config.steps, lon, lat);
            continue;
        }

        // Step N: advance from t = (N-1)*dt to t = N*dt.
        let elapsed = simulation_time(start_date, step_count - 1, steps_per_day);
        let date = elapsed.date();
        let current_date_int =
            date.year() as usize * 10000 + date.month() as usize * 100 + date.day() as usize;
        let hour = elapsed.hour() as f32 + elapsed.minute() as f32 / 60.0
            + elapsed.second() as f32 / 3600.0;

        simulation.release_particles(step_count);

        let positions = unstranded_flat(&simulation);
        forcing
            .prepare_forcing(positions.clone(), current_date_int, hour, dt_days * 24.0)
            .await
            .with_context(|| format!("failed to prepare forcing for step {step_count}"))?;
        forcing.validate_forcing_interval(current_date_int, hour, dt_days * 24.0)?;
        landmask.load_landmask_tiles(positions).await.map_err(anyhow::Error::msg)?;

        simulation.update_particles_batch(dt_days, &forcing, current_date_int, hour, &landmask);
        if particle_output::should_snapshot(step_count, config.steps, config.snapshot_every) {
            if let Some(writer) = &mut particle_file {
                particle_output::write_snapshot(writer, step_count,
                    simulation_time(start_date, step_count, steps_per_day), simulation.get_particles())?;
            }
        }

        let (lon, lat) = centroid(&simulation);
        results.push((
            step_count as usize,
            lon,
            lat,
            mass_weighted_evaporation(&simulation),
            mass_weighted_emulsification(&simulation),
        ));

        log::info!(
            "step {}/{} centroid=({:.4}, {:.4})",
            step_count,
            config.steps,
            lon,
            lat
        );
    }

    if let Some(writer) = &mut particle_file { writer.flush()?; }
    // Write output
    let out: Vec<serde_json::Value> = results
        .iter()
        .map(
            |(step, lon, lat, evaporation_percent, emulsification_percent)| {
                serde_json::json!({
                    "step": step,
                    "lon": lon,
                    "lat": lat,
                    "evaporation_percent": evaporation_percent,
                    "emulsification_percent": emulsification_percent
                })
            },
        )
        .collect();

    let output_path = config_dir.join(&config.output);
    std::fs::write(&output_path, serde_json::to_string_pretty(&out)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    log::info!(
        "wrote {} simulation results to {}",
        results.len(),
        output_path.display()
    );
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
        (
            (sum_lon / count as f64) as f32,
            (sum_lat / count as f64) as f32,
        )
    }
}

pub fn mass_weighted_evaporation(simulation: &Simulation) -> f32 {
    let particles = simulation.get_particles();
    let mut total_initial = 0.0;
    let mut total_evaporated = 0.0;

    if let TracerKind::Oil(oil) = &particles.tracer {
        for i in 0..particles.len {
            if !particles.stranded[i] {
                let initial_mass = simulation.release_manager.initial_mass_per_particle();
                total_initial += initial_mass;
                total_evaporated += initial_mass * oil.data.f_evap[i];
            }
        }
    }

    if total_initial > 0.0 {
        total_evaporated / total_initial * 100.0
    } else {
        0.0
    }
}

pub fn mass_weighted_emulsification(simulation: &Simulation) -> f32 {
    let particles = simulation.get_particles();
    let mut total_initial = 0.0;
    let mut total_emulsified = 0.0;

    if let TracerKind::Oil(oil) = &particles.tracer {
        for i in 0..particles.len {
            if !particles.stranded[i] {
                let initial_mass = simulation.release_manager.initial_mass_per_particle();
                total_initial += initial_mass;
                total_emulsified += initial_mass * oil.data.y_w[i];
            }
        }
    }

    if total_initial > 0.0 {
        total_emulsified / total_initial * 100.0
    } else {
        0.0
    }
}

/// Resolve the tile base URL from the config value.
///
/// - If `tile_url` already has a scheme (`file://`, `http://`, `https://`),
///   use it as-is. This supports local paths and custom servers.
/// - Otherwise, treat it as a path under the production CDN. So
///   `"validation-tiles"` becomes `"https://tiles.driftmap2d.com/validation-tiles"`.
fn resolve_tile_base_url(tile_url: Option<&str>, model: ForcingModel) -> String {
    let tile_url = tile_url.unwrap_or(match model {
        ForcingModel::Global => "tiles",
        ForcingModel::Norkyst => "norkyst",
    }).trim().trim_end_matches('/');
    if tile_url.starts_with("file://")
        || tile_url.starts_with("http://")
        || tile_url.starts_with("https://")
    {
        tile_url.to_string()
    } else {
        format!("https://tiles.driftmap2d.com/{}", tile_url.trim_start_matches('/'))
    }
}

fn simulation_time(start: NaiveDateTime, step: u32, steps_per_day: u32) -> NaiveDateTime {
    let seconds = (step as f64 * 86400.0 / steps_per_day as f64).round() as i64;
    start + chrono::Duration::seconds(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_value() -> serde_json::Value {
        serde_json::json!({
            "start_date": "2026-10-23 00:30", "tracer_type": "generic",
            "releases": [{"lon": 4.62534, "lat": 60.16331, "radius": 0.0,
                "schedule": [{"amount": 1, "duration": 0}]}],
            "particles": 1, "time_step_minutes": 15, "steps": 1
        })
    }

    #[test]
    fn model_defaults_and_legacy_tile_prefixes_are_preserved() {
        let mut value = config_value();
        let config: Config = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(config.forcing_model, ForcingModel::Global);
        assert_eq!(resolve_tile_base_url(config.tile_url.as_deref(), config.forcing_model),
            "https://tiles.driftmap2d.com/tiles");
        value["forcing_model"] = serde_json::json!("norkyst");
        let config: Config = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(resolve_tile_base_url(config.tile_url.as_deref(), config.forcing_model),
            "https://tiles.driftmap2d.com/norkyst");
        value["forcing_model"] = serde_json::json!("unknown");
        assert!(serde_json::from_value::<Config>(value).is_err());
        assert_eq!(resolve_tile_base_url(Some("validation-tiles"), ForcingModel::Global),
            "https://tiles.driftmap2d.com/validation-tiles");
    }

    #[test]
    fn explicit_roots_are_not_given_an_extra_model_prefix() {
        for (input, expected) in [
            ("norkyst/", "https://tiles.driftmap2d.com/norkyst"),
            ("https://example.com/custom/native/", "https://example.com/custom/native"),
            ("file:///D:/forcing/norkyst/", "file:///D:/forcing/norkyst"),
        ] {
            assert_eq!(resolve_tile_base_url(Some(input), ForcingModel::Norkyst), expected);
        }
        let args = Args::try_parse_from(["proteus", "config.json", "--forcing-model", "norkyst"]).unwrap();
        assert_eq!(args.forcing_model, Some(ForcingModel::Norkyst));
    }

    #[test]
    fn forcing_clock_keeps_start_time_and_crosses_midnight() {
        let start = NaiveDateTime::parse_from_str("2026-10-23 23:45", "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(simulation_time(start, 0, 96), start);
        assert_eq!(simulation_time(start, 1, 96).to_string(), "2026-10-24 00:00:00");
        assert_eq!(simulation_time(start, 2, 96).to_string(), "2026-10-24 00:15:00");
    }

    #[tokio::test]
    async fn native_loader_reads_file_manifest_and_projected_daily_tiles() {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/cli-norkyst-tests");
        std::fs::create_dir_all(&target).unwrap();
        let target = target.canonicalize().unwrap();
        let root = target.join(format!("{}", std::process::id()));
        let day = root.join("2026/10/23");
        std::fs::create_dir_all(&day).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../proteus-core/src/forcing/norkyst/fixtures");
        for name in ["004_005.bin", "005_005.bin"] {
            std::fs::copy(fixture.join(name), day.join(name)).unwrap();
        }
        std::fs::copy(fixture.join("manifest.json"), root.join("manifest.json")).unwrap();
        let root_url = url::Url::from_directory_path(&root).unwrap();
        let base = resolve_tile_base_url(Some(root_url.as_str()), ForcingModel::Norkyst);
        let mut forcing = ForcingKind::norkyst(&base, ReqwestFetcher::new());
        let position = (4.62534, 60.16331);
        forcing.prepare_forcing(vec![position.0, position.1], 20261023, 0.5, 0.25).await.unwrap();
        let values = forcing.get_wind_sst([position].into_iter(), 20261023, 0.5);
        assert_eq!(values[0], (0.0, 4.0, 280.5));
        assert!(forcing.validate_forcing_interval(20261023, 1.5, 1.0).is_err());
        assert!(root.starts_with(target));
        std::fs::remove_dir_all(root).unwrap();
    }
}
