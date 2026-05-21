"""
Convert GLORYS NetCDF files to tiled binary format for Driftmap.
Processes one timestep at a time, filters extra day, merges depths.

Input: glorys_3yr_global/glorys_YYYYMM_depth_DD.nc
       where DD is 0, 40, 92, or 318
Output: glorys_tiles/YYYY/MM/DD/XXX_YYY.bin (all depths merged)
"""

import numpy as np
import xarray as xr
from pathlib import Path
import struct
import pandas as pd
from concurrent.futures import ProcessPoolExecutor, as_completed
import argparse

# ============================================================
# CONFIGURATION
# ============================================================

TILE_SIZE = 5.0                     # degrees
LON_STEP = 1/12                     # 0.08333 degrees
LAT_STEP = 1/12

# GLORYS grid bounds
LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP          # 179.91666666
LAT_MIN = -80.0
LAT_MAX = 90.0

# Your 4 depth levels (values as they appear in filenames)
DEPTH_VALUES = [0]     # meters

# Number of tiles
N_LON_TILES = int(360 / TILE_SIZE)   # 72
N_LAT_TILES = int(170 / TILE_SIZE)   # 34


# ============================================================
# UTILITY FUNCTIONS
# ============================================================

def get_tile_bounds(lon_idx, lat_idx):
    """Get tile boundaries for given tile indices."""
    lon_min = LON_MIN + lon_idx * TILE_SIZE
    lon_max = min(lon_min + TILE_SIZE, LON_MAX)
    lat_min = LAT_MIN + lat_idx * TILE_SIZE
    lat_max = min(lat_min + TILE_SIZE, LAT_MAX)
    return lon_min, lon_max, lat_min, lat_max


def process_depth_file(nc_path, output_root):
    """
    Process a single NetCDF file (one depth level) and write depth-specific tile files.
    Returns: list of (year, month, day, lon_idx, lat_idx, depth_pos) for tiles created.
    """
    filename = Path(nc_path).stem
    parts = filename.split('_')
    date_str = parts[1]              # "201101"
    depth_value = float(parts[3])    # 0, 40, 92, or 318
    
    # Skip if not one of our target depths
    if depth_value not in DEPTH_VALUES:
        return []
    
    depth_pos = DEPTH_VALUES.index(depth_value)
    
    year = int(date_str[:4])
    month = int(date_str[4:6])
    
    print(f"  Processing: {filename} (depth {depth_value:.0f}m, pos {depth_pos})")
    
    # Open NetCDF
    ds = xr.open_dataset(nc_path)
    
    # Get lazy references
    u = ds['uo']
    v = ds['vo']
    
    # Get coordinate arrays (small)
    lons = u['longitude'].values
    lats = u['latitude'].values
    
    # Get time coordinates
    times = ds['time'].values
    
    tiles_created = []
    
    # Process each timestep
    for t_idx in range(u.shape[0]):
        # Get actual date for this time step
        date = pd.to_datetime(times[t_idx])
        
        # Skip if month doesn't match the file's month (extra day)
        if date.month != month:
            print(f"    Skipping {date.strftime('%Y-%m-%d')} (not in {year:04d}-{month:02d})")
            continue
        
        day = date.day
        print(f"    Processing day {day:02d}")
        
        # Load data for this timestep (only one depth, cast to float16)
        u_t = u.isel(time=t_idx, depth=0).astype(np.float16).values
        v_t = v.isel(time=t_idx, depth=0).astype(np.float16).values
        
        # Process each tile
        for lon_idx in range(N_LON_TILES):
            lon_min, lon_max, _, _ = get_tile_bounds(lon_idx, 0)
            
            # Find indices within this longitude tile
            lon_mask = (lons >= lon_min) & (lons < lon_max)
            lon_indices = np.where(lon_mask)[0]
            
            if len(lon_indices) == 0:
                continue
            
            for lat_idx in range(N_LAT_TILES):
                _, _, lat_min, lat_max = get_tile_bounds(0, lat_idx)
                lat_mask = (lats >= lat_min) & (lats < lat_max)
                lat_indices = np.where(lat_mask)[0]
                
                if len(lat_indices) == 0:
                    continue
                
                # Extract tile data
                u_tile = u_t[np.ix_(lat_indices, lon_indices)]
                v_tile = v_t[np.ix_(lat_indices, lon_indices)]
                
                # Skip if no data (all zeros)
                if np.all(u_tile == 0) and np.all(v_tile == 0):
                    continue
                
                # Create output directory
                day_dir = output_root / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
                day_dir.mkdir(parents=True, exist_ok=True)
                
                # Write depth-specific tile file (temporary)
                depth_file = day_dir / f"{lon_idx:03d}_{lat_idx:03d}_{depth_pos}.bin"
                with open(depth_file, 'wb') as f:
                    f.write(struct.pack('<I', len(lon_indices)))   # n_lon
                    f.write(struct.pack('<I', len(lat_indices)))   # n_lat
                    f.write(struct.pack('<I', 1))                  # n_depths
                    f.write(struct.pack('<f', depth_value))        # depth value
                    u_tile.astype(np.float16).tofile(f)
                    v_tile.astype(np.float16).tofile(f)
                
                tiles_created.append((year, month, day, lon_idx, lat_idx, depth_pos))
    
    ds.close()
    return tiles_created


def merge_tiles(output_root, year, month, day):
    """
    Merge depth-specific tile files into a single file with all depths.
    Called after all depths for a day have been processed.
    """
    day_dir = output_root / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
    if not day_dir.exists():
        return
    
    # Find all tile bases (without depth suffix)
    tile_bases = set()
    for f in day_dir.glob("*.bin"):
        if '_' in f.stem and len(f.stem.split('_')) == 3:
            # This is a depth file: lon_lat_depth
            lon_idx, lat_idx, depth = f.stem.split('_')
            tile_bases.add((int(lon_idx), int(lat_idx)))
    
    for lon_idx, lat_idx in tile_bases:
        # Collect all depth files for this tile
        u_all = []
        v_all = []
        n_lon = None
        n_lat = None
        
        for depth_pos in range(len(DEPTH_VALUES)):
            depth_file = day_dir / f"{lon_idx:03d}_{lat_idx:03d}_{depth_pos}.bin"
            if depth_file.exists():
                with open(depth_file, 'rb') as f:
                    n_lon = struct.unpack('<I', f.read(4))[0]
                    n_lat = struct.unpack('<I', f.read(4))[0]
                    f.read(4)  # n_depths
                    depth_val = struct.unpack('<f', f.read(4))[0]
                    
                    u_data = np.frombuffer(f.read(n_lon * n_lat * 2), dtype=np.float16)
                    v_data = np.frombuffer(f.read(n_lon * n_lat * 2), dtype=np.float16)
                    
                    u_all.append(u_data)
                    v_all.append(v_data)
            else:
                # Missing depth file — create zeros
                if n_lon is not None:
                    u_all.append(np.zeros(n_lon * n_lat, dtype=np.float16))
                    v_all.append(np.zeros(n_lon * n_lat, dtype=np.float16))
                else:
                    # No reference tile exists, skip this tile
                    break
        
        if len(u_all) != len(DEPTH_VALUES):
            print(f"    Warning: Tile {lon_idx:03d}_{lat_idx:03d} has {len(u_all)} depths, expected {len(DEPTH_VALUES)} — skipping")
            continue
        
        # Write merged file
        output_file = day_dir / f"{lon_idx:03d}_{lat_idx:03d}.bin"
        with open(output_file, 'wb') as f:
            f.write(struct.pack('<I', n_lon))
            f.write(struct.pack('<I', n_lat))
            f.write(struct.pack('<I', len(DEPTH_VALUES)))
            for depth_val in DEPTH_VALUES:
                f.write(struct.pack('<f', float(depth_val)))
            
            np.stack(u_all, axis=0).astype(np.float16).tofile(f)
            np.stack(v_all, axis=0).astype(np.float16).tofile(f)
        
        # Delete individual depth files
        for depth_pos in range(len(DEPTH_VALUES)):
            depth_file = day_dir / f"{lon_idx:03d}_{lat_idx:03d}_{depth_pos}.bin"
            if depth_file.exists():
                depth_file.unlink()


# ============================================================
# MAIN
# ============================================================

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=str, default='glorys_3yr_global', help='Input directory')
    parser.add_argument('--output', type=str, default='glorys_tiles', help='Output directory')
    parser.add_argument('--workers', type=int, default=2, help='Number of parallel workers')
    args = parser.parse_args()
    
    input_path = Path(args.input)
    output_root = Path(args.output)
    output_root.mkdir(parents=True, exist_ok=True)
    
    # Get all NetCDF files
    nc_files = sorted(input_path.glob("glorys_*.nc"))
    print(f"Found {len(nc_files)} NetCDF files")
    
    # Filter to only our target depths
    target_files = []
    for f in nc_files:
        parts = f.stem.split('_')
        depth_val = float(parts[3])
        if depth_val in DEPTH_VALUES:
            target_files.append(f)
    
    print(f"Target depth files: {len(target_files)} (depths {DEPTH_VALUES})")
    
    # Process files in parallel
    all_tiles = []
    with ProcessPoolExecutor(max_workers=args.workers) as executor:
        futures = []
        for nc_path in target_files:
            futures.append(executor.submit(process_depth_file, nc_path, output_root))
        
        for future in as_completed(futures):
            all_tiles.extend(future.result())
    
    # Group by date for merging
    dates = set((y, m, d) for (y, m, d, _, _, _) in all_tiles)
    
    print(f"\nMerging tiles for {len(dates)} dates...")
    for year, month, day in sorted(dates):
        print(f"  Merging: {year:04d}-{month:02d}-{day:02d}")
        merge_tiles(output_root, year, month, day)
    
    print("\nDone!")


if __name__ == "__main__":
    main()