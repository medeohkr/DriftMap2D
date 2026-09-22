use anyhow::{Context, Result};
use clap::Parser;
use roaring::{RoaringBitmap, RoaringTreemap};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Grid dimensions of the source GSHHG binary mask.
/// GSHHG mask is 1/240° resolution: 86400 × 43200 cells.
const NX: u64 = 86400;
const NY: u64 = 43200;

/// Tile size in cells. 2400 cells @ 1/240° = 10° per tile.
const TILE_SIZE: u64 = 2400;

/// Number of tiles. 86400 / 2400 = 36 in x, 43200 / 2400 = 18 in y.
const N_TILES_X: u64 = 36;
const N_TILES_Y: u64 = 18;

#[derive(Parser, Debug)]
#[command(
    name = "roaring-landmask",
    about = "Tile a GSHHG RoaringTreemap landmask into 10° × 10° RoaringBitmap tiles."
)]
struct Args {
    /// Path to the source GSHHG binary mask (.tbmap, a serialized RoaringTreemap).
    /// Can also be set via the GSHHG_MASK_PATH environment variable.
    #[arg(
        long,
        env = "GSHHG_MASK_PATH",
        default_value = "data/gshhg_mask.tbmap"
    )]
    mask: PathBuf,

    /// Output directory for the tiled landmask files.
    #[arg(long, default_value = "data/roaring_landmask_tiled")]
    output: PathBuf,

    /// Verbose output: print every tile written.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let file = File::open(&args.mask).with_context(|| {
        format!(
            "Failed to open GSHHG mask at {}\n\
             Hint: pass --mask <PATH> or set GSHHG_MASK_PATH",
            args.mask.display()
        )
    })?;

    eprintln!("Loading landmask from {}...", args.mask.display());
    let global = RoaringTreemap::deserialize_from(file)
        .context("Failed to deserialize RoaringTreemap from mask file")?;

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Failed to create output dir {}", args.output.display()))?;

    for tx in 0..N_TILES_X {
        for ty in 0..N_TILES_Y {
            let mut tile = RoaringBitmap::new();

            let start_x = tx * TILE_SIZE;
            let start_y = ty * TILE_SIZE;

            for iy in 0..TILE_SIZE {
                let global_y = start_y + iy;
                for ix in 0..TILE_SIZE {
                    let global_x = start_x + ix;
                    let global_idx = global_y * NX + global_x;

                    if global.contains(global_idx) {
                        let local_idx = (iy * TILE_SIZE + ix) as u32;
                        tile.insert(local_idx);
                    }
                }
            }

            let path = args
                .output
                .join(format!("landmask_{:03}_{:03}.bin", tx, ty));

            let mut writer = BufWriter::new(
                File::create(&path)
                    .with_context(|| format!("Failed to create {}", path.display()))?,
            );

            // Header: tile dimensions in cells (u32 LE each)
            writer.write_all(&(TILE_SIZE as u32).to_le_bytes())?;
            writer.write_all(&(TILE_SIZE as u32).to_le_bytes())?;

            tile.serialize_into(&mut writer)
                .with_context(|| format!("Failed to serialize tile {}", path.display()))?;

            if args.verbose {
                println!("Written tile {}/{} -> {}", tx, ty, path.display());
            }
        }
    }

    eprintln!(
        "Done. {} tiles written to {}",
        N_TILES_X * N_TILES_Y,
        args.output.display()
    );

    Ok(())
}