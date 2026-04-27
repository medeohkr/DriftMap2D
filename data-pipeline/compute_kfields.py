#!/usr/bin/env python3
"""
Tiled K-Value Calculator (Complete Pipeline)
Combines GLORYS tiles + gridded eddy data to compute K fields following Klocker et al. 2012

Pipeline:
1. Read gridded eddy data (radius + phase speed) from prepare_atlas.py output
2. Read tiled GLORYS data from tile_generator.py output
3. Compute 3-year means of u, v
4. For each day: anomalies → EKE → K with suppression
5. Output tiled K fields
"""

import numpy as np
import struct
import json
from pathlib import Path
from datetime import datetime, timedelta
import gc
from concurrent.futures import ProcessPoolExecutor, as_completed
import argparse
from functools import partial
from tqdm import tqdm

# ============================================================
# CONFIGURATION
# ============================================================

TILE_SIZE = 5.0                     # degrees
LON_STEP = 1/12                     # 0.08333 degrees
LAT_STEP = 1/12

# GLORYS grid bounds
LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP
LAT_MIN = -80.0
LAT_MAX = 90.0

# Klocker parameters
MIXING_EFFICIENCY = 0.35
G_OVER_K = 0.03

# Depth levels (meters)
DEPTH_VALUES = [0, 40, 92, 318]

# Number of tiles
N_LON_TILES = int(360 / TILE_SIZE)   # 72
N_LAT_TILES = int(170 / TILE_SIZE)   # 34

BASE_DATE = datetime(2011, 1, 1)


# ============================================================
# FILE READING FUNCTIONS
# ============================================================

def read_glorys_tile(tile_path):
    """
    Read a GLORYS tile binary file.
    Returns: u (n_depth, n_lat, n_lon), v, n_lon, n_lat, n_depth, depths
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


def read_eddy_gridded(date, eddy_root, coords_file):
    """
    Read gridded eddy data (radius and phase speed) for a specific date.
    Returns: radius_km (n_lat, n_lon), speed_ms (n_lat, n_lon)
    """
    date_str = date.strftime('%Y%m%d')
    eddy_file = eddy_root / "daily" / f"eddy_{date_str}.bin"
    
    if not eddy_file.exists():
        return None, None
    
    with open(eddy_file, 'rb') as f:
        header = struct.unpack('4i', f.read(16))
        version, year, month, day = header
        
        # Read radius and speed
        data = np.frombuffer(f.read(), dtype=np.float32)
        
        if version == 1:
            # Old format: only radius
            radius_km = data.reshape((N_GLORYS_LAT, N_GLORYS_LON))
            speed_ms = np.zeros_like(radius_km)
        else:
            # Version 2: radius + speed
            half = len(data) // 2
            radius_km = data[:half].reshape((N_GLORYS_LAT, N_GLORYS_LON))
            speed_ms = data[half:].reshape((N_GLORYS_LAT, N_GLORYS_LON))
        
        return radius_km, speed_ms


def read_eddy_coords(coords_file):
    """Read GLORYS grid coordinates."""
    with open(coords_file, 'rb') as f:
        header = struct.unpack('3i', f.read(12))
        version, n_lat, n_lon = header
        
        lon = np.frombuffer(f.read(n_lat * n_lon * 4), dtype=np.float32)
        lat = np.frombuffer(f.read(n_lat * n_lon * 4), dtype=np.float32)
        
        lon_grid = lon.reshape((n_lat, n_lon))
        lat_grid = lat.reshape((n_lat, n_lon))
        
        return lon_grid, lat_grid, n_lat, n_lon


# ============================================================
# TILE EXTRACTION FROM GRIDDED DATA
# ============================================================

def extract_tile_from_grid(grid_data, lon_grid, lat_grid, lon_idx, lat_idx):
    """
    Extract a tile from gridded data (radius or speed) based on GLORYS grid.
    Returns: tile_data (n_lat_tile, n_lon_tile)
    """
    lon_min, lon_max, lat_min, lat_max = get_tile_bounds(lon_idx, lat_idx)
    
    # Find indices within this tile
    lon_mask = (lon_grid[0, :] >= lon_min) & (lon_grid[0, :] < lon_max)
    lat_mask = (lat_grid[:, 0] >= lat_min) & (lat_grid[:, 0] < lat_max)
    
    lon_indices = np.where(lon_mask)[0]
    lat_indices = np.where(lat_mask)[0]
    
    if len(lon_indices) == 0 or len(lat_indices) == 0:
        return None
    
    # Extract tile
    tile_data = grid_data[np.ix_(lat_indices, lon_indices)]
    
    return tile_data, len(lon_indices), len(lat_indices)


# ============================================================
# PASS 1: Compute 3-year means
# ============================================================

def compute_tile_mean(tile_info, glorys_root, years):
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
            for day in range(1, 32):
                day_dir = glorys_root / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
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


def compute_all_means(glorys_root, output_root, years, n_workers=4):
    """Compute 3-year means for all tiles."""
    print("\n" + "="*70)
    print("📊 PASS 1: Computing 3-year means from GLORYS tiles...")
    print("="*70)
    
    all_tiles = [(lon_idx, lat_idx) 
                 for lon_idx in range(N_LON_TILES) 
                 for lat_idx in range(N_LAT_TILES)]
    
    print(f"Processing {len(all_tiles)} tiles across {len(years)} years...")
    
    means_by_tile = {}
    with ProcessPoolExecutor(max_workers=n_workers) as executor:
        future_to_tile = {
            executor.submit(compute_tile_mean, tile, glorys_root, years): tile 
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
    
    # Save means
    means_path = output_root / "3yr_means_tiled.npz"
    np.savez(means_path, 
             means_by_tile=means_by_tile,
             n_lon_tiles=N_LON_TILES,
             n_lat_tiles=N_LAT_TILES,
             tile_size=TILE_SIZE,
             depth_values=DEPTH_VALUES)
    
    print(f"\n✓ Computed means for {len(means_by_tile)} tiles")
    print(f"✓ Saved to {means_path}")
    
    return means_by_tile


# ============================================================
# PASS 2: Compute daily K
# ============================================================

def process_daily_tile(tile_info, date, glorys_root, eddy_root, output_root, means_by_tile, coords_info):
    """
    Process a single tile for a specific day.
    Returns: (lon_idx, lat_idx, k_data, n_lon, n_lat, n_depth, depths) or None
    """
    lon_idx, lat_idx = tile_info
    year, month, day = date.year, date.month, date.day
    
    # Check if we have mean data for this tile
    if (lon_idx, lat_idx) not in means_by_tile:
        return None
    
    mean_data = means_by_tile[(lon_idx, lat_idx)]
    
    # Read today's GLORYS data
    tile_pattern = f"{lon_idx:03d}_{lat_idx:03d}.bin"
    glorys_path = glorys_root / f"{year:04d}" / f"{month:02d}" / f"{day:02d}" / tile_pattern
    
    if not glorys_path.exists():
        return None
    
    try:
        # Read u, v for today
        u_daily, v_daily, n_lon, n_lat, n_depth, depths = read_glorys_tile(glorys_path)
        
        # Get means
        u_mean = mean_data['u_mean']
        v_mean = mean_data['v_mean']
        
        # Compute anomalies
        u_prime = u_daily - u_mean
        v_prime = v_daily - v_mean
        
        # Compute daily EKE
        eke_daily = 0.5 * (u_prime**2 + v_prime**2)
        
        # Read gridded eddy data for this day
        radius_km, speed_ms = read_eddy_gridded(date, eddy_root, coords_info)
        
        if radius_km is None:
            return None
        
        # Extract tile from gridded eddy data
        lon_grid = coords_info['lon_grid']
        lat_grid = coords_info['lat_grid']
        
        radius_tile, tile_n_lon, tile_n_lat = extract_tile_from_grid(radius_km, lon_grid, lat_grid, lon_idx, lat_idx)
        speed_tile, _, _ = extract_tile_from_grid(speed_ms, lon_grid, lat_grid, lon_idx, lat_idx)
        
        if radius_tile is None:
            return None
        
        # Ensure tile dimensions match
        if radius_tile.shape != (n_lat, n_lon):
            # Resample if needed (shouldn't happen with proper grid alignment)
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
        K_daily = np.zeros((n_depth, n_lat, n_lon), dtype=np.float32)
        
        # Compute K for each depth
        for depth_idx in range(n_depth):
            U_depth = U_mean_tile[depth_idx]
            eke_depth = eke_daily[depth_idx]
            
            # Unsuppressed diffusivity: K0 = C * sqrt(2*EKE) * L
            K0 = MIXING_EFFICIENCY * np.sqrt(2 * np.maximum(eke_depth, 0)) * L_m
            
            # Suppression factor
            rel_speed = np.abs(speed_tile - U_depth)
            suppression = 1.0 / (1.0 + (k**2 * rel_speed**2) / (g**2 + 1e-10))
            
            K_daily[depth_idx] = K0 * suppression
        
        return (lon_idx, lat_idx, K_daily, n_lon, n_lat, n_depth, depths)
        
    except Exception as e:
        # Silent fail for missing data
        return None


def process_daily_k(date, glorys_root, eddy_root, output_root, means_by_tile, coords_info, n_workers=4):
    """Process all tiles for a specific day."""
    year, month, day = date.year, date.month, date.day
    
    # Create output directory
    day_dir = output_root / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
    day_dir.mkdir(parents=True, exist_ok=True)
    
    # Process all tiles
    active_tiles = list(means_by_tile.keys())
    tiles_written = 0
    
    with ProcessPoolExecutor(max_workers=n_workers) as executor:
        process_func = partial(process_daily_tile, 
                               date=date, 
                               glorys_root=glorys_root,
                               eddy_root=eddy_root,
                               output_root=output_root,
                               means_by_tile=means_by_tile,
                               coords_info=coords_info)
        
        futures = {executor.submit(process_func, tile): tile for tile in active_tiles}
        
        for future in tqdm(as_completed(futures), total=len(active_tiles), desc="  Processing tiles", leave=False):
            result = future.result()
            if result is not None:
                lon_idx, lat_idx, k_data, n_lon, n_lat, n_depth, depths = result
                
                # Write K tile
                output_path = day_dir / f"{lon_idx:03d}_{lat_idx:03d}.bin"
                with open(output_path, 'wb') as f:
                    f.write(struct.pack('<I', n_lon))
                    f.write(struct.pack('<I', n_lat))
                    f.write(struct.pack('<I', n_depth))
                    for depth_val in depths:
                        f.write(struct.pack('<f', float(depth_val)))
                    k_data.astype(np.float16).tofile(f)
                
                tiles_written += 1
    
    return tiles_written


# ============================================================
# MAIN
# ============================================================

def main():
    parser = argparse.ArgumentParser(description='Compute tiled K fields from GLORYS + eddy data')
    parser.add_argument('--glorys_root', type=str, 
                       default='/web/glorys_tiles',
                       help='Root directory of tiled GLORYS data')
    parser.add_argument('--eddy_root', type=str,
                       default='/data/eddy_radii_grid_glorys',
                       help='Root directory of gridded eddy data')
    parser.add_argument('--output_root', type=str,
                       default='/data/k_fields_tiled',
                       help='Output directory for K fields')
    parser.add_argument('--start_year', type=int, default=2011,
                       help='Start year for mean calculation')
    parser.add_argument('--end_year', type=int, default=2013,
                       help='End year for mean calculation')
    parser.add_argument('--compute_means', action='store_true',
                       help='Compute 3-year means (PASS 1)')
    parser.add_argument('--compute_k', action='store_true',
                       help='Compute daily K fields (PASS 2)')
    parser.add_argument('--workers', type=int, default=4,
                       help='Number of parallel workers')
    parser.add_argument('--start_date', type=str, default='2011-01-01',
                       help='Start date for K computation (YYYY-MM-DD)')
    parser.add_argument('--end_date', type=str, default='2013-12-31',
                       help='End date for K computation (YYYY-MM-DD)')
    
    args = parser.parse_args()
    
    glorys_root = Path(args.glorys_root)
    eddy_root = Path(args.eddy_root)
    output_root = Path(args.output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    
    # Load GLORYS grid coordinates from eddy data
    coords_file = eddy_root / "eddy_coords.bin"
    if not coords_file.exists():
        print(f"Error: Eddy coordinates file not found: {coords_file}")
        print("Run prepare_atlas.py first to generate gridded eddy data.")
        return
    
    lon_grid, lat_grid, n_glorys_lat, n_glorys_lon = read_eddy_coords(coords_file)
    global N_GLORYS_LAT, N_GLORYS_LON
    N_GLORYS_LAT, N_GLORYS_LON = n_glorys_lat, n_glorys_lon
    
    coords_info = {
        'lon_grid': lon_grid,
        'lat_grid': lat_grid,
        'n_lat': n_glorys_lat,
        'n_lon': n_glorys_lon
    }
    
    # PASS 1: Compute 3-year means
    means_by_tile = None
    if args.compute_means:
        years = list(range(args.start_year, args.end_year + 1))
        means_by_tile = compute_all_means(glorys_root, output_root, years, args.workers)
    else:
        # Try to load existing means
        means_path = output_root / "3yr_means_tiled.npz"
        if means_path.exists():
            print(f"\n📂 Loading existing means from {means_path}")
            data = np.load(means_path, allow_pickle=True)
            means_by_tile = data['means_by_tile'].item()
            print(f"  Loaded means for {len(means_by_tile)} tiles")
        else:
            print("\n⚠️ No means found. Run with --compute_means first.")
            return
    
    # PASS 2: Compute daily K
    if args.compute_k:
        print("\n" + "="*70)
        print("⚙️  PASS 2: Computing daily K fields...")
        print("="*70)
        
        start_date = datetime.strptime(args.start_date, "%Y-%m-%d")
        end_date = datetime.strptime(args.end_date, "%Y-%m-%d")
        
        current_date = start_date
        days_processed = 0
        daily_stats = []
        
        while current_date <= end_date:
            print(f"\n📅 {current_date.strftime('%Y-%m-%d')}")
            
            tiles_written = process_daily_k(
                current_date,
                glorys_root,
                eddy_root,
                output_root,
                means_by_tile,
                coords_info,
                args.workers
            )
            
            if tiles_written > 0:
                days_processed += 1
                daily_stats.append({
                    'date': current_date.strftime('%Y-%m-%d'),
                    'tiles': tiles_written
                })
                print(f"  ✅ Processed {tiles_written} tiles")
            else:
                print(f"  ⚠️ No data found")
            
            current_date += timedelta(days=1)
            
            if days_processed % 10 == 0:
                gc.collect()
        
        print(f"\n🎉 COMPLETE! Processed {days_processed} days")
        print(f"📁 Output: {output_root}")
        
        # Save daily stats
        stats_path = output_root / 'k_daily_stats.json'
        with open(stats_path, 'w') as f:
            json.dump(daily_stats, f, indent=2)
    
    # Save metadata
    metadata = {
        'description': 'Tiled eddy diffusivity (K) fields following Klocker et al. 2012',
        'method': '3-year mean for U, daily anomalies for EKE, with phase speed suppression',
        'version': 2,
        'format': 'float16',
        'tile_size_degrees': TILE_SIZE,
        'parameters': {
            'mixing_efficiency': MIXING_EFFICIENCY,
            'g_over_k': G_OVER_K,
            'mean_period': f"{args.start_year}-{args.end_year}"
        },
        'grid': {
            'n_lon_tiles': N_LON_TILES,
            'n_lat_tiles': N_LAT_TILES,
            'lon_min': LON_MIN,
            'lon_max': LON_MAX,
            'lat_min': LAT_MIN,
            'lat_max': LAT_MAX,
            'lon_step': LON_STEP,
            'lat_step': LAT_STEP
        },
        'depths': DEPTH_VALUES,
        'depth_count': len(DEPTH_VALUES),
        'base_date': BASE_DATE.isoformat(),
        'total_tiles': len(means_by_tile) if means_by_tile else 0,
        'processing_date': datetime.now().isoformat()
    }
    
    metadata_path = output_root / 'k_fields_metadata.json'
    with open(metadata_path, 'w') as f:
        json.dump(metadata, f, indent=2)
    
    print(f"\n📝 Metadata saved to {metadata_path}")
    print("="*70)


# ============================================================
# UTILITY: Get tile bounds
# ============================================================

def get_tile_bounds(lon_idx, lat_idx):
    """Get tile boundaries for given tile indices."""
    lon_min = LON_MIN + lon_idx * TILE_SIZE
    lon_max = min(lon_min + TILE_SIZE, LON_MAX)
    lat_min = LAT_MIN + lat_idx * TILE_SIZE
    lat_max = min(lat_min + TILE_SIZE, LAT_MAX)
    return lon_min, lon_max, lat_min, lat_max


if __name__ == "__main__":
    main()