#!/usr/bin/env python3
"""
Daily forecast update: download SMOC + ECMWF, tile together, upload to R2.
Maintains a 40-day rolling window (30 days hindcast + 10 days forecast).

Wind and SST stored as separate arrays in the same tile at native 0.25° resolution.
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
from scipy.interpolate import RegularGridInterpolator
# import datetime
import cartopy.crs as ccrs
import matplotlib.pyplot as plt
import numpy as np
import xarray as xr

# ===== CONFIG =====
ACCOUNT_ID = "d733df1598b4cde0f885a7ef0db5ccb8"
ACCESS_KEY = "036e8c8eeb522c92c85bea2783ee28f1"
SECRET_KEY = "4d92d6cda85083ffb841da100b49aa8ccb9609ac7ccd169ca678730bcde5ba32"
BUCKET = "driftmap-tiles"
FORECAST_DAYS = 10
HINDCAST_DAYS = 30
WINDOW_DAYS = HINDCAST_DAYS + FORECAST_DAYS

TILE_SIZE = 10.0
N_LON_TILES = 36
N_LAT_TILES = 17

# Wind grid (0.25°)
WIND_LON_STEP = 0.25
WIND_LAT_STEP = 0.25
WIND_LON_MIN = -180.0
WIND_LON_MAX = 180.0
WIND_LAT_MIN = -80.0
WIND_LAT_MAX = 90.0

# Local paths
BASE_DIR = Path("D:/projects/driftmap/data")
NC_DIR = BASE_DIR / "smoc_nc"
WIND_DIR = BASE_DIR / "ecmwf"
TILES_DIR = BASE_DIR / "forecast_tiles_smoc_combined"
VIZ_DIR = BASE_DIR / "visualization"

s3 = boto3.client(
    's3',
    endpoint_url=f'https://{ACCOUNT_ID}.r2.cloudflarestorage.com',
    aws_access_key_id=ACCESS_KEY,
    aws_secret_access_key=SECRET_KEY,
    region_name='auto',
)


def clear_folder(folder):
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True, exist_ok=True)
    print(f"  Cleared: {folder}")


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


def delete_forecast_window():
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    deleted = 0
    for offset in range(0, FORECAST_DAYS):
        d = yesterday + timedelta(days=offset)
        prefix = f"tiles/{d.strftime('%Y/%m/%d')}/"
        deleted += delete_prefix(prefix)
    print(f"  Deleted forecast window ({yesterday.date()} to {(yesterday + timedelta(days=FORECAST_DAYS-1)).date()}): {deleted} tiles")
    return deleted


def download_smoc_forecast():
    import copernicusmarine
    
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    end_date = today + timedelta(days=FORECAST_DAYS - 1)
    
    copernicusmarine.login(username=".", password=".")
    NC_DIR.mkdir(parents=True, exist_ok=True)
    
    out_file = str(NC_DIR / f"smoc_forecast_{today.strftime('%Y%m%d')}.nc")
    
    copernicusmarine.subset(
        dataset_id="cmems_mod_glo_phy_anfc_merged-uv_PT1H-i",
        variables=["utotal", "vtotal"],
        minimum_longitude=-180,
        maximum_longitude=179.916,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=yesterday.strftime("%Y-%m-%dT00:00:00"),
        end_datetime=end_date.strftime("%Y-%m-%dT23:00:00"),
        minimum_depth=0.49402499198913574,
        maximum_depth=0.49402499198913574,
        output_filename=out_file,
    )
    return out_file


def download_ecmwf_forecast():
    """Download ECMWF wind + SST — 6-hourly, 11 days."""
    from ecmwf.opendata import Client
    
    today = datetime.utcnow()
    yesterday = today - timedelta(days=1)
    date_str = yesterday.strftime("%Y-%m-%d")
    
    WIND_DIR.mkdir(parents=True, exist_ok=True)
    
    # Try AWS source, fallback to ecmwf
    for source in ["aws", "ecmwf"]:
        try:
            client = Client(source=source)
            out_file = str(WIND_DIR / f"ecmwf_{date_str.replace('-', '')}_12z.grib2")
            
            client.retrieve(
                date=date_str,
                time="00",
                step=list(range(0, 264, 6)),
                param=["10u", "10v", "skt"],
                target=out_file,
                type="fc",
                levtype="sfc",
            )
            return out_file
        except Exception as e:
            print(f"  {source} source failed: {e}")
    return None


def find_existing_nc():
    if not NC_DIR.exists():
        return None
    nc_files = sorted(NC_DIR.glob("smoc_forecast_*.nc"), reverse=True)
    return str(nc_files[0]) if nc_files else None


def find_existing_wind():
    if not WIND_DIR.exists():
        return None
    wind_files = sorted(WIND_DIR.glob("ecmwf_*.grib2"), reverse=True)
    return str(wind_files[0]) if wind_files else None


def extract_wind_for_tile(u_wind_global, v_wind_global, skt_global, 
                          wind_lons, wind_lats, tilex, tiley, n_wind_steps):
    """Extract wind data at native 0.25° resolution for a tile."""
    lon_min = -180 + TILE_SIZE * tilex
    lon_max = lon_min + TILE_SIZE
    lat_min = -80 + TILE_SIZE * tiley
    lat_max = lat_min + TILE_SIZE
    
    lon_mask = (wind_lons >= lon_min) & (wind_lons < lon_max)
    lat_mask = (wind_lats >= lat_min) & (wind_lats < lat_max)
    lon_idx = np.where(lon_mask)[0]
    lat_idx = np.where(lat_mask)[0]
    
    if len(lon_idx) == 0 or len(lat_idx) == 0:
        return None, None, None, 0, 0
    
    # Extract at native resolution (no interpolation)
    u_tile = u_wind_global[:, lat_idx[0]:lat_idx[-1]+1, lon_idx[0]:lon_idx[-1]+1]
    v_tile = v_wind_global[:, lat_idx[0]:lat_idx[-1]+1, lon_idx[0]:lon_idx[-1]+1]
    sst_tile = skt_global[:, lat_idx[0]:lat_idx[-1]+1, lon_idx[0]:lon_idx[-1]+1]
    
    return u_tile, v_tile, sst_tile, len(lon_idx), len(lat_idx)


def tile_one_day(u_day, v_day, lons, lats, n_hours, date, output_base,
                 u_wind_global, v_wind_global, skt_global, wind_lons, wind_lats, n_wind_steps):
    """Tile a single day with currents + wind + SST."""
    day_dir = output_base / date.strftime("%Y/%m/%d")
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
            
            # Extract wind at native resolution for this tile
            u_wind_tile, v_wind_tile, sst_tile, wind_nlon, wind_nlat = \
                extract_wind_for_tile(u_wind_global, v_wind_global, skt_global,
                                     wind_lons, wind_lats, tilex, tiley, n_wind_steps)
            
            if u_wind_tile is None:
                continue
            
            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
            
            try:
                with open(tile_path, 'wb') as f:
                    # Header: current grid dimensions
                    f.write(struct.pack('<I', len(lon_idx)))    # n_lon (current)
                    f.write(struct.pack('<I', len(lat_idx)))    # n_lat (current)
                    f.write(struct.pack('<I', 1))               # n_levels
                    f.write(struct.pack('<f', 0.0))             # level value
                    
                    # Current data: n_hours × (u + v)
                    for h in range(n_hours):
                        u_tile = u_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        v_tile = v_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        u_tile = np.nan_to_num(u_tile, nan=0.0).astype(np.float16)
                        v_tile = np.nan_to_num(v_tile, nan=0.0).astype(np.float16)
                        u_tile.tofile(f)
                        v_tile.tofile(f)
                    
                    # Wind grid header
                    f.write(struct.pack('<I', wind_nlon))       # n_lon (wind)
                    f.write(struct.pack('<I', wind_nlat))       # n_lat (wind)
                    f.write(struct.pack('<I', n_wind_steps))    # n_wind_steps
                    
                    # Wind data: n_wind_steps × (u + v)
                    for h in range(n_wind_steps):
                        u_w = np.nan_to_num(u_wind_tile[h, :, :], nan=0.0).astype(np.float16)
                        v_w = np.nan_to_num(v_wind_tile[h, :, :], nan=0.0).astype(np.float16)
                        u_w.tofile(f)
                        v_w.tofile(f)
                    
                    # SST data: n_wind_steps × skt
                    for h in range(n_wind_steps):
                        s = np.nan_to_num(sst_tile[h, :, :], nan=273.15).astype(np.float16)
                        s.tofile(f)
                
                tiles += 1
            except Exception as e:
                print(f"  ⚠️ Skipping tile ({tilex},{tiley}) on {date.date()}: {e}")
                if tile_path.exists():
                    tile_path.unlink()
                continue
    
    return tiles


def tile_all_days(nc_file, wind_file):
    """Tile the forecast with currents + wind + SST."""
    ds = xr.open_dataset(nc_file, chunks={'time': 24})
    ds_wind = xr.open_dataset(wind_file, engine="cfgrib")
    
    lons = ds['longitude'].values
    lats = ds['latitude'].values
    times = ds['time'].values
    
    # Extract wind data
    u_wind = ds_wind['u10'].values  # (step, lat, lon)
    v_wind = ds_wind['v10'].values
    skt = ds_wind['skt'].values if 'skt' in ds_wind else np.zeros_like(u_wind)
    wind_lons = ds_wind['longitude'].values
    wind_lats = ds_wind['latitude'].values
    n_wind_steps = u_wind.shape[0]
    
    print(f"  Wind steps: {n_wind_steps} (6-hourly)")
    print(f"  Wind grid: {len(wind_lats)} × {len(wind_lons)}")
    
    n_total_hours = len(times)
    hours_per_day = 24
    n_days = n_total_hours // hours_per_day
    
    clear_folder(TILES_DIR)
    total_tiles = 0
    
    print(f"  Current hours: {n_total_hours}")
    print(f"  Days: {n_days}")
    
    for d in range(n_days):
        start = d * hours_per_day
        end = start + hours_per_day
        
        u_day = ds['utotal'].isel(depth=0, time=slice(start, end)).values
        v_day = ds['vtotal'].isel(depth=0, time=slice(start, end)).values
        times_day = times[start:end]
        
        ts = (times_day[0] - np.datetime64('1970-01-01T00:00:00')) / np.timedelta64(1, 's')
        date = datetime.utcfromtimestamp(ts)
        
        tiles = tile_one_day(u_day, v_day, lons, lats, hours_per_day, date, TILES_DIR,
                            u_wind, v_wind, skt, wind_lons, wind_lats, n_wind_steps)
        total_tiles += tiles
        
        if d % 2 == 0 or d == n_days - 1:
            print(f"  Tiled {date.date()}: {tiles} tiles ({d+1}/{n_days} days)")
    
    ds.close()
    ds_wind.close()
    print(f"  Total: {total_tiles} tiles across {n_days} days")
    return TILES_DIR

def nc_to_png():
    today = datetime.utcnow()
    ds = xr.open_dataset(str(NC_DIR / f"smoc_forecast_{today.strftime('%Y%m%d')}.nc"))

    # 2. Calculate speed
    u = ds['utotal'].isel(time=24, depth=0)
    v = ds['vtotal'].isel(time=24, depth=0)
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
        VIZ_DIR / 'currents.png',
        dpi=150,
        transparent=True
    )
    plt.close(fig)



def upload_all(tile_dir):
    files = []
    
    # First, generate and add the PNG if it exists
    png_path = VIZ_DIR / 'currents.png'
    if png_path.exists():
        print(f"  📸 Found currents PNG: {png_path}")
        files.append((png_path, "currents.png"))
    
    # Add all tile files
    for root, dirs, fnames in os.walk(tile_dir):
        for fname in fnames:
            if fname.endswith('.bin'):
                local = Path(root) / fname
                rel = str(local.relative_to(tile_dir)).replace('\\', '/')
                files.append((local, f"tiles/{rel}"))

    print(f"  Uploading {len(files)} files ({len([f for f in files if f[1]=='currents.png'])} PNG, {len([f for f in files if f[1]!='currents.png'])} tiles)...")
    uploaded = 0
    
    with ThreadPoolExecutor(max_workers=12) as executor:
        futures = {}
        for local, key in files:
            # For PNG, set proper content type
            if key == "currents.png":
                extra_args = {'ContentType': 'image/png'}
            else:
                extra_args = {}
            
            future = executor.submit(
                s3.upload_file, 
                str(local), 
                BUCKET, 
                key, 
                ExtraArgs=extra_args
            )
            futures[future] = key
        
        for future in as_completed(futures):
            try:
                future.result()
                uploaded += 1
            except Exception as e:
                print(f"  ❌ Upload failed for {futures[future]}: {e}")
            if uploaded % 200 == 0:
                print(f"  {uploaded}/{len(files)}")
    
    print(f"  ✅ Uploaded {uploaded}/{len(files)}")
    return uploaded
def main():
    retain = "--retain" in sys.argv
    
    print(f"\n{'='*60}")
    print(f"🌊 Daily update: {datetime.utcnow().strftime('%Y-%m-%d %H:%M UTC')}")
    print(f"   Mode: {'Retain (skip download)' if retain else 'Full download'}")
    print(f"   Window: {HINDCAST_DAYS}d hindcast + {FORECAST_DAYS}d forecast = {WINDOW_DAYS}d total")
    print(f"{'='*60}")
    
    # Step 1: Delete oldest day
    print("\n🗑️  Step 1: Removing day outside rolling window...")
    delete_oldest_day()
    
    # Step 2: Delete old forecast
    # print("\n🗑️  Step 2: Removing old forecast window...")
    # delete_forecast_window()
    
    # Step 3: Download SMOC
    if retain:
        print("\n📂 Step 3a: Using existing SMOC NC file...")
        nc_file = find_existing_nc()
        if nc_file is None:
            print("  ❌ No existing NC file found.")
            return
        print(f"  Using: {nc_file}")
    else:
        print("\n📡 Step 3a: Downloading SMOC forecast...")
        nc_file = download_smoc_forecast()
        print(f"  Downloaded: {nc_file}")
    
    # Step 3b: Download ECMWF wind
    if retain:
        print("\n📂 Step 3b: Using existing ECMWF file...")
        wind_file = find_existing_wind()
        if wind_file is None:
            print("  ⚠️ No existing wind file. Downloading...")
            wind_file = download_ecmwf_forecast()
    else:
        print("\n📡 Step 3b: Downloading ECMWF wind forecast...")
        wind_file = download_ecmwf_forecast()
    
    if wind_file is None:
        print("  ❌ Failed to get ECMWF data. Continuing with currents only.")
        wind_file = None
    
    # Step 4: Tile
    print("\n🔨 Step 4: Tiling forecast...")
    tile_dir = tile_all_days(nc_file, wind_file)
    try:
        nc_to_png()
        print("  ✅ PNG generated")
    except Exception as e:
        print(f"  ⚠️ PNG generation failed: {e}")
    # Step 5: Upload
    print("\n☁️  Step 5: Uploading to R2...")
    upload_all(tile_dir)
    
    # Step 6: Clean up
    # print("\n🧹 Step 6: Cleaning up local files...")
    # clear_folder(TILES_DIR)
    # if not retain:
    #     clear_folder(NC_DIR)
    #     clear_folder(WIND_DIR)
    
    print(f"\n✅ Update complete")


if __name__ == "__main__":
    main()