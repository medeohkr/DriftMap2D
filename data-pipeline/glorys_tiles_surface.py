import numpy as np
import xarray as xr
from pathlib import Path
import struct


DATA_DIR = Path("data/glorys_10yr_global")
OUTPUT_DIR = Path("data/glorys_tiles_surface")

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

TILE_SIZE = 10.0                     # degrees
LON_STEP = 1/12                     # 0.08333 degrees
LAT_STEP = 1/12

# GLORYS grid bounds
LON_MIN = -180.0
LON_MAX = 180.0 - LON_STEP          # 179.91666666
LAT_MIN = -80.0
LAT_MAX = 90.0

N_LON_TILES = int(360 / TILE_SIZE)   # 72
N_LAT_TILES = int(170 / TILE_SIZE)   # 34

def get_tile_range(tilex, tiley):
    tileLonMin = LON_MIN + TILE_SIZE * tilex
    tileLonMax = min(LON_MAX, LON_MIN + TILE_SIZE * (tilex + 1))
    tileLatMin = LAT_MIN + TILE_SIZE * (tiley)
    tileLatMax = min(LAT_MAX, LAT_MIN + TILE_SIZE * (tiley + 1))
    return tileLonMin, tileLonMax, tileLatMin, tileLatMax



def main():
    for file in DATA_DIR.iterdir():
        filename = Path(file).stem
        parts = filename.split('_')
        yearmonth = parts[1]
        year = int(yearmonth[0:4])
        month = int(yearmonth[4:6])

        year_dir = OUTPUT_DIR / f"{year:04d}"
        year_dir.mkdir(parents=True, exist_ok=True)

        month_dir = year_dir / f"{month:02d}"
        month_dir.mkdir(parents=True, exist_ok=True)

        ds = xr.open_dataset(file)

        u = ds['uo'].isel(depth=0)
        v = ds['vo'].isel(depth=0)
        m = ds['mlotst']
        lons = ds['longitude'].values
        lats = ds['latitude'].values
        time = ds.dims['time']

        for t in range(time):
            u_t = u.isel(time=t).values
            v_t = v.isel(time=t).values
            m_t = m.isel(time=t).values

            day_dir = month_dir / f"{t:02d}"
            day_dir.mkdir(parents=True, exist_ok=True)

            for tilex in range(N_LON_TILES):
                tileLonMin, tileLonMax, _, _  = get_tile_range(tilex, 0)
                lons_mask = (lons >= tileLonMin) & (lons < tileLonMax)
                lons_indices = np.where(lons_mask)[0]

                for tiley in range(N_LAT_TILES):
                    _, _, tileLatMin, tileLatMax = get_tile_range(0, tiley)
                    lats_mask = (lats >= tileLatMin) & (lats < tileLatMax)
                    lats_indices = np.where(lats_mask)[0]

                    u_tile = u_t[np.ix_(lats_indices, lons_indices)]
                    v_tile = v_t[np.ix_(lats_indices, lons_indices)]
                    m_tile = m_t[np.ix_(lats_indices, lons_indices)]

                    tile_file = day_dir / f"{tilex:03d}_{tiley:03d}.bin"

                    with open(tile_file, 'wb') as f:
                        f.write(struct.pack('<I', len(lons_indices)))
                        f.write(struct.pack('<I', len(lats_indices)))
                        u_tile.astype(np.float16).tofile(f)
                        v_tile.astype(np.float16).tofile(f)
                        m_tile.astype(np.float16).tofile(f)
        ds.close()

if __name__ == "__main__":
    main()

