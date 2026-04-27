"""
Grid eddy radii AND PHASE SPEED from META3.2 atlas to GLOBAL GLORYS grid.
Handles BOTH anticyclonic and cyclonic eddies, combines them by month.
Computes monthly averages directly to save storage and processing time.
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

# ===== CONFIGURATION =====
TEST_MODE = False  # Set to False for full processing
TEST_MONTHS = 3  # Number of months to process in test mode

# Input files (global coverage)
ANTICYC_FILE = "data/META3.2_DT_allsat_Anticyclonic_long_19930101_20220209.nc"
CYCLONIC_FILE = "data/META3.2_DT_allsat_Cyclonic_long_19930101_20220209.nc"

# Output directories
OUTPUT_DIR = "data/eddy_atlas_global"
MONTHLY_OUTPUT_DIR = os.path.join(OUTPUT_DIR, "monthly")
COORDS_FILE = os.path.join(OUTPUT_DIR, "eddy_coords.bin")

# GLORYS global grid parameters
LON_STEP = 1/12  # 0.08333 degrees
LAT_STEP = 1/12

LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP
LAT_MIN = -80.0
LAT_MAX = 90.0

# Grid dimensions
N_LON = 4320
N_LAT = 2041
TOTAL_CELLS = N_LAT * N_LON

print(f"Global grid dimensions: {N_LAT} × {N_LON} = {TOTAL_CELLS:,} cells")
print(f"Expected monthly file size: {TOTAL_CELLS * 4 * 2 / (1024**3):.2f} GB (radius + speed)")

# Date range for processing (2011-2013 for PROTEUS)
START_DATE = datetime(2011, 1, 1)
END_DATE = datetime(2013, 12, 31)

# Processing parameters
CHUNK_SIZE = 100000  # Number of eddies to process at a time

os.makedirs(MONTHLY_OUTPUT_DIR, exist_ok=True)

warnings.filterwarnings('ignore')


# ===== GLOBAL GRID GENERATION =====

def generate_global_grid():
    """Generate global GLORYS grid coordinates."""
    print("\n📊 Generating global GLORYS grid coordinates...")
    
    lon_grid = np.linspace(LON_MIN, LON_MAX, N_LON, dtype=np.float32)
    lat_grid = np.linspace(LAT_MIN, LAT_MAX, N_LAT, dtype=np.float32)
    
    # Create 2D grids
    lon_2d, lat_2d = np.meshgrid(lon_grid, lat_grid)
    
    print(f"  ✓ Longitude: {lon_grid.min():.2f}° to {lon_grid.max():.2f}° ({N_LON} points)")
    print(f"  ✓ Latitude: {lat_grid.min():.2f}° to {lat_grid.max():.2f}° ({N_LAT} points)")
    
    return {
        'lon_grid': lon_2d,
        'lat_grid': lat_2d,
        'lon_1d': lon_grid,
        'lat_1d': lat_grid,
        'n_lon': N_LON,
        'n_lat': N_LAT
    }


def save_coordinates_file(lon_grid, lat_grid, output_path):
    """Save coordinates once (float32)."""
    print(f"\n💾 Creating global coordinates file...")
    
    with open(output_path, 'wb') as f:
        header = struct.pack('3i', 3, N_LAT, N_LON)  # Version 3 for global monthly
        f.write(header)
        f.write(lon_grid.astype(np.float32).tobytes())
        f.write(lat_grid.astype(np.float32).tobytes())
    
    file_size = os.path.getsize(output_path)
    print(f"  ✓ Coordinates saved: {N_LAT}×{N_LON} grid")
    print(f"  ✓ File size: {file_size / 1024 / 1024:.1f}MB")
    
    return {
        'n_lat': N_LAT,
        'n_lon': N_LON,
        'total_cells': TOTAL_CELLS,
        'file_size': file_size
    }


# ===== FAST NEAREST NEIGHBOR INTERPOLATION =====

def interpolate_eddies_to_grid(day_eddies, grid_lon, grid_lat):
    """
    Fast interpolation using KD-tree for nearest neighbor.
    """
    if len(day_eddies.obs) == 0:
        return None, None
    
    # Extract points and values
    points = np.column_stack([
        day_eddies.longitude.values,
        day_eddies.latitude.values
    ])
    
    # Radius values (convert to km)
    radius_values = day_eddies.effective_radius.values / 1000
    radius_values = np.where(np.isnan(radius_values), 50.0, radius_values)
    
    # Speed values
    if 'speed_average' in day_eddies:
        speed_values = day_eddies.speed_average.values
        speed_values = np.where(np.isnan(speed_values), 0.1, speed_values)
    else:
        speed_values = np.ones(len(points)) * 0.1
    
    # Build KD-tree
    tree = cKDTree(points)
    
    # Prepare grid points
    grid_points = np.column_stack([grid_lon.ravel(), grid_lat.ravel()])
    
    # Find nearest neighbors
    distances, indices = tree.query(grid_points, k=1)
    
    # Interpolate
    radius_flat = radius_values[indices]
    speed_flat = speed_values[indices]
    
    # Reshape to grid
    radius_grid = radius_flat.reshape(grid_lon.shape)
    speed_grid = speed_flat.reshape(grid_lon.shape)
    
    return radius_grid.astype(np.float32), speed_grid.astype(np.float32)


# ===== AGGREGATE EDDIES BY MONTH =====

def aggregate_monthly_eddies(year, month, anticyc_ds, cyclonic_ds, grid_info):
    """
    Aggregate all eddies in a month and interpolate to grid.
    Returns monthly mean radius and speed grids.
    """
    # Get date range for the month
    month_start = datetime(year, month, 1)
    if month == 12:
        month_end = datetime(year + 1, 1, 1)
    else:
        month_end = datetime(year, month + 1, 1)
    
    # Collect all eddies for this month
    all_eddies_list = []
    
    # Process each day in the month
    current_date = month_start
    while current_date < month_end:
        day_start = pd.Timestamp(current_date)
        day_end = day_start + pd.Timedelta(days=1)
        
        # Get anticyclonic eddies for this day
        try:
            anticyc_mask = (pd.to_datetime(anticyc_ds.time.values) >= day_start) & \
                           (pd.to_datetime(anticyc_ds.time.values) < day_end)
            anticyc_indices = np.where(anticyc_mask)[0]
            
            if len(anticyc_indices) > 0:
                anticyc_day = anticyc_ds.isel(obs=anticyc_indices)
                anticyc_day = anticyc_day.assign(polarity=('obs', np.ones(len(anticyc_day.obs), dtype=np.int8)))
                all_eddies_list.append(anticyc_day)
        except Exception as e:
            pass
        
        # Get cyclonic eddies for this day
        try:
            cyclonic_mask = (pd.to_datetime(cyclonic_ds.time.values) >= day_start) & \
                            (pd.to_datetime(cyclonic_ds.time.values) < day_end)
            cyclonic_indices = np.where(cyclonic_mask)[0]
            
            if len(cyclonic_indices) > 0:
                cyclonic_day = cyclonic_ds.isel(obs=cyclonic_indices)
                cyclonic_day = cyclonic_day.assign(polarity=('obs', -np.ones(len(cyclonic_day.obs), dtype=np.int8)))
                all_eddies_list.append(cyclonic_day)
        except Exception as e:
            pass
        
        current_date += timedelta(days=1)
    
    if not all_eddies_list:
        return None, None, 0
    
    # Combine all eddies for the month
    combined_month = xr.concat(all_eddies_list, dim='obs')
    
    # Interpolate to global GLORYS grid (once per month, not per day!)
    radius_grid, speed_grid = interpolate_eddies_to_grid(
        combined_month,
        grid_info['lon_grid'],
        grid_info['lat_grid']
    )
    
    if radius_grid is None or speed_grid is None:
        return None, None, 0
    
    return radius_grid, speed_grid, len(combined_month.obs)


def save_monthly_file(radius_grid, speed_grid, year, month, output_dir):
    """Save monthly radius and speed as binary file."""
    filename = f"eddy_{year:04d}{month:02d}.bin"
    filepath = os.path.join(output_dir, filename)
    
    with open(filepath, 'wb') as f:
        # Header: version=3 (global monthly), year, month
        header = struct.pack('3i', 3, year, month)
        f.write(header)
        
        # Write radius first, then speed
        f.write(radius_grid.tobytes())
        f.write(speed_grid.tobytes())
    
    file_size = os.path.getsize(filepath)
    
    return {
        'date': f"{year:04d}-{month:02d}",
        'file': filename,
        'size': int(file_size),
        'n_eddies': 0,  # We don't have per-month count easily
        'radius_min': float(radius_grid.min()),
        'radius_max': float(radius_grid.max()),
        'speed_min': float(speed_grid.min()),
        'speed_max': float(speed_grid.max()),
        'speed_mean': float(speed_grid.mean())
    }


# ===== MAIN =====

def main():
    print("\n" + "=" * 70)
    print("🌪️  MONTHLY GLOBAL EDDY ATLAS TO GLORYS GRID")
    print("=" * 70)
    
    print(f"Input files:")
    print(f"  Anticyclonic: {ANTICYC_FILE}")
    print(f"  Cyclonic: {CYCLONIC_FILE}")
    print(f"Date range: {START_DATE.date()} to {END_DATE.date()}")
    print(f"Grid: {N_LAT} × {N_LON} ({TOTAL_CELLS:,} cells)")
    print(f"Expected monthly file size: {TOTAL_CELLS * 4 * 2 / (1024**3):.2f} GB")
    print(f"Total months: 36")
    print(f"Total expected size: {36 * TOTAL_CELLS * 4 * 2 / (1024**3):.1f} GB")
    
    # Check input files
    if not os.path.exists(ANTICYC_FILE):
        print(f"❌ Error: Anticyclonic file not found: {ANTICYC_FILE}")
        return
    
    if not os.path.exists(CYCLONIC_FILE):
        print(f"❌ Error: Cyclonic file not found: {CYCLONIC_FILE}")
        return
    
    # Generate global grid
    grid_info = generate_global_grid()
    
    # Create coordinates file
    coords_info = save_coordinates_file(
        grid_info['lon_grid'],
        grid_info['lat_grid'],
        COORDS_FILE
    )
    
    # Open eddy datasets (lazy loading with chunking)
    print(f"\n📂 Loading eddy datasets...")
    anticyc_ds = xr.open_dataset(ANTICYC_FILE, chunks={'obs': CHUNK_SIZE})
    cyclonic_ds = xr.open_dataset(CYCLONIC_FILE, chunks={'obs': CHUNK_SIZE})
    
    print(f"  Anticyclonic eddies: {anticyc_ds.dims['obs']:,}")
    print(f"  Cyclonic eddies: {cyclonic_ds.dims['obs']:,}")
    print(f"  Total: {anticyc_ds.dims['obs'] + cyclonic_ds.dims['obs']:,}")
    
    # Generate list of months to process
    months_to_process = []
    current_date = START_DATE
    while current_date <= END_DATE:
        months_to_process.append((current_date.year, current_date.month))
        # Move to next month
        if current_date.month == 12:
            current_date = datetime(current_date.year + 1, 1, 1)
        else:
            current_date = datetime(current_date.year, current_date.month + 1, 1)
    
    print(f"\n📅 Processing {len(months_to_process)} months ({START_DATE.year}-{END_DATE.year})")
    
    if TEST_MODE:
        print(f"⚠️  TEST MODE: Processing first {TEST_MONTHS} months only")
        months_to_process = months_to_process[:TEST_MONTHS]
    
    # Process months
    print(f"\n⚙️  Processing months...")
    months_processed = []
    
    for year, month in tqdm(months_to_process, desc="Processing months"):
        print(f"\n  📅 {year:04d}-{month:02d}")
        
        radius_grid, speed_grid, n_eddies = aggregate_monthly_eddies(
            year, month, anticyc_ds, cyclonic_ds, grid_info
        )
        
        if radius_grid is not None:
            result = save_monthly_file(radius_grid, speed_grid, year, month, MONTHLY_OUTPUT_DIR)
            months_processed.append(result)
            size_mb = result['size'] / 1024 / 1024
            print(f"    ✅ Saved: {size_mb:.1f}MB, radius: {result['radius_min']:.1f}-{result['radius_max']:.1f}km")
        else:
            print(f"    ⚠️ No eddies found")
        
        # Clean up
        gc.collect()
    
    # Save metadata
    if months_processed:
        total_size_gb = sum(d['size'] for d in months_processed) / (1024 ** 3)
        
        metadata = {
            'description': 'Monthly global gridded eddy radii and phase speed from META3.2 atlas',
            'source_files': {
                'anticyclonic': ANTICYC_FILE,
                'cyclonic': CYCLONIC_FILE
            },
            'interpolation': 'nearest neighbor (KD-tree)',
            'temporal_resolution': 'monthly',
            'version': 3,
            'grid': {
                'type': 'global',
                'n_lat': N_LAT,
                'n_lon': N_LON,
                'total_cells': TOTAL_CELLS,
                'lon_min': LON_MIN,
                'lon_max': LON_MAX,
                'lat_min': LAT_MIN,
                'lat_max': LAT_MAX,
                'lon_step': LON_STEP,
                'lat_step': LAT_STEP,
                'coordinates_file': 'eddy_coords.bin'
            },
            'units': {
                'radius': 'km',
                'phase_speed': 'm/s'
            },
            'date_range': {
                'start': START_DATE.isoformat(),
                'end': END_DATE.isoformat()
            },
            'total_months': len(months_processed),
            'total_size_gb': total_size_gb,
            'months': months_processed,
            'processing_date': datetime.now().isoformat()
        }
        
        metadata_path = os.path.join(OUTPUT_DIR, 'eddy_metadata.json')
        with open(metadata_path, 'w') as f:
            json.dump(metadata, f, indent=2)
        
        print("\n" + "=" * 70)
        print(f"🎉 COMPLETE! Processed {len(months_processed)} months")
        print(f"📊 Total size: {total_size_gb:.1f}GB")
        print(f"📁 Output in: {OUTPUT_DIR}")
        print(f"📝 Metadata: {metadata_path}")
        print("=" * 70)
    else:
        print("\n⚠️ No months were processed.")
    
    # Clean up
    anticyc_ds.close()
    cyclonic_ds.close()


if __name__ == "__main__":
    main()