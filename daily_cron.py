#!/usr/bin/env python3
"""
Daily forecast update: download ECMWF once, then SMOC day by day,
tile immediately, upload to R2.
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
import cartopy.crs as ccrs
import matplotlib.pyplot as plt

# ===== CONFIG =====

if sys.platform == "linux":
    BASE_DIR = Path("./data")
else:
    BASE_DIR = Path("D:/projects/driftmap/data")

BUCKET = "driftmap-tiles"
FORECAST_DAYS = 11  # Yesterday + 10 forecast days
HINDCAST_DAYS = 30

TILE_SIZE = 10.0
N_LON_TILES = 36
N_LAT_TILES = 17

NC_DIR = BASE_DIR / "smoc_nc"
WIND_DIR = BASE_DIR / "ecmwf"
TILES_DIR = BASE_DIR / "forecast_tiles"

# R2 credentials
ACCOUNT_ID = os.environ.get("ACCOUNT_ID", "").strip()
ACCESS_KEY = os.environ.get("ACCESS_KEY", "").strip()
SECRET_KEY = os.environ.get("SECRET_KEY", "").strip()
CMEMS_USER = os.environ.get("CMEMS_USER", "").strip()
CMEMS_PASS = os.environ.get("CMEMS_PASS", "").strip()

if not all([ACCOUNT_ID, ACCESS_KEY, SECRET_KEY]):
    print("❌ ERROR: Missing R2 credentials")
    sys.exit(1)

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
    for i in range(3):
        oldest = datetime.utcnow() - timedelta(days=HINDCAST_DAYS + 1 + i)
        prefix = f"tiles/{oldest.strftime('%Y/%m/%d')}/"
        deleted = delete_prefix(prefix)
        if deleted > 0:
            print(f"  Deleted oldest days {oldest.date()}: {deleted} tiles")


def download_ecmwf():
    """Download ECMWF wind + SST once (contains all forecast steps)."""
    from ecmwf.opendata import Client
    
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    date_str = yesterday.strftime("%Y-%m-%d")
    
    WIND_DIR.mkdir(parents=True, exist_ok=True)
    out_file = str(WIND_DIR / f"ecmwf_{date_str.replace('-', '')}_00z.grib2")
    
    print(f"📡 Downloading ECMWF forecast (initialized {date_str})...")
    
    for source in ["ecmwf", "aws"]:
        try:
            client = Client(source=source)
            client.retrieve(
                date=date_str,
                time="00",
                step=list(range(0, 264, 6)),  # 0 to 264 = 11 days of 6-hourly
                param=["10u", "10v", "skt"],
                target=out_file,
                type="fc",
                levtype="sfc",
            )
            if Path(out_file).exists():
                print(f"  ✅ ECMWF: {Path(out_file).stat().st_size / 1e6:.1f} MB")
                return out_file
        except Exception as e:
            print(f"  {source} source failed: {e}")
    
    print("  ❌ Failed to get ECMWF data")
    return None


def download_smoc_day(date):
    """Download a single day of SMOC data (24 hours, hourly)."""
    import copernicusmarine
    
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
        maximum_longitude=179.9166717529297,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=date.strftime("%Y-%m-%dT00:00:00"),
        end_datetime=date.strftime("%Y-%m-%dT23:00:00"),
        minimum_depth=0.49402499198913574,
        maximum_depth=0.49402499198913574,
        output_filename=out_file,
        force_download=True,
    )
    
    if Path(out_file).exists() and Path(out_file).stat().st_size > 0:
        print(f"    ✅ SMOC: {Path(out_file).stat().st_size / 1e6:.1f} MB")
        return out_file
    else:
        print(f"    ❌ SMOC download failed")
        return None


def tile_and_upload_day(date, wind_file, wind_data_cache):
    """Tile a single day and upload immediately, then delete."""
    
    nc_file = NC_DIR / f"smoc_{date.strftime('%Y%m%d')}.nc"
    if not nc_file.exists():
        print(f"    ❌ SMOC file not found: {nc_file}")
        return 0
    
    # Load SMOC data
    ds = xr.open_dataset(str(nc_file))
    lons = ds['longitude'].values
    lats = ds['latitude'].values
    
    # Current data: 24 hours (hourly)
    u_day = ds['utotal'].isel(depth=0).values
    v_day = ds['vtotal'].isel(depth=0).values
    n_hours = 24
    
    # Get wind data for this specific day from the cached global wind
    day_offset = (date - wind_data_cache['reference_date']).days
    wind_start_step = day_offset * 4
    wind_end_step = wind_start_step + 4
    print(f"    Wind steps: {wind_start_step} to {wind_end_step}, day offset: {day_offset}")
    
    u_wind_global = wind_data_cache['u_wind'][wind_start_step:wind_end_step]
    v_wind_global = wind_data_cache['v_wind'][wind_start_step:wind_end_step]
    sst_global = wind_data_cache['sst'][wind_start_step:wind_end_step]
    wind_lons = wind_data_cache['lons']
    wind_lats = wind_data_cache['lats']
    n_wind_steps = 4
    
    # FIX 1: Reverse latitude dimension to ascending order (-90 to 90)
    # ECMWF provides latitudes descending (90 to -90), but we need ascending
    wind_lats = wind_lats[::-1]  # Now -90 to 90
    u_wind_global = u_wind_global[:, ::-1, :]  # Reverse lat dimension for all steps
    v_wind_global = v_wind_global[:, ::-1, :]
    sst_global = sst_global[:, ::-1, :]
    
    print(f"    Normalized wind coords: lon[{wind_lons[0]:.1f} to {wind_lons[-1]:.1f}], "
          f"lat[{wind_lats[0]:.1f} to {wind_lats[-1]:.1f}]")
    
    day_dir = TILES_DIR / date.strftime("%Y/%m/%d")
    day_dir.mkdir(parents=True, exist_ok=True)
    tiles = 0
    
    for tilex in range(N_LON_TILES):
        lon_min = -180 + TILE_SIZE * tilex
        lon_max = lon_min + TILE_SIZE
        # For the last longitude tile (tilex=35)
        if tilex == N_LON_TILES - 1:
            lon_idx = np.where((lons >= lon_min) & (lons <= 180.0))[0]  # Include 180.0
        else:
            lon_idx = np.where((lons >= lon_min) & (lons < lon_max))[0]
        if len(lon_idx) == 0:
            continue
        
        for tiley in range(N_LAT_TILES):
            lat_min = -80 + TILE_SIZE * tiley
            lat_max = lat_min + TILE_SIZE
            lat_idx = np.where((lats >= lat_min) & (lats < lat_max))[0]
            if len(lat_idx) == 0:
                continue
            
            # Extract wind for this tile
            wind_lon_mask = (wind_lons >= lon_min) & (wind_lons < lon_max)
            wind_lat_mask = (wind_lats >= lat_min) & (wind_lats < lat_max)
            wind_lon_indices = np.where(wind_lon_mask)[0]
            wind_lat_indices = np.where(wind_lat_mask)[0]
            
            if len(wind_lon_indices) == 0 or len(wind_lat_indices) == 0:
                print(f"      ⚠️ No wind data for tile ({tilex},{tiley})")
                wind_nlon = 0
                wind_nlat = 0
            else:
                wind_nlon = len(wind_lon_indices)
                wind_nlat = len(wind_lat_indices)
            
            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
            
            try:
                with open(tile_path, 'wb') as f:
                    # Header
                    f.write(struct.pack('<I', len(lon_idx)))   # n_lon
                    f.write(struct.pack('<I', len(lat_idx)))   # n_lat
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
                    
                    # Wind header
                    f.write(struct.pack('<I', wind_nlon))
                    f.write(struct.pack('<I', wind_nlat))
                    f.write(struct.pack('<I', n_wind_steps))
                    
                    if wind_nlon > 0 and wind_nlat > 0:
                        # FIX 2: Interleave wind and SST by step (u, v, sst for each step)
                        for h in range(n_wind_steps):
                            u_w = u_wind_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]
                            v_w = v_wind_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]
                            s = sst_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]
                            
                            u_w = np.nan_to_num(u_w, nan=0.0).astype(np.float16)
                            v_w = np.nan_to_num(v_w, nan=0.0).astype(np.float16)
                            s = np.nan_to_num(s, nan=273.15).astype(np.float16)
                            
                            u_w.tofile(f)
                            v_w.tofile(f)
                            s.tofile(f)
                
                tiles += 1
            except Exception as e:
                print(f"      ⚠️ Tile ({tilex},{tiley}) failed: {e}")
                import traceback
                traceback.print_exc()
                if tile_path.exists():
                    tile_path.unlink()
    
    ds.close()
    
    # Upload tiles for this day to R2
    print(f"    📤 Uploading {tiles} tiles to R2...")
    uploaded = 0
    for bin_file in day_dir.glob("*.bin"):
        key = f"tiles/{date.strftime('%Y/%m/%d')}/{bin_file.name}"
        try:
            s3.upload_file(str(bin_file), BUCKET, key)
            uploaded += 1
        except Exception as e:
            print(f"      ❌ Upload failed: {bin_file.name} - {e}")
    
    print(f"    ✅ Tiled and uploaded: {uploaded}/{tiles} tiles")
    
    # Clean up local files
    shutil.rmtree(day_dir)
    if nc_file.exists():
        os.remove(nc_file)
    
    return uploaded

def generate_overlay(smoc_path):
    ds = xr.open_dataset(smoc_path)

    # 2. Calculate speed
    u = ds['utotal'].isel(time=0, depth=0)
    v = ds['vtotal'].isel(time=0, depth=0)
    speed = np.sqrt(u**2 + v**2)

    # Extract raw geographic coordinates from NetCDF
    lon = ds.longitude.values
    lat = ds.latitude.values
    lon_min, lon_max = float(lon.min()), float(lon.max())
    lat_min, lat_max = float(lat.min()), float(lat.max())

    # 3. Create Figure using Cartopy Web Mercator (EPSG:3857)
    fig = plt.figure(figsize=(10, 8), facecolor='none')

    # ccrs.Mercator.GOOGLE matches MapLibre's internal engine
    ax = fig.add_subplot(1, 1, 1, projection=ccrs.Mercator.GOOGLE)

    # --- MODERN SYNTAX TO STRIP BACKGROUNDS & BORDERS ---
    ax.patch.set_visible(False)          # Replaces background_patch
    ax.spines['geo'].set_visible(False)  # Replaces outline_patch
    ax.axis('off')

    # Set the map view limits strictly to your NetCDF bounding box
    ax.set_extent([lon_min, lon_max, lat_min, lat_max], crs=ccrs.PlateCarree())

    # Extract the actual pixel dimensions of your data array
    data_height, data_width = speed.shape
    print(f"Original Data Resolution: {data_width}x{data_height} pixels")

    # Plot data forcing Cartopy to match your exact grid dimensions
    im = ax.imshow(
        speed,
        extent=[lon_min, lon_max, lat_min, lat_max],
        transform=ccrs.PlateCarree(),
        cmap='viridis',
        vmin=0,
        vmax=1,
        origin='lower',
        interpolation='bilinear',
        interpolation_stage='data',
        regrid_shape=(data_height, data_width)  # <-- FORCE FULL RESOLUTION HERE
    )


    # 4. Save without margins or borders 
    plt.subplots_adjust(left=0, right=1, bottom=0, top=1)

    plt.savefig(
        'currents.png',
        dpi=150,
        transparent=True
    )
    plt.close(fig)


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
    
    # Step 2: Download ECMWF once
    print("\n📡 Step 2: Downloading ECMWF forecast...")
    wind_file = download_ecmwf()
    if not wind_file:
        print("  ❌ Failed to get ECMWF data. Exiting.")
        sys.exit(1)
    
    # Step 3: Load wind data
    print("\n📂 Step 3: Loading ECMWF data into memory...")
    ds_wind = xr.open_dataset(wind_file, engine="cfgrib")
    u_wind_global = ds_wind['u10'].values
    v_wind_global = ds_wind['v10'].values
    sst_global = ds_wind['skt'].values if 'skt' in ds_wind else np.zeros_like(u_wind_global)
    wind_lons = ds_wind['longitude'].values
    wind_lats = ds_wind['latitude'].values
    
    wind_data_cache = {
        'u_wind': u_wind_global,
        'v_wind': v_wind_global,
        'sst': sst_global,
        'lons': wind_lons,
        'lats': wind_lats,
        'reference_date': datetime.utcnow() - timedelta(days=1),
    }
    ds_wind.close()
    print(f"  ✅ Wind data loaded: {u_wind_global.shape[0]} steps, {len(wind_lats)} lat, {len(wind_lons)} lon")
    
    # Step 4: Process each day
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    
    total_tiles = 0
    print(f"\n📡 Step 4: Processing {FORECAST_DAYS} days...")
    
    for day_offset in range(FORECAST_DAYS):
        day_date = yesterday + timedelta(days=day_offset)
        print(f"\n  Day {day_offset+1}/{FORECAST_DAYS}: {day_date.date()}")
        
        nc_file = download_smoc_day(day_date)
        if not nc_file:
            print(f"    ⚠️ Skipping day {day_date.date()} - SMOC download failed")
            continue
        if day_offset == 1:
            today = datetime.utcnow()
            today_nc = NC_DIR / f"smoc_{today.strftime('%Y%m%d')}.nc"
            
            if today_nc.exists():
                generate_overlay(str(today_nc))
                overlay_path = "currents.png"
                if Path(overlay_path).exists():
                    print(f"  📤 Uploading overlay to R2...")
                    s3.upload_file(
                        overlay_path, 
                        BUCKET, 
                        "currents.png",
                        ExtraArgs={'ContentType': 'image/png', 'CacheControl': 'max-age=3600'}
                    )
                    print(f"  ✅ Overlay uploaded")
                    os.remove(overlay_path)
            else:
                print(f"  ⚠️ Today's SMOC file not found, skipping overlay")
        tiles = tile_and_upload_day(day_date, wind_file, wind_data_cache)
        total_tiles += tiles
        print(f"    Cumulative tiles: {total_tiles}")

    # Step 5: Generate overlay (OUTSIDE the for loop)
    print("\n🎨 Step 5: Generating current overlay...")


    # Clean up
    if wind_file and Path(wind_file).exists():
        os.remove(wind_file)
        print(f"\n🧹 Cleaned up wind file")
    
    print(f"\n✅ Complete: {total_tiles} tiles uploaded")


if __name__ == "__main__":
    main()