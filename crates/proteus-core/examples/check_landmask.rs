// check_landmask.rs compares local combined landmask tiles with reference CSV points

use proteus_core::{basemodel::LandMaskLoader, BoxFuture, FetchError, TileFetcher};

struct LocalFiles {
    polygon_only: bool,
}

impl TileFetcher for LocalFiles {
    fn fetch_bytes<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<Vec<u8>, FetchError>> {
        Box::pin(async move {
            if self.polygon_only && path.ends_with(".bin") {
                let mut bitmap = roaring::RoaringBitmap::new();
                bitmap.insert_range(0..2400 * 2400);
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&2400u32.to_le_bytes());
                bytes.extend_from_slice(&2400u32.to_le_bytes());
                bitmap.serialize_into(&mut bytes).unwrap();
                return Ok(bytes);
            }
            std::fs::read(path).map_err(|e| FetchError::Network(e.to_string()))
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: check_landmask tile_root points.csv [--polygon-only]".into());
    }
    let text = std::fs::read_to_string(&args[2])?;
    let points: Vec<(f32, f32, bool)> = text
        .lines()
        .skip(1)
        .map(|line| {
            let values: Vec<_> = line.split(',').collect();
            Ok((values[0].parse()?, values[1].parse()?, values[2] == "1"))
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    let mut loader = LandMaskLoader::new(
        &args[1],
        -180.0,
        -90.0,
        90.0,
        LocalFiles {
            polygon_only: args.get(3).is_some_and(|v| v == "--polygon-only"),
        },
    );
    loader
        .load_landmask_tiles(points.iter().flat_map(|&(x, y, _)| [x, y]).collect())
        .await?;
    let mut mismatches = 0;
    for &(x, y, expected) in &points {
        if loader.is_on_land(x, y) != expected {
            mismatches += 1;
            if mismatches <= 10 {
                eprintln!("mismatch at {x}, {y}, expected {expected}");
            }
        }
    }
    println!("{} points, {mismatches} mismatches", points.len());
    if mismatches != 0 {
        return Err("landmask comparison failed".into());
    }
    Ok(())
}
