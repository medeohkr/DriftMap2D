#!/usr/bin/env python3
"""
Grid eddy radii AND PHASE SPEED from META3.2 NRT atlas to GLOBAL GLORYS grid.
DAILY output — processes one day at a time, outputs eddy_YYYYMMDD.bin files.
Uses KD-tree nearest neighbor interpolation.
"""

import xarray as xr
import numpy as np
import struct
import json
import os
from datetime import datetime, timedelta
import gc
from scipy.spatial import cKDTree
import warnings
import pandas as pd
from tqdm import tqdm
import argparse

# ===== CONFIGURATION =====
TEST_MODE = False
TEST_DAYS = 5

# Input files (NRT — update paths as new files arrive)
ANTICYC_FILE = "data\META3.2_DT_allsat_Anticyclonic_long_19930101_20220209.nc"
CYCLONIC_FILE = "data\META3.2_DT_allsat_Cyclonic_long_19930101_20220209.nc"

# Output directories
OUTPUT_DIR = "data/eddy_atlas_global_DT"
DAILY_OUTPUT_DIR = os.path.join(OUTPUT_DIR, "daily")
COORDS_FILE = os.path.join(OUTPUT_DIR, "eddy_coords.bin")

# GLORYS global grid parameters
LON_STEP = 1/12
LAT_STEP = 1/12
LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP
LAT_MIN = -80.0
LAT_MAX = 90.0

# Grid dimensions
N_LON = 4320
N_LAT = 2041
TOTAL_CELLS = N_LAT * N_LON

DATE_START = datetime(2022, 1, 1)  # Match your NRT file start
DATE_END = datetime(2022, 1, 14)   # Match your NRT file end

CHUNK_SIZE = 50000

os.makedirs(DAILY_OUTPUT_DIR, exist_ok=True)
warnings.filterwarnings('ignore')


# ===== GLOBAL GRID =====

def generate_global_grid():
    """Generate global GLORYS grid coordinates."""
    print("\n📊 Generating global GLORYS grid coordinates...")
    lon_grid_1d = np.linspace(LON_MIN, LON_MAX, N_LON, dtype=np.float32)
    lat_grid_1d = np.linspace(LAT_MIN, LAT_MAX, N_LAT, dtype=np.float32)
    lon_2d, lat_2d = np.meshgrid(lon_grid_1d, lat_grid_1d)
    print(f"  ✓ Grid: {N_LAT} × {N_LON} = {TOTAL_CELLS:,} cells")
    print(f"  ✓ Single file size: {TOTAL_CELLS * 4 * 2 / (1024**3):.2f} GB")
    return {
        'lon_grid': lon_2d,
        'lat_grid': lat_2d,
        'lon_1d': lon_grid_1d,
        'lat_1d': lat_grid_1d,
        'n_lon': N_LON,
        'n_lat': N_LAT
    }


def save_coordinates_file(lon_grid, lat_grid, output_path):
    """Save coordinates once for decoding."""
    print(f"\n💾 Creating coordinates file...")
    with open(output_path, 'wb') as f:
        header = struct.pack('3i', 4, N_LAT, N_LON)  # Version 4 for daily
        f.write(header)
        f.write(lon_grid.astype(np.float32).tobytes())
        f.write(lat_grid.astype(np.float32).tobytes())
    file_size = os.path.getsize(output_path)
    print(f"  ✓ {file_size / 1024 / 1024:.1f}MB")
    return {'n_lat': N_LAT, 'n_lon': N_LON, 'total_cells': TOTAL_CELLS, 'file_size': file_size}


# ===== INTERPOLATION =====

def interpolate_eddies_to_grid(day_eddies, grid_lon, grid_lat):
    """KD-tree nearest neighbor interpolation for a single day's eddies."""
    if len(day_eddies.obs) == 0:
        return None, None

    points = np.column_stack([
        day_eddies.longitude.values,
        day_eddies.latitude.values
    ])

    # Radius (km)
    radius_values = day_eddies.effective_radius.values / 1000
    radius_values = np.where(np.isnan(radius_values), 50.0, radius_values)

    # Phase speed (m/s)
    if 'speed_average' in day_eddies:
        speed_values = day_eddies.speed_average.values
        speed_values = np.where(np.isnan(speed_values), 0.1, speed_values)
    else:
        speed_values = np.full(len(points), 0.1)

    # Build KD-tree and query
    tree = cKDTree(points)
    grid_points = np.column_stack([grid_lon.ravel(), grid_lat.ravel()])
    _, indices = tree.query(grid_points, k=1)

    radius_grid = radius_values[indices].reshape(grid_lon.shape).astype(np.float32)
    speed_grid = speed_values[indices].reshape(grid_lon.shape).astype(np.float32)

    return radius_grid, speed_grid


# ===== DAILY PROCESSING =====

def get_eddies_for_day(date, anticyc_ds, cyclonic_ds):
    """Extract all eddies (anticyclonic + cyclonic) for a single day."""
    day_start = pd.Timestamp(date)
    day_end = day_start + pd.Timedelta(days=1)

    eddies_list = []

    # Anticyclonic
    try:
        mask = (pd.to_datetime(anticyc_ds.time.values) >= day_start) & \
               (pd.to_datetime(anticyc_ds.time.values) < day_end)
        indices = np.where(mask)[0]
        if len(indices) > 0:
            eddies_list.append(anticyc_ds.isel(obs=indices))
    except Exception:
        pass

    # Cyclonic
    try:
        mask = (pd.to_datetime(cyclonic_ds.time.values) >= day_start) & \
               (pd.to_datetime(cyclonic_ds.time.values) < day_end)
        indices = np.where(mask)[0]
        if len(indices) > 0:
            eddies_list.append(cyclonic_ds.isel(obs=indices))
    except Exception:
        pass

    if not eddies_list:
        return None

    return xr.concat(eddies_list, dim='obs')


def save_daily_file(radius_grid, speed_grid, date, output_dir):
    """Save daily radius + phase speed as binary file."""
    filename = f"eddy_{date.strftime('%Y%m%d')}.bin"
    filepath = os.path.join(output_dir, filename)

    with open(filepath, 'wb') as f:
        # Header: version=4 (daily), year, month, day
        header = struct.pack('4i', 4, date.year, date.month, date.day)
        f.write(header)
        f.write(radius_grid.tobytes())
        f.write(speed_grid.tobytes())

    return {
        'date': date.strftime('%Y-%m-%d'),
        'file': filename,
        'size': int(os.path.getsize(filepath)),
        'radius_min': float(radius_grid.min()),
        'radius_max': float(radius_grid.max()),
        'speed_mean': float(speed_grid.mean())
    }


def process_date(date, anticyc_ds, cyclonic_ds, grid_info):
    """Process a single date and save daily file."""
    output_path = os.path.join(DAILY_OUTPUT_DIR, f"eddy_{date.strftime('%Y%m%d')}.bin")
    
    # Skip if already processed
    if os.path.exists(output_path):
        return None, True  # Already done

    day_eddies = get_eddies_for_day(date, anticyc_ds, cyclonic_ds)
    
    if day_eddies is None or len(day_eddies.obs) == 0:
        return None, False  # No eddies today

    radius_grid, speed_grid = interpolate_eddies_to_grid(
        day_eddies,
        grid_info['lon_grid'],
        grid_info['lat_grid']
    )

    if radius_grid is None:
        return None, False

    result = save_daily_file(radius_grid, speed_grid, date, DAILY_OUTPUT_DIR)
    return result, True


# ===== MAIN =====

def main():
    parser = argparse.ArgumentParser(description='Process AVISO eddy atlas to daily grids')
    parser.add_argument('--start', type=str, help='Start date (YYYY-MM-DD)')
    parser.add_argument('--end', type=str, help='End date (YYYY-MM-DD)')
    parser.add_argument('--auto', action='store_true', 
                       help='Process all dates in NRT files')
    parser.add_argument('--days', type=int, default=None,
                       help='Process last N days only (for cron updates)')
    
    args = parser.parse_args()

    print("\n" + "=" * 70)
    print("🌪️  DAILY EDDY ATLAS TO GLORYS GRID")
    print("=" * 70)
    print(f"Input: {ANTICYC_FILE}")
    print(f"Input: {CYCLONIC_FILE}")
    print(f"Output: {DAILY_OUTPUT_DIR}")
    print(f"Single file size: {TOTAL_CELLS * 4 * 2 / (1024**3):.2f} GB")
    print(f"Expected total for 3 years: {3 * 365 * TOTAL_CELLS * 4 * 2 / (1024**3):.1f} GB")

    # Generate grid
    grid_info = generate_global_grid()

    # Save coordinates (first time only)
    if not os.path.exists(COORDS_FILE):
        save_coordinates_file(grid_info['lon_grid'], grid_info['lat_grid'], COORDS_FILE)

    # Determine date range
    if args.start and args.end:
        start_date = datetime.strptime(args.start, "%Y-%m-%d")
        end_date = datetime.strptime(args.end, "%Y-%m-%d")
    elif args.days:
        end_date = datetime.utcnow()
        start_date = end_date - timedelta(days=args.days)
    elif args.auto:
        start_date = DATE_START
        end_date = DATE_END
    else:
        print("\nUsage:")
        print("  python process_atlas_daily.py --auto")
        print("  python process_atlas_daily.py --start 2025-04-01 --end 2025-05-01")
        print("  python process_atlas_daily.py --days 30")
        return

    print(f"\n📅 Processing: {start_date.date()} to {end_date.date()}")

    # Open datasets
    print(f"\n📂 Loading eddy datasets (lazy)...")
    anticyc_ds = xr.open_dataset(ANTICYC_FILE, chunks={'obs': CHUNK_SIZE})
    cyclonic_ds = xr.open_dataset(CYCLONIC_FILE, chunks={'obs': CHUNK_SIZE})
    print(f"  Anticyclonic: {anticyc_ds.dims['obs']:,} observations")
    print(f"  Cyclonic: {cyclonic_ds.dims['obs']:,} observations")

    # Generate date list
    dates = []
    current = start_date
    while current <= end_date:
        dates.append(current)
        current += timedelta(days=1)

    if TEST_MODE:
        print(f"⚠️  TEST MODE: Processing first {TEST_DAYS} days only")
        dates = dates[:TEST_DAYS]

    print(f"\n⚙️  Processing {len(dates)} days...")

    results = []
    skipped = 0
    no_eddies = 0
    processed = 0

    for date in tqdm(dates, desc="Processing days"):
        result, success = process_date(date, anticyc_ds, cyclonic_ds, grid_info)

        if result is not None:
            processed += 1
            results.append(result)
            if processed % 30 == 0:
                print(f"\n    ✅ {date.date()}: radius {result['radius_min']:.0f}-{result['radius_max']:.0f}km, "
                      f"speed mean {result['speed_mean']:.3f} m/s, "
                      f"size {result['size']/1024/1024:.1f}MB")
        elif success:
            skipped += 1
        else:
            no_eddies += 1

        if processed % 100 == 0:
            gc.collect()

    # Summary
    print("\n" + "=" * 70)
    print(f"🎉 COMPLETE!")
    print(f"  Processed: {processed} days")
    print(f"  Skipped (already done): {skipped} days")
    print(f"  No eddies found: {no_eddies} days")
    
    if results:
        total_gb = sum(r['size'] for r in results) / (1024**3)
        print(f"  Total size: {total_gb:.1f} GB")
    
    print(f"  Output: {DAILY_OUTPUT_DIR}")
    print("=" * 70)

    # Save metadata
    metadata = {
        'description': 'Daily gridded eddy radii and phase speed from META3.2 NRT atlas',
        'version': 4,
        'resolution': 'daily',
        'grid': {
            'n_lat': N_LAT, 'n_lon': N_LON,
            'lon_min': LON_MIN, 'lon_max': LON_MAX,
            'lat_min': LAT_MIN, 'lat_max': LAT_MAX,
            'lon_step': LON_STEP, 'lat_step': LAT_STEP,
        },
        'units': {'radius': 'km', 'phase_speed': 'm/s'},
        'date_range': {
            'start': dates[0].isoformat() if dates else None,
            'end': dates[-1].isoformat() if dates else None,
        },
        'total_days_processed': processed,
        'processing_date': datetime.utcnow().isoformat(),
    }

    with open(os.path.join(OUTPUT_DIR, 'eddy_metadata_daily.json'), 'w') as f:
        json.dump(metadata, f, indent=2)

    anticyc_ds.close()
    cyclonic_ds.close()


if __name__ == "__main__":
    main()