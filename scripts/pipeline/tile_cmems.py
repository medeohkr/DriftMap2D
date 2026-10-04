import numpy as np
import xarray as xr
from pathlib import Path
import struct
from datetime import datetime

DATA_DIR = Path("data/smoc_nc")
OUTPUT_DIR = Path("D:/projects/driftmap/web/data/forecast_tiles_smoc_combined")

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

TILE_SIZE = 10.0
LON_STEP = 1/12
LAT_STEP = 1/12

LON_MIN = -180.0
LON_MAX = 180.0
LAT_MIN = -80.0
LAT_MAX = 90.0

N_LON_TILES = int((LON_MAX - LON_MIN) / TILE_SIZE)  # 72
N_LAT_TILES = int((LAT_MAX - LAT_MIN) / TILE_SIZE)  # 34

SURFACE_DEPTH = 0.0
N_DEPTHS = 1
HOURS_PER_DAY = 24


def get_tile_bounds(tilex, tiley):
    lon_min = LON_MIN + TILE_SIZE * tilex
    lon_max = LON_MIN + TILE_SIZE * (tilex + 1)
    lat_min = LAT_MIN + TILE_SIZE * tiley
    lat_max = LAT_MIN + TILE_SIZE * (tiley + 1)
    return lon_min, lon_max, lat_min, lat_max


def process_day(u_day, v_day, times_day, lons, lats, year, month, day):
    """Process 24 hours into a single tile per grid cell."""
    n_hours = u_day.shape[0]
    day_dir = OUTPUT_DIR / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
    day_dir.mkdir(parents=True, exist_ok=True)
    
    tiles_written = 0
    
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
            
            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
            
            with open(tile_path, 'wb') as f:
                # Header
                f.write(struct.pack('<I', len(lon_indices)))
                f.write(struct.pack('<I', len(lat_indices)))
                f.write(struct.pack('<I', N_DEPTHS))
                f.write(struct.pack('<f', SURFACE_DEPTH))
                
                # Write all 24 hours of data
                for h in range(n_hours):
                    u_tile = u_day[h, :, :]
                    v_tile = v_day[h, :, :]
                    
                    u_tile = u_tile[np.ix_(lat_indices, lon_indices)]
                    v_tile = v_tile[np.ix_(lat_indices, lon_indices)]
                    
                    u_tile = np.nan_to_num(u_tile, nan=0.0, posinf=0.0, neginf=0.0)
                    v_tile = np.nan_to_num(v_tile, nan=0.0, posinf=0.0, neginf=0.0)
                    
                    u_tile.astype(np.float16).tofile(f)
                    v_tile.astype(np.float16).tofile(f)
            
            tiles_written += 1
    
    return tiles_written


def main():
    files = sorted([f for f in DATA_DIR.iterdir() if f.name.endswith('.nc')])
    
    if len(files) == 0:
        print(f"ERROR: No .nc files in {DATA_DIR.resolve()}")
        return
    
    file = files[0]
    print(f"Processing: {file.name}")
    
    ds = xr.open_dataset(file)
    
    lons = ds['longitude'].values
    lats = ds['latitude'].values
    times = ds['time'].values
    
    n_total_hours = len(times)
    n_days = n_total_hours // HOURS_PER_DAY
    
    print(f"  Total hours: {n_total_hours}")
    print(f"  Days: {n_days}")
    
    total_tiles = 0
    
    for d in range(n_days):
        start = d * HOURS_PER_DAY
        end = start + HOURS_PER_DAY
        
        u_day = ds['utotal'].isel(depth=0, time=slice(start, end)).values
        v_day = ds['vtotal'].isel(depth=0, time=slice(start, end)).values
        times_day = times[start:end]
        
        # Get date from first hour
        ts = (times_day[0] - np.datetime64('1970-01-01T00:00:00')) / np.timedelta64(1, 's')
        dt = datetime.utcfromtimestamp(ts)
        
        tiles = process_day(u_day, v_day, times_day, lons, lats, 
                           dt.year, dt.month, dt.day)
        total_tiles += tiles
        
        if d % 5 == 0:
            print(f"  Day {dt.date()}: {tiles} tiles")
    
    ds.close()
    
    print(f"\n{'='*60}")
    print(f"COMPLETE: {total_tiles} total tiles across {n_days} days")
    print(f"Output: {OUTPUT_DIR.resolve()}")
    print(f"{'='*60}")


if __name__ == "__main__":
    main()