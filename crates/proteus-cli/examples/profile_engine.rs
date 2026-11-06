//! profile_engine.rs measures cached engine work without rendering or trajectory output

#[path = "../src/fetch.rs"]
mod fetch;

use anyhow::Result;
use proteus_core::{
    basemodel::{LandMaskLoader, Simulation},
    forcing::{Forcing, ForcingKind},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    time::Instant,
};

thread_local! {
    static ALLOCATIONS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

struct CountingAllocator;

fn count(bytes: usize) {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some((calls, requested)) = counter.get() {
            counter.set(Some((calls + 1, requested + bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        System.alloc_zeroed(layout)
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        System.realloc(pointer, layout, size)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCATIONS.with(|counter| counter.set(Some((0, 0))));
    let result = operation();
    let (calls, bytes) = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
    (result, calls, bytes)
}

fn flat(simulation: &Simulation) -> Vec<f32> {
    simulation
        .particles
        .view()
        .iter()
        .flat_map(|(_, x, y)| [x, y])
        .collect()
}

#[inline(never)]
fn dense_advection(lons: &mut [f32], velocity: &[f32], dt: f32) {
    for (lon, speed) in lons.iter_mut().zip(velocity) {
        *lon += dt * speed;
    }
}

#[inline(never)]
fn indexed_advection(lons: &mut [f32], velocity: &[f32], indices: &[usize], dt: f32) {
    for (i, &index) in indices.iter().enumerate() {
        lons[index] += dt * velocity[i];
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let particles: usize = args
        .get(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(100_000);
    let iterations: usize = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(100);
    let kind = args.get(3).map(String::as_str).unwrap_or("generic");
    let scheme = args.get(4).map(String::as_str).unwrap_or("euler");
    let root = args
        .get(5)
        .map(String::as_str)
        .unwrap_or("https://tiles.driftmap2d.com/norkyst");
    let land_root = args
        .get(6)
        .map(String::as_str)
        .unwrap_or("https://tiles.driftmap2d.com/roaring_landmask");
    let date: usize = args
        .get(7)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(20261105);
    anyhow::ensure!(
        particles > 0 && iterations > 0,
        "particles and iterations must be positive"
    );
    anyhow::ensure!(
        matches!(kind, "generic" | "sar"),
        "tracer must be generic or sar"
    );
    anyhow::ensure!(
        matches!(scheme, "euler" | "rk2" | "rk4"),
        "integrator must be euler, rk2, or rk4"
    );
    let tracer = match kind {
        "sar" => {
            r#"{"downwind":[0.96,0,12],"right":[0.54,0,9.4],"left":[-0.54,0,9.4],"jibe_probability":0,"capsizing":false,"capsize_threshold":30,"capsize_fraction":0.4,"capsize_sigma":5}"#
        }
        _ => r#"{"wind_factor":0.02,"wind_deflection":0}"#,
    };
    let mut simulation = Simulation::new(
        kind,
        tracer,
        r#"[{"lon":3,"lat":59,"radius":500,"schedule":[{"amount":1,"duration":0}]}]"#,
        particles,
        96,
        scheme,
        "constant",
        vec![0.0, 0.0],
    );
    simulation.release_particles(0);
    let original_lons = simulation.particles.lons.clone();
    let original_lats = simulation.particles.lats.clone();
    let mut forcing = ForcingKind::norkyst(root, fetch::ReqwestFetcher::new());
    let mut landmask =
        LandMaskLoader::new(land_root, -180.0, -90.0, 90.0, fetch::ReqwestFetcher::new());
    let setup = Instant::now();
    forcing
        .prepare_forcing(flat(&simulation), date, 12.0, 0.25)
        .await?;
    landmask
        .load_landmask_tiles(flat(&simulation))
        .await
        .map_err(anyhow::Error::msg)?;
    eprintln!(
        "loaded forcing and landmask in {:.3}s, excluded from measurements",
        setup.elapsed().as_secs_f64()
    );
    let restore = |simulation: &mut Simulation| {
        simulation.particles.lons.copy_from_slice(&original_lons);
        simulation.particles.lats.copy_from_slice(&original_lats);
        simulation.particles.stranded.fill(false);
    };
    for _ in 0..5 {
        restore(&mut simulation);
        simulation.update_particles_batch(1.0 / 96.0, &forcing, date, 12.0, &landmask);
    }
    let mut times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        restore(&mut simulation);
        let started = Instant::now();
        simulation.update_particles_batch(1.0 / 96.0, &forcing, date, 12.0, &landmask);
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        black_box(&simulation.particles.lons);
    }
    times.sort_by(f64::total_cmp);
    restore(&mut simulation);
    let (_, calls, bytes) = allocations(|| {
        simulation.update_particles_batch(1.0 / 96.0, &forcing, date, 12.0, &landmask)
    });
    let (_, release_calls, release_bytes) = allocations(|| simulation.release_particles(1));
    println!("kind={kind} integrator={scheme} particles={particles} repeats={iterations}");
    println!(
        "physics median_ms={:.4} p90_ms={:.4} allocation_calls={calls} requested_bytes={bytes}",
        times[iterations / 2],
        times[(iterations * 9 / 10).min(iterations - 1)]
    );
    println!("empty_release allocation_calls={release_calls} requested_bytes={release_bytes}");
    let positions: Vec<_> = simulation
        .particles
        .view()
        .iter()
        .map(|(_, x, y)| (x, y))
        .collect();
    for (name, currents) in [("wind_temperature", false), ("currents_wind", true)] {
        let start = Instant::now();
        for _ in 0..iterations {
            if currents {
                black_box(forcing.get_velocities_wind(positions.iter().copied(), date, 12.0));
            } else {
                black_box(forcing.get_wind_sst(positions.iter().copied(), date, 12.0));
            }
        }
        println!(
            "{name} mean_ms={:.4}",
            start.elapsed().as_secs_f64() * 1000.0 / iterations as f64
        );
        let (_, calls, bytes) = allocations(|| {
            if currents {
                black_box(forcing.get_velocities_wind(positions.iter().copied(), date, 12.0));
            } else {
                black_box(forcing.get_wind_sst(positions.iter().copied(), date, 12.0));
            }
        });
        println!("{name} allocation_calls={calls} requested_bytes={bytes}");
    }
    let start = Instant::now();
    for _ in 0..iterations {
        forcing
            .prepare_forcing(flat(&simulation), date, 12.0, 0.25)
            .await?;
    }
    println!(
        "cached_prepare mean_ms={:.4}",
        start.elapsed().as_secs_f64() * 1000.0 / iterations as f64
    );
    let start = Instant::now();
    for _ in 0..iterations {
        landmask
            .load_landmask_tiles(flat(&simulation))
            .await
            .map_err(anyhow::Error::msg)?;
    }
    println!(
        "cached_landmask_prepare mean_ms={:.4}",
        start.elapsed().as_secs_f64() * 1000.0 / iterations as f64
    );
    let mut lons = vec![3.0; particles];
    let velocity = vec![0.000001; particles];
    let indices: Vec<_> = (0..particles).collect();
    for dense in [true, false] {
        let start = Instant::now();
        for _ in 0..iterations * 100 {
            if dense {
                dense_advection(black_box(&mut lons), black_box(&velocity), black_box(1.0));
            } else {
                indexed_advection(
                    black_box(&mut lons),
                    black_box(&velocity),
                    black_box(&indices),
                    black_box(1.0),
                );
            }
        }
        println!(
            "{}_advection mean_us={:.4}",
            if dense { "dense" } else { "indexed" },
            start.elapsed().as_secs_f64() * 1e6 / (iterations * 100) as f64
        );
    }
    Ok(())
}
