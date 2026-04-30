import numpy as np
import xarray as xr
from pathlib import Path
import struct

DATA_DIR = Path("D:\projects\driftmap\web\data/forecast")
OUTPUT_DIR = Path("D:\projects\driftmap\web\data/forecast_tiles")

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

TILE_SIZE = 10.0                     # degrees
LON_STEP = 1/12                     # 0.08333 degrees
LAT_STEP = 1/12

# GLORYS grid bounds
LON_MIN = -180.0
LON_MAX = 180.0
LAT_MIN = -80.0
LAT_MAX = 90.0

N_LON_TILES = int((LON_MAX - LON_MIN) / TILE_SIZE)   # 72
N_LAT_TILES = int((LAT_MAX - LAT_MIN) / TILE_SIZE)   # 34

# Surface only: single depth value
SURFACE_DEPTH = 0.0
N_DEPTHS = 1

def get_tile_bounds(tilex, tiley):
    lon_min = LON_MIN + TILE_SIZE * tilex
    lon_max = LON_MIN + TILE_SIZE * (tilex + 1)
    lat_min = LAT_MIN + TILE_SIZE * tiley
    lat_max = LAT_MIN + TILE_SIZE * (tiley + 1)
    return lon_min, lon_max, lat_min, lat_max

def main():
    for file in DATA_DIR.iterdir():
        if not file.name.endswith('.nc'):
            continue
            
        filename = Path(file).stem
        parts = filename.split('_')
        yearmonth = parts[1]
        year = int(yearmonth[0:4])
        month = int(yearmonth[4:6])

        print(f"Processing: {year}-{month:02d}")

        ds = xr.open_dataset(file)

        # Surface only
        u = ds['uo'].isel(depth=0)  # Shape: (time, lat, lon)
        v = ds['vo'].isel(depth=0)
        
        lons = ds['longitude'].values
        lats = ds['latitude'].values
        n_time = ds.dims['time']

        for t in range(n_time):
            u_t = u.isel(time=t).values  # Shape: (lat, lon)
            v_t = v.isel(time=t).values
            day = t + 1
            day_dir = OUTPUT_DIR / f"{year:04d}" / f"{month:02d}" / f"{day:02d}"
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

                    # Extract tile (2D surface only)
                    u_tile = u_t[np.ix_(lat_indices, lon_indices)]  # Shape: (lat, lon)
                    v_tile = v_t[np.ix_(lat_indices, lon_indices)]

                    tile_file = day_dir / f"{tilex:03d}_{tiley:03d}.bin"

                    with open(tile_file, 'wb') as f:
                        # Header: n_lon, n_lat, n_depths
                        f.write(struct.pack('<I', len(lon_indices)))   # n_lon
                        f.write(struct.pack('<I', len(lat_indices)))   # n_lat
                        f.write(struct.pack('<I', N_DEPTHS))           # n_depths = 1
                        
                        # Write the single depth value (0.0)
                        f.write(struct.pack('<f', SURFACE_DEPTH))
                        
                        # Write u and v data (float16)
                        u_tile.astype(np.float16).tofile(f)
                        v_tile.astype(np.float16).tofile(f)

            print(f"  Day {day:02d} done")

        ds.close()
        print(f"Done with {year}-{month:02d}\n")

if __name__ == "__main__":
    main()