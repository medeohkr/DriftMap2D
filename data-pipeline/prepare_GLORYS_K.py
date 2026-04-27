#!/usr/bin/env python3
"""
PROTEUS Monthly K-Value Calculator with Tiled Output
Following Klocker et al. 2012:
- Pass 1: Compute 3-year means (u_mean, v_mean) from tiled GLORYS data
- Pass 2: For each month, compute monthly mean anomalies → monthly EKE → monthly K
- Uses monthly global eddy atlas (radius + phase speed)
- Outputs tiled K fields matching GLORYS tile structure
"""

import numpy as np
import struct
import json
from pathlib import Path
from datetime import datetime, timedelta
import gc
from tqdm import tqdm
from concurrent.futures import ProcessPoolExecutor, as_completed
import argparse
from functools import partial

# ===== CONFIGURATION =====

# Tile configuration
TILE_SIZE = 5.0                     # degrees
LON_STEP = 1/12                     # 0.08333 degrees
LAT_STEP = 1/12

# GLORYS global grid bounds
LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP
LAT_MIN = -80.0
LAT_MAX = 90.0

# Number of tiles
N_LON_TILES = int(360 / TILE_SIZE)   # 72
N_LAT_TILES = int(170 / TILE_SIZE)   # 34

# Global GLORYS grid dimensions (from your NetCDF)
N_LON_GLOBAL = 4320
N_LAT_GLOBAL = 2041

# Depth levels (meters)
DEPTH_VALUES = [0, 40, 92, 318]
N_DEPTH = len(DEPTH_VALUES)

# Klocker parameters
MIXING_EFFICIENCY = 0.35
G_OVER_K = 0.03

# Date range
START_DATE = datetime(2011, 1, 1)
END_DATE = datetime(2013, 12, 31)
BASE_DATE = START_DATE

# Directories
GLORYS_TILES_DIR = Path(r"web/glorys_tiles")  # Your tiled GLORYS data
EDDY_DIR = Path(r"data/eddy_atlas_global/monthly")  # Monthly eddy data
OUTPUT_DIR = Path(r"data/k_field_tiles_global_monthly")

# Create output directory
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

# Processing parameters
N_WORKERS = 4


# ===== UTILITY FUNCTIONS =====

def get_tile_bounds(lon_idx, lat_idx):
    """Get tile boundaries for given tile indices."""
    lon_min = LON_MIN + lon_idx * TILE_SIZE
    lon_max = min(lon_min + TILE_SIZE, LON_MAX)
    lat_min = LAT_MIN + lat_idx * TILE_SIZE
    lat_max = min(lat_min + TILE_SIZE, LAT_MAX)
    return lon_min, lon_max, lat_min, lat_max


def read_glorys_tile(tile_path):
    """
    Read a GLORYS tile binary file.
    Returns: u, v, n_lon, n_lat, n_depth, depths
    """
    with open(tile_path, 'rb') as f:
        n_lon = struct.unpack('<I', f.read(4))[0]
        n_lat = struct.unpack('<I', f.read(4))[0]
        n_depth = struct.unpack('<I', f.read(4))[0]
        
        depths = [struct.unpack('<f', f.read(4))[0] for _ in range(n_depth)]
        
        u = np.frombuffer(f.read(n_depth * n_lat * n_lon * 2), dtype=np.float16)
        v = np.frombuffer(f.read(n_depth * n_lat * n_lon * 2), dtype=np.float16)
        
        u = u.reshape((n_depth, n_lat, n_lon)).astype(np.float32)
        v = v.reshape((n_depth, n_lat, n_lon)).astype(np.float32)
        
        return u, v, n_lon, n_lat, n_depth, depths


def read_eddy_monthly(eddy_path):
    """
    Read monthly gridded eddy data (global grid).
    Returns: radius_km (N_LAT_GLOBAL, N_LON_GLOBAL), speed_ms (N_LAT_GLOBAL, N_LON_GLOBAL)
    """
    with open(eddy_path, 'rb') as f:
        header = struct.unpack('3i', f.read(12))  # version, year, month
        version, year, month = header
        
        data = np.frombuffer(f.read(), dtype=np.float32)
        
        half = len(data) // 2
        radius_km = data[:half].reshape((N_LAT_GLOBAL, N_LON_GLOBAL))
        speed_ms = data[half:].reshape((N_LAT_GLOBAL, N_LON_GLOBAL))
        
        return radius_km, speed_ms


def write_k_tile(output_path, k_data, n_lon, n_lat, n_depth, depths):
    """Write a K tile binary file with safe float16 conversion."""
    # Clip K values to valid float16 range
    k_data_clipped = np.clip(k_data, -65000, 65000)
    k_data_clipped = np.nan_to_num(k_data_clipped, nan=0.0, posinf=65000, neginf=-65000)
    
    with open(output_path, 'wb') as f:
        f.write(struct.pack('<I', n_lon))
        f.write(struct.pack('<I', n_lat))
        f.write(struct.pack('<I', n_depth))
        for depth_val in depths:
            f.write(struct.pack('<f', float(depth_val)))
        k_data_clipped.astype(np.float16).tofile(f)


def extract_tile_from_global_grid(grid_data, lon_idx, lat_idx):
    """
    Extract a tile from global gridded eddy data based on tile indices.
    """
    # Get tile bounds in degrees
    lon_min, lon_max, lat_min, lat_max = get_tile_bounds(lon_idx, lat_idx)
    
    # Create global coordinate arrays
    lon_grid_1d = np.linspace(LON_MIN, LON_MAX, N_LON_GLOBAL)
    lat_grid_1d = np.linspace(LAT_MIN, LAT_MAX, N_LAT_GLOBAL)
    
    # Find indices within this tile
    # Handle longitude wrap-around
    if lon_min < 0 and lon_max > 0:
        # Tile crosses 0° meridian (unlikely for 5° tiles, but handle)
        lon_indices = np.where((lon_grid_1d >= lon_min) | (lon_grid_1d < lon_max))[0]
    else:
        lon_indices = np.where((lon_grid_1d >= lon_min) & (lon_grid_1d < lon_max))[0]
    
    lat_indices = np.where((lat_grid_1d >= lat_min) & (lat_grid_1d < lat_max))[0]
    
    if len(lat_indices) == 0 or len(lon_indices) == 0:
        return None, 0, 0
    
    tile_data = grid_data[np.ix_(lat_indices, lon_indices)]
    return tile_data, len(lon_indices), len(lat_indices)


# ===== PASS 1: Compute 3-year means from tiles =====

def compute_tile_mean(tile_info, glorys_tiles_dir, years):
    """
    Compute mean u, v for a single tile across multiple years.
    """
    lon_idx, lat_idx = tile_info
    tile_pattern = f"{lon_idx:03d}_{lat_idx:03d}.bin"
    
    u_sum = None
    v_sum = None
    count = 0
    n_lon = n_lat = n_depth = None
    depths = None
    
    for year in years:
        for month in range(1, 13):
            month_dir = glorys_tiles_dir / f"{year:04d}" / f"{month:02d}"
            if not month_dir.exists():
                continue
            
            # Check each day in month
            for day_dir in month_dir.iterdir():
                if day_dir.is_dir():
                    tile_path = day_dir / tile_pattern
                    if tile_path.exists():
                        try:
                            u, v, n_lo, n_la, n_d, d = read_glorys_tile(tile_path)
                            
                            if u_sum is None:
                                n_lon, n_lat, n_depth = n_lo, n_la, n_d
                                depths = d
                                u_sum = np.zeros((n_depth, n_lat, n_lon), dtype=np.float64)
                                v_sum = np.zeros((n_depth, n_lat, n_lon), dtype=np.float64)
                            
                            u_sum += u
                            v_sum += v
                            count += 1
                        except Exception as e:
                            continue
    
    if count > 0:
        u_mean = (u_sum / count).astype(np.float32)
        v_mean = (v_sum / count).astype(np.float32)
        return (lon_idx, lat_idx, u_mean, v_mean, n_lon, n_lat, n_depth, depths, count)
    return None


def compute_all_means(glorys_tiles_dir, output_dir, years, n_workers=4):
    """Compute 3-year means for all tiles."""
    print("\n" + "="*70)
    print("📊 PASS 1: Computing 3-year means from GLORYS tiles...")
    print("="*70)
    print(f"GLORYS tiles directory: {glorys_tiles_dir.absolute()}")
    print(f"Output directory: {output_dir.absolute()}")
    
    all_tiles = [(lon_idx, lat_idx) 
                 for lon_idx in range(N_LON_TILES) 
                 for lat_idx in range(N_LAT_TILES)]
    
    print(f"Processing {len(all_tiles)} tiles across years {years[0]}-{years[-1]}...")
    
    means_by_tile = {}
    with ProcessPoolExecutor(max_workers=n_workers) as executor:
        future_to_tile = {
            executor.submit(compute_tile_mean, tile, glorys_tiles_dir, years): tile 
            for tile in all_tiles
        }
        
        for future in tqdm(as_completed(future_to_tile), total=len(all_tiles), desc="Computing means"):
            result = future.result()
            if result is not None:
                lon_idx, lat_idx, u_mean, v_mean, n_lon, n_lat, n_depth, depths, count = result
                means_by_tile[(lon_idx, lat_idx)] = {
                    'u_mean': u_mean,
                    'v_mean': v_mean,
                    'n_lon': n_lon,
                    'n_lat': n_lat,
                    'n_depth': n_depth,
                    'depths': depths,
                    'count': count
                }
    
    # Save means for reuse
    means_path = output_dir / "3yr_means_tiled.npz"
    print(f"\n💾 Saving means to: {means_path.absolute()}")
    np.savez(means_path, 
             means_by_tile=means_by_tile,
             n_lon_tiles=N_LON_TILES,
             n_lat_tiles=N_LAT_TILES,
             tile_size=TILE_SIZE,
             depth_values=DEPTH_VALUES)
    
    if means_path.exists():
        print(f"✓ Successfully saved! File size: {means_path.stat().st_size / 1024 / 1024:.2f} MB")
    
    print(f"\n✓ Computed means for {len(means_by_tile)} tiles")
    
    return means_by_tile


# ===== PASS 2: Compute monthly K =====

def process_month_tile(tile_info, year, month, glorys_tiles_dir, eddy_dir, means_by_tile):
    """
    Process a single tile for a specific month.
    """
    lon_idx, lat_idx = tile_info
    
    # Check if we have mean data for this tile
    if (lon_idx, lat_idx) not in means_by_tile:
        return None
    
    mean_data = means_by_tile[(lon_idx, lat_idx)]
    
    # Accumulate u, v for this month
    u_month_sum = None
    v_month_sum = None
    days_count = 0
    n_lon = n_lat = n_depth = None
    depths = None
    
    # Get all days in this month
    month_start = datetime(year, month, 1)
    if month == 12:
        month_end = datetime(year + 1, 1, 1)
    else:
        month_end = datetime(year, month + 1, 1)
    
    current_date = month_start
    while current_date < month_end:
        day_dir = glorys_tiles_dir / f"{year:04d}" / f"{month:02d}" / f"{current_date.day:02d}"
        tile_path = day_dir / f"{lon_idx:03d}_{lat_idx:03d}.bin"
        
        if tile_path.exists():
            try:
                u, v, n_lo, n_la, n_d, d = read_glorys_tile(tile_path)
                
                if u_month_sum is None:
                    n_lon, n_lat, n_depth = n_lo, n_la, n_d
                    depths = d
                    u_month_sum = np.zeros((n_depth, n_lat, n_lon), dtype=np.float64)
                    v_month_sum = np.zeros((n_depth, n_lat, n_lon), dtype=np.float64)
                
                u_month_sum += u
                v_month_sum += v
                days_count += 1
            except Exception as e:
                pass
        
        current_date += timedelta(days=1)
    
    if days_count == 0:
        return None
    
    # Compute monthly means
    u_month_mean = (u_month_sum / days_count).astype(np.float32)
    v_month_mean = (v_month_sum / days_count).astype(np.float32)
    
    # Get 3-year means
    u_mean = mean_data['u_mean']
    v_mean = mean_data['v_mean']
    
    # Compute anomalies (monthly mean - 3-year mean)
    u_prime = u_month_mean - u_mean
    v_prime = v_month_mean - v_mean
    
    # Compute monthly EKE
    eke_monthly = 0.5 * (u_prime**2 + v_prime**2)
    
    # Get monthly mean eddy data
    eddy_file = eddy_dir / f"eddy_{year:04d}{month:02d}.bin"
    if not eddy_file.exists():
        return None
    
    radius_global, speed_global = read_eddy_monthly(eddy_file)
    
    # Extract tile from global eddy grid
    radius_tile, tile_n_lon, tile_n_lat = extract_tile_from_global_grid(radius_global, lon_idx, lat_idx)
    speed_tile, _, _ = extract_tile_from_global_grid(speed_global, lon_idx, lat_idx)
    
    if radius_tile is None:
        return None
    
    # Ensure dimensions match
    if radius_tile.shape != (n_lat, n_lon):
        from scipy.ndimage import zoom
        zoom_factor = (n_lat / radius_tile.shape[0], n_lon / radius_tile.shape[1])
        radius_tile = zoom(radius_tile, zoom_factor, order=0)
        speed_tile = zoom(speed_tile, zoom_factor, order=0)
    
    # Convert radius to meters
    L_m = radius_tile * 1000
    
    # Precompute grid-scale quantities
    k = 2 * np.pi / np.maximum(L_m, 1e-6)
    g = G_OVER_K * k
    
    # Compute U (mean flow speed)
    U_mean_tile = np.sqrt(u_mean**2 + v_mean**2)
    
    # Initialize K array
    K_monthly = np.zeros((n_depth, n_lat, n_lon), dtype=np.float32)
    
    # Compute K for each depth
    for depth_idx in range(n_depth):
        U_depth = U_mean_tile[depth_idx]
        eke_depth = eke_monthly[depth_idx]
        
        # Unsuppressed diffusivity: K0 = C * sqrt(2*EKE) * L
        K0 = MIXING_EFFICIENCY * np.sqrt(2 * np.maximum(eke_depth, 0)) * L_m
        
        # Suppression factor
        rel_speed = np.abs(speed_tile - U_depth)
        suppression = 1.0 / (1.0 + (k**2 * rel_speed**2) / (g**2 + 1e-10))
        
        K_monthly[depth_idx] = K0 * suppression
    
    return (lon_idx, lat_idx, K_monthly, n_lon, n_lat, n_depth, depths)


def process_month(year, month, glorys_tiles_dir, eddy_dir, output_dir, means_by_tile, n_workers=4):
    """Process all tiles for a specific month."""
    print(f"\n📅 Processing {year:04d}-{month:02d}...")
    
    # Create output directory
    month_dir = output_dir / f"{year:04d}" / f"{month:02d}"
    month_dir.mkdir(parents=True, exist_ok=True)
    
    # Process all tiles that have mean data
    active_tiles = list(means_by_tile.keys())
    tiles_written = 0
    
    with ProcessPoolExecutor(max_workers=n_workers) as executor:
        process_func = partial(process_month_tile, 
                               year=year, 
                               month=month,
                               glorys_tiles_dir=glorys_tiles_dir,
                               eddy_dir=eddy_dir,
                               means_by_tile=means_by_tile)
        
        futures = {executor.submit(process_func, tile): tile for tile in active_tiles}
        
        for future in tqdm(as_completed(futures), total=len(active_tiles), desc="  Processing tiles", leave=False):
            result = future.result()
            if result is not None:
                lon_idx, lat_idx, k_data, n_lon, n_lat, n_depth, depths = result
                
                # Write K tile
                output_path = month_dir / f"{lon_idx:03d}_{lat_idx:03d}.bin"
                write_k_tile(output_path, k_data, n_lon, n_lat, n_depth, depths)
                tiles_written += 1
    
    return tiles_written


# ===== MAIN =====

def main():
    parser = argparse.ArgumentParser(description='Compute monthly tiled K fields from GLORYS data')
    parser.add_argument('--compute_means', action='store_true',
                       help='Compute 3-year means (PASS 1)')
    parser.add_argument('--compute_k', action='store_true',
                       help='Compute monthly K fields (PASS 2)')
    parser.add_argument('--workers', type=int, default=4,
                       help='Number of parallel workers')
    
    args = parser.parse_args()
    
    print("\n" + "="*70)
    print("🌊 PROTEUS Monthly K-Value Calculator (Global Tiled Output)")
    print("="*70)
    print(f"GLORYS tiles directory: {GLORYS_TILES_DIR.absolute()}")
    print(f"Eddy directory: {EDDY_DIR.absolute()}")
    print(f"Output directory: {OUTPUT_DIR.absolute()}")
    print(f"Global grid: {N_LAT_GLOBAL} × {N_LON_GLOBAL} points")
    print(f"Tiles: {N_LON_TILES} × {N_LAT_TILES} = {N_LON_TILES * N_LAT_TILES} tiles")
    
    # Check if directories exist
    if not GLORYS_TILES_DIR.exists():
        print(f"\n❌ ERROR: GLORYS tiles directory not found: {GLORYS_TILES_DIR.absolute()}")
        return
    
    if not EDDY_DIR.exists():
        print(f"\n❌ ERROR: Eddy directory not found: {EDDY_DIR.absolute()}")
        return
    
    # Create output directory
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    # PASS 1: Compute 3-year means
    means_by_tile = None
    if args.compute_means:
        years = list(range(START_DATE.year, END_DATE.year + 1))
        means_by_tile = compute_all_means(GLORYS_TILES_DIR, OUTPUT_DIR, years, args.workers)
    else:
        # Try to load existing means
        means_path = OUTPUT_DIR / "3yr_means_tiled.npz"
        if means_path.exists():
            print(f"\n📂 Loading existing means from {means_path.absolute()}")
            data = np.load(means_path, allow_pickle=True)
            means_by_tile = data['means_by_tile'].item()
            print(f"  Loaded means for {len(means_by_tile)} tiles")
        else:
            print("\n⚠️ No means found. Run with --compute_means first.")
            return
    
    # PASS 2: Compute monthly K
    if args.compute_k:
        print("\n" + "="*70)
        print("⚙️  PASS 2: Computing monthly K fields...")
        print("="*70)
        
        months_processed = []
        
        for year in range(START_DATE.year, END_DATE.year + 1):
            for month in range(1, 13):
                tiles_written = process_month(
                    year, month,
                    GLORYS_TILES_DIR,
                    EDDY_DIR,
                    OUTPUT_DIR,
                    means_by_tile,
                    args.workers
                )
                
                if tiles_written > 0:
                    months_processed.append({
                        'year': year,
                        'month': month,
                        'tiles': tiles_written
                    })
                    print(f"  ✅ {year:04d}-{month:02d}: {tiles_written} tiles")
                else:
                    print(f"  ⚠️ {year:04d}-{month:02d}: No tiles processed")
                
                gc.collect()
        
        print(f"\n🎉 COMPLETE! Processed {len(months_processed)} months")
        print(f"📁 Output: {OUTPUT_DIR.absolute()}")
        
        # Save monthly stats
        stats_path = OUTPUT_DIR / 'k_monthly_stats.json'
        with open(stats_path, 'w') as f:
            json.dump(months_processed, f, indent=2)
    
    # Save metadata
    metadata = {
        'description': 'Monthly tiled eddy diffusivity (K) fields following Klocker et al. 2012',
        'method': '3-year mean for U, monthly anomalies for EKE, with phase speed suppression',
        'version': 2,
        'format': 'float16',
        'temporal_resolution': 'monthly',
        'spatial_resolution': 'global',
        'tile_size_degrees': TILE_SIZE,
        'parameters': {
            'mixing_efficiency': MIXING_EFFICIENCY,
            'g_over_k': G_OVER_K,
            'mean_period': f"{START_DATE.year}-{END_DATE.year}"
        },
        'grid': {
            'global_grid_points': {
                'n_lon': N_LON_GLOBAL,
                'n_lat': N_LAT_GLOBAL
            },
            'tiles': {
                'n_lon_tiles': N_LON_TILES,
                'n_lat_tiles': N_LAT_TILES,
                'lon_min': LON_MIN,
                'lon_max': LON_MAX,
                'lat_min': LAT_MIN,
                'lat_max': LAT_MAX,
                'lon_step': LON_STEP,
                'lat_step': LAT_STEP
            }
        },
        'depths': DEPTH_VALUES,
        'depth_count': N_DEPTH,
        'base_date': BASE_DATE.isoformat(),
        'total_tiles': len(means_by_tile) if means_by_tile else 0,
        'processing_date': datetime.now().isoformat()
    }
    
    metadata_path = OUTPUT_DIR / 'k_fields_metadata.json'
    with open(metadata_path, 'w') as f:
        json.dump(metadata, f, indent=2)
    
    print(f"\n📝 Metadata saved to: {metadata_path.absolute()}")
    print("="*70)


if __name__ == "__main__":
    main()