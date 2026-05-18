#!/usr/bin/env python3
"""
Daily forecast update: download SMOC + ECMWF day by day, tile immediately, upload to R2.
Maintains a 40-day rolling window (30 days hindcast + 10 days forecast).
"""

import os
import sys
import struct
import shutil
import numpy as np
import xarray as xr
import boto3
from pathlib import Path
from datetime import datetime, timedelta
from concurrent.futures import ThreadPoolExecutor, as_completed

# ===== CONFIG =====

if sys.platform == "linux":
    BASE_DIR = Path("./data")
else:
    BASE_DIR = Path("D:/projects/driftmap/data")

BUCKET = "driftmap-tiles"
FORECAST_DAYS = 11
HINDCAST_DAYS = 30

TILE_SIZE = 10.0
N_LON_TILES = 36
N_LAT_TILES = 17

NC_DIR = BASE_DIR / "smoc_nc"
WIND_DIR = BASE_DIR / "ecmwf"
TILES_DIR = BASE_DIR / "forecast_tiles"
VIZ_DIR = BASE_DIR / "visualization"

# R2 credentials
ACCOUNT_ID = os.environ.get("ACCOUNT_ID", "").strip()
ACCESS_KEY = os.environ.get("ACCESS_KEY", "").strip()
SECRET_KEY = os.environ.get("SECRET_KEY", "").strip()
CMEMS_USER = os.environ.get("CMEMS_USER", "").strip()
CMEMS_PASS = os.environ.get("CMEMS_PASS", "").strip()

if not all([ACCOUNT_ID, ACCESS_KEY, SECRET_KEY]):
    print("❌ ERROR: Missing R2 credentials")
    sys.exit(1)

if not CMEMS_USER or not CMEMS_PASS:
    print("⚠️  WARNING: Missing CMEMS credentials, SMOC download may fail")

s3 = boto3.client(
    's3',
    endpoint_url=f'https://{ACCOUNT_ID}.r2.cloudflarestorage.com',
    aws_access_key_id=ACCESS_KEY,
    aws_secret_access_key=SECRET_KEY,
    region_name='auto',
)


def delete_prefix(prefix):
    deleted = 0
    paginator = s3.get_paginator('list_objects_v2')
    for page in paginator.paginate(Bucket=BUCKET, Prefix=prefix):
        if 'Contents' in page:
            objects = [{'Key': obj['Key']} for obj in page['Contents']]
            s3.delete_objects(Bucket=BUCKET, Delete={'Objects': objects})
            deleted += len(objects)
    return deleted


def delete_oldest_day():
    oldest = datetime.utcnow() - timedelta(days=HINDCAST_DAYS + 1)
    prefix = f"tiles/{oldest.strftime('%Y/%m/%d')}/"
    deleted = delete_prefix(prefix)
    if deleted > 0:
        print(f"  Deleted oldest day {oldest.date()}: {deleted} tiles")
    return deleted


def download_smoc_day(date):
    """Download a single day of SMOC data (24 hours, hourly)."""
    import copernicusmarine
    
    # Login with credentials
    if CMEMS_USER and CMEMS_PASS:
        copernicusmarine.login(username=CMEMS_USER, password=CMEMS_PASS)
    else:
        copernicusmarine.login(username=".", password=".")
    
    NC_DIR.mkdir(parents=True, exist_ok=True)
    out_file = str(NC_DIR / f"smoc_{date.strftime('%Y%m%d')}.nc")
    
    print(f"    Downloading SMOC {date.date()}...")
    
    copernicusmarine.subset(
        dataset_id="cmems_mod_glo_phy_anfc_merged-uv_PT1H-i",
        variables=["utotal", "vtotal"],
        minimum_longitude=-180,
        maximum_longitude=179.916,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=date.strftime("%Y-%m-%dT00:00:00"),
        end_datetime=date.strftime("%Y-%m-%dT23:00:00"),
        minimum_depth=0.49402499198913574,
        maximum_depth=0.49402499198913574,
        output_filename=out_file,
        force_download=True,
    )
    
    # Verify file
    if Path(out_file).exists() and Path(out_file).stat().st_size > 0:
        print(f"    ✅ SMOC: {Path(out_file).stat().st_size / 1e6:.1f} MB")
        return out_file
    else:
        print(f"    ❌ SMOC download failed")
        return None


def download_ecmwf_day(date):
    """Download a single day of ECMWF wind + SST (6-hourly)."""
    from ecmwf.opendata import Client
    
    WIND_DIR.mkdir(parents=True, exist_ok=True)
    out_file = str(WIND_DIR / f"ecmwf_{date.strftime('%Y%m%d')}.grib2")
    
    print(f"    Downloading ECMWF {date.date()}...")
    
    for source in ["aws", "ecmwf"]:
        try:
            client = Client(source=source)
            client.retrieve(
                date=date.strftime("%Y-%m-%d"),
                time="00",
                step=[0, 6, 12, 18],
                param=["10u", "10v", "skt"],
                target=out_file,
                type="fc",
                levtype="sfc",
            )
            if Path(out_file).exists():
                print(f"    ✅ ECMWF: {Path(out_file).stat().st_size / 1e6:.1f} MB")
                return out_file
        except Exception as e:
            print(f"      {source} failed: {e}")
    return None


def tile_and_upload_day(date, wind_file):
    """Tile a single day and upload immediately, then delete."""
    
    nc_file = NC_DIR / f"smoc_{date.strftime('%Y%m%d')}.nc"
    if not nc_file.exists():
        print(f"    ❌ SMOC file not found: {nc_file}")
        return 0
    
    ds = xr.open_dataset(str(nc_file))
    lons = ds['longitude'].values
    lats = ds['latitude'].values
    
    # Current data: 24 hours (hourly)
    u_day = ds['utotal'].isel(depth=0).values  # shape: (24, lat, lon)
    v_day = ds['vtotal'].isel(depth=0).values
    n_hours = 24
    
    # Load wind if available
    has_wind = False
    u_wind_global = v_wind_global = skt_global = wind_lons = wind_lats = None
    n_wind_steps = 0
    
    if wind_file and Path(wind_file).exists():
        try:
            ds_wind = xr.open_dataset(wind_file, engine="cfgrib")
            u_wind_global = ds_wind['u10'].values  # shape: (4, lat, lon)
            v_wind_global = ds_wind['v10'].values
            skt_global = ds_wind['skt'].values if 'skt' in ds_wind else np.zeros_like(u_wind_global)
            wind_lons = ds_wind['longitude'].values
            wind_lats = ds_wind['latitude'].values
            n_wind_steps = u_wind_global.shape[0]
            has_wind = True
            ds_wind.close()
        except Exception as e:
            print(f"    ⚠️ Wind load failed: {e}")
    
    day_dir = TILES_DIR / date.strftime("%Y/%m/%d")
    day_dir.mkdir(parents=True, exist_ok=True)
    tiles = 0
    
    for tilex in range(N_LON_TILES):
        lon_min = -180 + TILE_SIZE * tilex
        lon_max = lon_min + TILE_SIZE
        lon_idx = np.where((lons >= lon_min) & (lons < lon_max))[0]
        if len(lon_idx) == 0:
            continue
        
        for tiley in range(N_LAT_TILES):
            lat_min = -80 + TILE_SIZE * tiley
            lat_max = lat_min + TILE_SIZE
            lat_idx = np.where((lats >= lat_min) & (lats < lat_max))[0]
            if len(lat_idx) == 0:
                continue
            
            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
            
            try:
                with open(tile_path, 'wb') as f:
                    # Header
                    f.write(struct.pack('<I', len(lon_idx)))   # n_lon
                    f.write(struct.pack('<I', len(lat_idx)))   # n_lat
                    # f.write(struct.pack('<I', n_hours))        # n_hours (24)
                    f.write(struct.pack('<I', 1))              # n_levels
                    f.write(struct.pack('<f', 0.0))            # level
                    
                    # Current data (24 hours)
                    for h in range(n_hours):
                        u_tile = u_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        v_tile = v_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        u_tile = np.nan_to_num(u_tile, nan=0.0).astype(np.float16)
                        v_tile = np.nan_to_num(v_tile, nan=0.0).astype(np.float16)
                        u_tile.tofile(f)
                        v_tile.tofile(f)
                    
                    if has_wind:
                        # Extract wind for this tile
                        wind_lon_mask = (wind_lons >= lon_min) & (wind_lons < lon_max)
                        wind_lat_mask = (wind_lats >= lat_min) & (wind_lats < lat_max)
                        wind_lon_idx = np.where(wind_lon_mask)[0]
                        wind_lat_idx = np.where(wind_lat_mask)[0]
                        
                        if len(wind_lon_idx) > 0 and len(wind_lat_idx) > 0:
                            wind_nlon = len(wind_lon_idx)
                            wind_nlat = len(wind_lat_idx)
                            
                            f.write(struct.pack('<I', wind_nlon))
                            f.write(struct.pack('<I', wind_nlat))
                            f.write(struct.pack('<I', n_wind_steps))
                            
                            for h in range(n_wind_steps):
                                u_w = u_wind_global[h, wind_lat_idx[0]:wind_lat_idx[-1]+1, wind_lon_idx[0]:wind_lon_idx[-1]+1]
                                v_w = v_wind_global[h, wind_lat_idx[0]:wind_lat_idx[-1]+1, wind_lon_idx[0]:wind_lon_idx[-1]+1]
                                u_w = np.nan_to_num(u_w, nan=0.0).astype(np.float16)
                                v_w = np.nan_to_num(v_w, nan=0.0).astype(np.float16)
                                u_w.tofile(f)
                                v_w.tofile(f)
                            
                            for h in range(n_wind_steps):
                                s = skt_global[h, wind_lat_idx[0]:wind_lat_idx[-1]+1, wind_lon_idx[0]:wind_lon_idx[-1]+1]
                                s = np.nan_to_num(s, nan=273.15).astype(np.float16)
                                s.tofile(f)
                        else:
                            f.write(struct.pack('<I', 0))
                            f.write(struct.pack('<I', 0))
                            f.write(struct.pack('<I', 0))
                    else:
                        f.write(struct.pack('<I', 0))
                        f.write(struct.pack('<I', 0))
                        f.write(struct.pack('<I', 0))
                
                tiles += 1
            except Exception as e:
                print(f"      ⚠️ Tile ({tilex},{tiley}) failed: {e}")
                if tile_path.exists():
                    tile_path.unlink()
    
    ds.close()
    
    # Upload tiles for this day
    uploaded = 0
    for bin_file in day_dir.glob("*.bin"):
        key = f"tiles/{date.strftime('%Y/%m/%d')}/{bin_file.name}"
        try:
            s3.upload_file(str(bin_file), BUCKET, key)
            uploaded += 1
        except Exception as e:
            print(f"      ❌ Upload failed: {bin_file.name} - {e}")
    
    print(f"    ✅ Tiled and uploaded: {uploaded}/{tiles} tiles")
    
    # Clean up
    shutil.rmtree(day_dir)
    os.remove(nc_file)
    if wind_file and Path(wind_file).exists():
        os.remove(wind_file)
    
    return tiles


def main():
    print(f"\n{'='*60}")
    print(f"🌊 Daily tile update: {datetime.utcnow().strftime('%Y-%m-%d %H:%M UTC')}")
    print(f"{'='*60}")
    
    # Delete oldest day from R2
    print("\n🗑️  Step 1: Removing days outside rolling window...")
    delete_oldest_day()
    
    # Prepare directories
    TILES_DIR.mkdir(parents=True, exist_ok=True)
    NC_DIR.mkdir(parents=True, exist_ok=True)
    WIND_DIR.mkdir(parents=True, exist_ok=True)
    
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    
    total_tiles = 0
    
    print(f"\n📡 Step 2: Processing {FORECAST_DAYS} days...")
    
    for day_offset in range(FORECAST_DAYS):
        day_date = yesterday + timedelta(days=day_offset)
        print(f"\n  Day {day_offset+1}/{FORECAST_DAYS}: {day_date.date()}")
        
        # Download SMOC for this day
        nc_file = download_smoc_day(day_date)
        if not nc_file:
            print(f"    ⚠️ Skipping day {day_date.date()} - SMOC download failed")
            continue
        
        # Download ECMWF (only once per run, reuse)
        wind_file = None
        wind_file = download_ecmwf_day(day_date)
        
        # Tile and upload this day
        tiles = tile_and_upload_day(day_date, wind_file)
        total_tiles += tiles
        
        print(f"    Cumulative tiles: {total_tiles}")
    
    print(f"\n✅ Complete: {total_tiles} tiles uploaded")


if __name__ == "__main__":
    main()