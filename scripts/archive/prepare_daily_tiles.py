#!/usr/bin/env python3
"""
Daily GLORYS + K tile pipeline.
Downloads monthly GLORYS NetCDF, tiles surface currents, computes daily K,
and bundles them into unified binary files.

Usage:
    python prepare_daily_tiles.py --year 2025 --month 5
    python prepare_daily_tiles.py --auto
"""

import numpy as np
import struct
import xarray as xr
import os
import json
import argparse
from pathlib import Path
from datetime import datetime, timedelta
from tqdm import tqdm
import gc

# ===== CONFIGURATION =====

TILE_SIZE = 10.0
LON_MIN = -180.0
LON_MAX = 180.0
LAT_MIN = -80.0
LAT_MAX = 90.0
LON_STEP = 1/12
LAT_STEP = 1/12
N_LON_TILES = 36
N_LAT_TILES = 34

# Global eddy grid dimensions (from process_atlas_daily.py)
N_LON_GLOBAL = 4320
N_LAT_GLOBAL = 2041

MIXING_EFFICIENCY = 0.35
G_OVER_K = 0.03

DATA_DIR = Path("./data")
CMEMS_DIR = DATA_DIR / "cmems_downloads"
TILES_DIR = DATA_DIR / "forecast_tiles"
EDDY_DAILY_DIR = DATA_DIR / "eddy_atlas_global/daily"
MEANS_DIR = DATA_DIR / "means"

ROLLING_WINDOW_YEARS = 3
EDDY_LAG_DAYS = 14

TILES_DIR.mkdir(parents=True, exist_ok=True)
MEANS_DIR.mkdir(parents=True, exist_ok=True)


# ===== STEP 1: Download =====

def download_glorys_month(year, month):
    import copernicusmarine
    
    month_dir = CMEMS_DIR / f"{year:04d}" / f"{month:02d}"
    month_dir.mkdir(parents=True, exist_ok=True)
    output_file = month_dir / f"glorys_{year:04d}{month:02d}.nc"
    
    if output_file.exists():
        print(f"  ✓ Already downloaded: {output_file}")
        return output_file
    
    print(f"  Downloading GLORYS {year:04d}-{month:02d}...")
    
    start_date = datetime(year, month, 1)
    if month == 12:
        end_date = datetime(year + 1, 1, 1) - timedelta(days=1)
    else:
        end_date = datetime(year, month + 1, 1) - timedelta(days=1)
    
    try:
        copernicusmarine.subset(
            dataset_id="cmems_mod_glo_phy-cur_anfc_0.083deg_P1D-m",
            variables=["uo", "vo"],
            minimum_longitude=LON_MIN,
            maximum_longitude=LON_MAX - LON_STEP,
            minimum_latitude=LAT_MIN,
            maximum_latitude=LAT_MAX,
            start_datetime=start_date.strftime("%Y-%m-%dT00:00:00"),
            end_datetime=end_date.strftime("%Y-%m-%dT23:59:59"),
            minimum_depth=0.49402499198913574,
            maximum_depth=0.49402499198913574,
            output_directory=month_dir,
            output_filename=output_file.name,
        )
        print(f"  ✓ Downloaded: {output_file}")
        return output_file
    except Exception as e:
        print(f"  ⚠️ Download failed: {e}")
        return output_file if output_file.exists() else None


# ===== STEP 2: Tile =====

def get_tile_bounds(tilex, tiley):
    lon_min = LON_MIN + TILE_SIZE * tilex
    lon_max = LON_MIN + TILE_SIZE * (tilex + 1)
    lat_min = LAT_MIN + TILE_SIZE * tiley
    lat_max = LAT_MIN + TILE_SIZE * (tiley + 1)
    return lon_min, lon_max, lat_min, lat_max


def tile_surface_currents(nc_file):
    print(f"  Tiling surface currents from {nc_file.name}...")
    
    ds = xr.open_dataset(nc_file)
    u = ds['uo'].isel(depth=0)
    v = ds['vo'].isel(depth=0)
    lons = ds['longitude'].values
    lats = ds['latitude'].values
    n_time = ds.dims['time']
    
    parts = nc_file.stem.split('_')
    yearmonth = parts[1]
    year = int(yearmonth[:4])
    month = int(yearmonth[4:6])
    
    tiles_written = 0
    
    for t in range(n_time):
        u_t = u.isel(time=t).values
        v_t = v.isel(time=t).values
        day = t + 1
        day_dir = TILES_DIR / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
        day_dir.mkdir(parents=True, exist_ok=True)
        
        for tilex in range(N_LON_TILES):
            lon_min, lon_max, _, _ = get_tile_bounds(tilex, 0)
            lon_mask = (lons >= lon_min) & (lons < lon_max)
            lon_indices = np.where(lon_mask)[0]
            if len(lon_indices) == 0:
                continue
            
            for tiley in range(N_LAT_TILES):
                _, _, lat_min, lat_max = get_tile_bounds(0, tiley)
                lat_mask = (lats >= lat_min) & (lats < lat_max)
                lat_indices = np.where(lat_mask)[0]
                if len(lat_indices) == 0:
                    continue
                
                u_tile = np.nan_to_num(
                    u_t[np.ix_(lat_indices, lon_indices)], nan=0.0
                )
                v_tile = np.nan_to_num(
                    v_t[np.ix_(lat_indices, lon_indices)], nan=0.0
                )
                
                tile_file = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
                
                with open(tile_file, 'wb') as f:
                    f.write(struct.pack('<I', len(lon_indices)))
                    f.write(struct.pack('<I', len(lat_indices)))
                    f.write(struct.pack('<I', 1))
                    f.write(struct.pack('<f', 0.0))
                    u_tile.astype(np.float16).tofile(f)
                    v_tile.astype(np.float16).tofile(f)
                    np.zeros_like(u_tile, dtype=np.float16).tofile(f)
                
                tiles_written += 1
    
    ds.close()
    print(f"  ✓ {tiles_written} tiles for {year:04d}-{month:02d}")
    return year, month, n_time


# ===== STEP 3: Rolling 3-year mean =====

def read_tile_uv(tile_path):
    """Read just u,v from a tile file. Returns (u, v, n_lon, n_lat) or None."""
    try:
        with open(tile_path, 'rb') as f:
            n_lon = struct.unpack('<I', f.read(4))[0]
            n_lat = struct.unpack('<I', f.read(4))[0]
            n_depth = struct.unpack('<I', f.read(4))[0]
            f.read(4)  # depth value
            u = np.frombuffer(f.read(n_lat * n_lon * 2), dtype=np.float16)
            v = np.frombuffer(f.read(n_lat * n_lon * 2), dtype=np.float16)
            return u.reshape((n_lat, n_lon)).astype(np.float32), \
                   v.reshape((n_lat, n_lon)).astype(np.float32), n_lon, n_lat
    except Exception:
        return None


def compute_daily_mean_for_date(year, month, day):
    """
    Compute 3-year rolling mean for a specific calendar date.
    Accumulates same month/day across all 3 prior years.
    Returns per-tile means keyed by (tilex, tiley).
    """
    tile_sums = {}  # (tilex, tiley) -> (u_sum, v_sum, count)
    
    for y in range(year - ROLLING_WINDOW_YEARS, year):
        day_dir = TILES_DIR / f"{y:04d}" / f"{month:02d}" / f"{day:02d}"
        if not day_dir.exists():
            continue
        
        for tilex in range(N_LON_TILES):
            for tiley in range(N_LAT_TILES):
                tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
                result = read_tile_uv(tile_path)
                if result is None:
                    continue
                u, v, n_lon, n_lat = result
                key = (tilex, tiley)
                if key not in tile_sums:
                    tile_sums[key] = (np.zeros_like(u), np.zeros_like(v), 0)
                su, sv, cnt = tile_sums[key]
                tile_sums[key] = (su + u, sv + v, cnt + 1)
    
    # Convert sums to means
    tile_means = {}
    for key, (su, sv, cnt) in tile_sums.items():
        if cnt > 0:
            tile_means[key] = (su / cnt, sv / cnt)
    
    return tile_means if tile_means else None


# ===== STEP 4: Compute daily K =====

def get_available_eddy_file(year, month, day):
    target_date = datetime(year, month, day)
    
    for offset in range(EDDY_LAG_DAYS, EDDY_LAG_DAYS + 30):
        check_date = target_date - timedelta(days=offset)
        eddy_file = EDDY_DAILY_DIR / f"eddy_{check_date.strftime('%Y%m%d')}.bin"
        if eddy_file.exists():
            return eddy_file, check_date
    
    for offset in range(EDDY_LAG_DAYS + 30, 90):
        check_date = target_date - timedelta(days=offset)
        eddy_file = EDDY_DAILY_DIR / f"eddy_{check_date.strftime('%Y%m%d')}.bin"
        if eddy_file.exists():
            return eddy_file, check_date
    
    return None, None


def read_eddy_daily(eddy_path):
    with open(eddy_path, 'rb') as f:
        f.read(16)  # header: version(4), year, month, day
        data = np.frombuffer(f.read(), dtype=np.float32)
        half = len(data) // 2
        radius = data[:half].reshape((N_LAT_GLOBAL, N_LON_GLOBAL))
        speed = data[half:].reshape((N_LAT_GLOBAL, N_LON_GLOBAL))
        return radius, speed


def extract_region(global_data, tilex, tiley, target_shape):
    """Extract tile region from global eddy grid and resize if needed."""
    lon_min, lon_max, lat_min, lat_max = get_tile_bounds(tilex, tiley)
    
    lon_1d = np.linspace(-180, 180 - 1/12, N_LON_GLOBAL)
    lat_1d = np.linspace(-80, 90, N_LAT_GLOBAL)
    
    lon_indices = np.where((lon_1d >= lon_min) & (lon_1d < lon_max))[0]
    lat_indices = np.where((lat_1d >= lat_min) & (lat_1d < lat_max))[0]
    
    if len(lon_indices) == 0 or len(lat_indices) == 0:
        return None
    
    region = global_data[np.ix_(lat_indices, lon_indices)]
    
    if region.shape != target_shape:
        from scipy.ndimage import zoom
        zoom_factors = (target_shape[0] / region.shape[0],
                       target_shape[1] / region.shape[1])
        region = zoom(region, zoom_factors, order=1)
    
    return region


def compute_daily_k(year, month, day):
    day_dir = TILES_DIR / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
    if not day_dir.exists():
        return 0
    
    eddy_file, eddy_date = get_available_eddy_file(year, month, day)
    if eddy_file is None:
        return 0
    
    radius_global, speed_global = read_eddy_daily(eddy_file)
    tile_means = compute_daily_mean_for_date(year, month, day)
    if tile_means is None:
        return 0
    
    tiles_updated = 0
    
    for (tilex, tiley), (u_mean, v_mean) in tile_means.items():
        tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
        if not tile_path.exists():
            continue
        
        result = read_tile_uv(tile_path)
        if result is None:
            continue
        u_tile, v_tile, n_lon, n_lat = result
        
        radius_tile = extract_region(radius_global, tilex, tiley, (n_lat, n_lon))
        speed_tile = extract_region(speed_global, tilex, tiley, (n_lat, n_lon))
        
        if radius_tile is None:
            continue
        
        u_anomaly = u_tile - u_mean
        v_anomaly = v_tile - v_mean
        eke = 0.5 * (u_anomaly**2 + v_anomaly**2)
        
        L_m = np.maximum(radius_tile * 1000, 1000)
        K0 = MIXING_EFFICIENCY * np.sqrt(2 * np.maximum(eke, 0)) * L_m
        
        k_wavenumber = 2 * np.pi / L_m
        g = G_OVER_K * k_wavenumber
        U_mean_speed = np.sqrt(u_mean**2 + v_mean**2)
        rel_speed = np.abs(speed_tile - U_mean_speed)
        suppression = 1.0 / (1.0 + (k_wavenumber**2 * rel_speed**2) / (g**2 + 1e-10))
        
        k_tile = (K0 * suppression).astype(np.float16)
        
        with open(tile_path, 'wb') as f:
            f.write(struct.pack('<I', n_lon))
            f.write(struct.pack('<I', n_lat))
            f.write(struct.pack('<I', 1))
            f.write(struct.pack('<f', 0.0))
            u_tile.astype(np.float16).tofile(f)
            v_tile.astype(np.float16).tofile(f)
            k_tile.tofile(f)
        
        tiles_updated += 1
    
    if tiles_updated > 0:
        eddy_age = (datetime(year, month, day) - eddy_date).days
        print(f"    {year:04d}-{month:02d}-{day:02d}: {tiles_updated} tiles, "
              f"eddy data {eddy_age}d old")
    
    return tiles_updated


# ===== MAIN PIPELINE =====

def process_month(year, month):
    print(f"\n{'='*60}")
    print(f"Processing {year:04d}-{month:02d}")
    print(f"{'='*60}")
    
    nc_file = download_glorys_month(year, month)
    if nc_file is None:
        print(f"  ❌ Cannot proceed without GLORYS data")
        return False
    
    year_out, month_out, n_days = tile_surface_currents(nc_file)
    
    print(f"\n  Computing daily K for {n_days} days...")
    total_k_tiles = 0
    for day in range(1, n_days + 1):
        k_tiles = compute_daily_k(year, month, day)
        total_k_tiles += k_tiles
        if day % 5 == 0 or day == n_days:
            print(f"    Day {day:02d}: {k_tiles} K tiles, total: {total_k_tiles}")
    
    print(f"\n  ✅ {year:04d}-{month:02d} complete: {total_k_tiles} K tiles")
    return True


def process_rolling_window():
    today = datetime.utcnow()
    start_date = today - timedelta(days=365 * ROLLING_WINDOW_YEARS)
    
    print(f"\n{'='*70}")
    print(f"🌊 Processing rolling window: {start_date.date()} to {today.date()}")
    print(f"{'='*70}")
    
    months_to_process = []
    current = datetime(start_date.year, start_date.month, 1)
    while current <= today:
        months_to_process.append((current.year, current.month))
        if current.month == 12:
            current = datetime(current.year + 1, 1, 1)
        else:
            current = datetime(current.year, current.month + 1, 1)
    
    print(f"Months to process: {len(months_to_process)}")
    
    for year, month in tqdm(months_to_process, desc="Processing months"):
        month_dir = TILES_DIR / f"{year:04d}" / f"{month:02d}"
        if month_dir.exists():
            first_day = month_dir / "01"
            if first_day.exists():
                sample_tile = first_day / "000_000.bin"
                if sample_tile.exists():
                    with open(sample_tile, 'rb') as f:
                        n_lon = struct.unpack('<I', f.read(4))[0]
                        n_lat = struct.unpack('<I', f.read(4))[0]
                        f.read(8)  # n_depth + depth val
                        f.read(n_lat * n_lon * 2)  # u
                        f.read(n_lat * n_lon * 2)  # v
                        k_check = np.frombuffer(f.read(n_lat * n_lon * 2), dtype=np.float16)
                        if np.any(k_check != 0):
                            continue  # already processed
        
        process_month(year, month)
        gc.collect()
    
    print(f"\n{'='*70}")
    print(f"🎉 Rolling window processing complete!")
    print(f"{'='*70}")


def update_metadata():
    metadata = {
        'description': 'Daily GLORYS surface currents + SMK diffusivity tiles',
        'version': 2,
        'format': 'Binary (u:float16, v:float16, K:float16)',
        'tile_size_degrees': TILE_SIZE,
        'n_lon_tiles': N_LON_TILES,
        'n_lat_tiles': N_LAT_TILES,
        'k_parameters': {
            'mixing_efficiency': MIXING_EFFICIENCY,
            'g_over_k': G_OVER_K,
            'method': 'Klocker et al. 2012, EKE from 3-year rolling anomalies',
            'eddy_source': 'AVISO META3.2 NRT daily',
            'eddy_lag_days': EDDY_LAG_DAYS,
        },
        'rolling_window_years': ROLLING_WINDOW_YEARS,
        'processing_date': datetime.utcnow().isoformat(),
    }
    with open(TILES_DIR / 'tiles_metadata.json', 'w') as f:
        json.dump(metadata, f, indent=2)
    print(f"📝 Metadata saved")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--year", type=int)
    parser.add_argument("--month", type=int)
    parser.add_argument("--auto", action="store_true")
    args = parser.parse_args()
    
    if args.auto:
        process_rolling_window()
        update_metadata()
    elif args.year and args.month:
        process_month(args.year, args.month)
        update_metadata()
    else:
        print("Usage: python prepare_daily_tiles.py --year 2025 --month 5")
        print("   or: python prepare_daily_tiles.py --auto")