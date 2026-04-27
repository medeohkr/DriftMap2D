"""
Grid eddy radii AND PHASE SPEED from META3.2 atlas to GLORYS grid.
Uses direct nearest-neighbor interpolation.
"""

import xarray as xr
import numpy as np
import struct
import json
import os
from datetime import datetime
import gc
from scipy.interpolate import NearestNDInterpolator
import warnings
import pandas as pd

# ===== CONFIGURATION =====
TEST_MODE = False
INPUT_FILE = "data/eddy_atlas/eddy_all_pacific_2011_2013.nc"
GLORYS_DIR = "data/glorys_3yr_bin"
OUTPUT_DIR = "data/eddy_radii_grid_glorys"
DAILY_OUTPUT_DIR = os.path.join(OUTPUT_DIR, "daily")
COORDS_FILE = os.path.join(OUTPUT_DIR, "eddy_coords.bin")

os.makedirs(DAILY_OUTPUT_DIR, exist_ok=True)

warnings.filterwarnings('ignore')


# ===== GLORYS GRID LOADING =====

def load_glorys_grid_from_binary():
    """Load GLORYS grid coordinates from first daily binary file."""
    print("📊 Loading GLORYS grid coordinates from binary...")

    files = sorted([f for f in os.listdir(GLORYS_DIR) if f.endswith('.bin') and 'glorys_' in f])

    if not files:
        raise FileNotFoundError(f"No GLORYS binary files found in {GLORYS_DIR}")

    first_file = os.path.join(GLORYS_DIR, files[0])
    print(f"  Using: {files[0]}")

    with open(first_file, 'rb') as f:
        header = struct.unpack('7i', f.read(28))
        version, n_lat, n_lon, n_depth, year, month, day = header

        total_cells = n_lat * n_lon
        print(f"  ✓ Grid: {n_lat}×{n_lon}")

        lon_array = np.frombuffer(f.read(total_cells * 4), dtype=np.float32)
        lat_array = np.frombuffer(f.read(total_cells * 4), dtype=np.float32)

        lon_grid = lon_array.reshape((n_lat, n_lon))
        lat_grid = lat_array.reshape((n_lat, n_lon))

        print(f"  ✓ Longitude: {lon_grid.min():.2f}° to {lon_grid.max():.2f}°")
        print(f"  ✓ Latitude: {lat_grid.min():.2f}° to {lat_grid.max():.2f}°")

        return {
            'lon_grid': lon_grid,
            'lat_grid': lat_grid,
            'n_lat': n_lat,
            'n_lon': n_lon,
            'metadata': {'files': files}
        }


# ===== COORDINATES FILE =====

def save_coordinates_file(lon_grid, lat_grid, output_path):
    """Save coordinates once (float32)."""
    print(f"\n💾 Creating single coordinates file...")

    n_lat, n_lon = lon_grid.shape
    total_cells = n_lat * n_lon

    with open(output_path, 'wb') as f:
        header = struct.pack('3i', 1, n_lat, n_lon)
        f.write(header)
        f.write(lon_grid.astype(np.float32).tobytes())
        f.write(lat_grid.astype(np.float32).tobytes())

    file_size = os.path.getsize(output_path)
    print(f"  ✓ Coordinates saved: {n_lat}×{n_lon} grid")
    print(f"  ✓ File size: {file_size / 1024 / 1024:.1f}MB")

    return {
        'n_lat': n_lat,
        'n_lon': n_lon,
        'total_cells': total_cells,
        'file_size': file_size
    }


# ===== INTERPOLATION TO GLORYS GRID =====
# NEW: Now interpolates BOTH radius and phase speed

# ===== INTERPOLATION TO GLORYS GRID =====

def interpolate_eddies_to_grid(day_eddies, grid_lon, grid_lat):
    """
    Interpolate eddy radii AND phase speed directly to target grid.
    """
    points = np.column_stack([
        day_eddies.longitude.values,
        day_eddies.latitude.values
    ])
    
    # ===== RADIUS (L) =====
    radius_values = day_eddies.effective_radius.values / 1000  # convert to km
    
    # ===== PHASE SPEED (c_w) =====
    # FIXED: Now using the correct variable name!
    if 'speed_average' in day_eddies:
        speed_values = day_eddies.speed_average.values
        print(f"    ✓ Found speed_average, range: {speed_values.min():.3f} to {speed_values.max():.3f} m/s")
    else:
        print(f"    ⚠️ speed_average not found, using default 0.1 m/s")
        speed_values = np.ones(len(points)) * 0.1
    
    # Handle missing values
    radius_values = np.where(np.isnan(radius_values), 50, radius_values)
    speed_values = np.where(np.isnan(speed_values), 0.1, speed_values)
    
    if len(points) == 0:
        return None, None
    
    # Create interpolators
    interp_r = NearestNDInterpolator(points, radius_values)
    interp_s = NearestNDInterpolator(points, speed_values)
    
    grid_points = np.column_stack([
        grid_lon.ravel(),
        grid_lat.ravel()
    ])
    
    radius_grid_flat = interp_r(grid_points)
    speed_grid_flat = interp_s(grid_points)
    
    radius_grid = radius_grid_flat.reshape(grid_lon.shape)
    speed_grid = speed_grid_flat.reshape(grid_lon.shape)
    
    return radius_grid.astype(np.float32), speed_grid.astype(np.float32)
# ===== SAVE DAILY FILE =====
# NEW: Now saves BOTH radius and phase speed

def save_daily_eddy_file(radius_grid, speed_grid, date_obj, coords_info, output_dir):
    """Save daily radius AND phase speed (version 2 format)."""
    if hasattr(date_obj, 'strftime'):
        date_str = date_obj.strftime('%Y%m%d')
        year, month, day = date_obj.year, date_obj.month, date_obj.day
    else:
        pd_date = pd.Timestamp(date_obj)
        date_str = pd_date.strftime('%Y%m%d')
        year, month, day = pd_date.year, pd_date.month, pd_date.day

    filename = f"eddy_{date_str}.bin"
    filepath = os.path.join(output_dir, filename)

    expected_shape = (coords_info['n_lat'], coords_info['n_lon'])
    
    # Validate shapes
    if radius_grid.shape != expected_shape:
        raise ValueError(f"Radius shape mismatch: {radius_grid.shape} vs {expected_shape}")
    if speed_grid.shape != expected_shape:
        raise ValueError(f"Speed shape mismatch: {speed_grid.shape} vs {expected_shape}")

    with open(filepath, 'wb') as f:
        # Header: version=2, year, month, day
        header = struct.pack('4i', 2, year, month, day)
        f.write(header)
        
        # Write radius first, then speed
        f.write(radius_grid.tobytes())
        f.write(speed_grid.tobytes())

    file_size = os.path.getsize(filepath)
    print(f"    📁 {date_str}: {file_size / 1024 / 1024:.2f}MB")
    print(f"    📊 Radius range: {radius_grid.min():.1f} to {radius_grid.max():.1f} km")
    print(f"    📊 Speed range: {speed_grid.min():.3f} to {speed_grid.max():.3f} m/s")
    print(f"    📊 Speed mean: {speed_grid.mean():.3f} m/s")

    return {
        'date': date_str,
        'file': filename,
        'size': int(file_size),
        'radius_min': float(radius_grid.min()),
        'radius_max': float(radius_grid.max()),
        'speed_min': float(speed_grid.min()),
        'speed_max': float(speed_grid.max())
    }


# ===== MAIN =====

def main():
    print("\n" + "=" * 70)
    print("🌪️  GRIDDING EDDY RADII AND PHASE SPEED TO GLORYS GRID")
    print("=" * 70)

    # Load GLORYS grid
    glorys_grid = load_glorys_grid_from_binary()

    # Create coordinates file
    coords_info = save_coordinates_file(
        glorys_grid['lon_grid'],
        glorys_grid['lat_grid'],
        COORDS_FILE
    )

    # Load eddy data
    print(f"\n📂 Loading eddy data from {INPUT_FILE}...")
    ds = xr.open_dataset(INPUT_FILE)
    
    # Print available variables to help debug
    print(f"  Available variables: {list(ds.data_vars)}")

    # Get unique days
    unique_days = np.unique(ds.time.values)
    print(f"Found {len(unique_days)} unique days")

    # Initialize metadata
    metadata = {
        'description': 'Gridded eddy radii and phase speed from META3.2 atlas on GLORYS grid',
        'source': 'META3.2_DT_allsat eddy atlas',
        'interpolation': 'nearest neighbor',
        'version': 2,  # NEW: version 2 includes phase speed
        'grid': {
            'source': f'GLORYS from {GLORYS_DIR}',
            'n_lat': coords_info['n_lat'],
            'n_lon': coords_info['n_lon'],
            'total_cells': coords_info['total_cells'],
            'coordinates_file': 'eddy_coords.bin'
        },
        'units': {
            'radius': 'km',
            'phase_speed': 'm/s'
        },
        'time_period': f"{unique_days[0]} to {unique_days[-1]}",
        'dates': [],
        'files': [],
        'processing_date': datetime.now().isoformat()
    }

    # Process each day
    total_days = 0
    total_size = 0

    for day_idx, day in enumerate(unique_days):
        if TEST_MODE and day_idx > 0:
            print(f"\n[TEST MODE] Stopping after first day")
            break

        print(f"\n📅 Day {day_idx + 1}/{len(unique_days)}: {day}")

        # Get eddies for this day
        day_eddies = ds.where(ds.time == day, drop=True)

        if len(day_eddies.obs) == 0:
            print(f"    No eddies found")
            continue

        print(f"    {len(day_eddies.obs)} eddy observations")

        # Interpolate directly to GLORYS grid
        radius_grid, speed_grid = interpolate_eddies_to_grid(
            day_eddies,
            glorys_grid['lon_grid'],
            glorys_grid['lat_grid']
        )

        if radius_grid is None or speed_grid is None:
            print(f"    Interpolation failed")
            continue

        # Save daily file (now with both fields)
        file_info = save_daily_eddy_file(
            radius_grid, speed_grid, day, coords_info, DAILY_OUTPUT_DIR
        )

        metadata['dates'].append(file_info['date'])
        metadata['files'].append(file_info)
        total_days += 1
        total_size += file_info['size']

        # Clean up
        del day_eddies, radius_grid, speed_grid
        if day_idx % 10 == 0:
            gc.collect()

    # Save metadata
    metadata['total_days'] = total_days
    metadata['total_size_gb'] = total_size / (1024 ** 3)

    metadata_path = os.path.join(OUTPUT_DIR, 'eddy_metadata.json')
    with open(metadata_path, 'w') as f:
        json.dump(metadata, f, indent=2)

    print("\n" + "=" * 70)
    print(f"🎉 COMPLETE! Processed {total_days} days")
    print(f"📊 Total size: {metadata['total_size_gb']:.1f}GB")
    print(f"📁 Output in: {OUTPUT_DIR}")
    print("=" * 70)


if __name__ == "__main__":
    main()