#!/usr/bin/env python3
"""
Write a single synthetic tile with constant u = 0.5, v = 0.5 m/s.

The tile is written to the location the CLI expects for tile (0, 0)
on the given day, so a config pointing at tile_url="validation-tiles"
with base_url pointing at a local HTTP server will pick it up.

Binary format (matches tiler output):
    [u32 n_lon][u32 n_lat][u32 n_depths][f32 depth_0]...
    n_depths * n_hours * (u f16 [n_lat*n_lon], v f16 [n_lat*n_lon])
    [u32 n_lon_wind][u32 n_lat_wind][u32 n_wind_steps]
    n_wind_steps * (u f16, v f16, sst f16)
"""
import struct
from pathlib import Path
import numpy as np

# ---------------------------------------------------------------------------
# Config — matches the CLI's expectations
# ---------------------------------------------------------------------------
TILE_SIZE = 10.0
# Grid resolution: 1/12 degree in longitude, 1/12 degree in latitude
STEP = 1.0 / 12.0

# Tile (0, 0) spans lon [-180, -170], lat [-80, -70]
LON_MIN = -180.0
LON_MAX = -170.0
LAT_MIN = -80.0
LAT_MAX = -70.0

# Number of grid points in each direction (inclusive of both edges)
N_LON = int(round((LON_MAX - LON_MIN) / STEP))
N_LAT = int(round((LAT_MAX - LAT_MIN) / STEP))

# Currents: 25 hourly steps (matches the 25-hour overlap convention)
N_HOURS = 25

# Wind: 5 six-hourly steps (matches the 5-step overlap convention)
N_WIND_STEPS = 5
N_LON_WIND = 40
N_LAT_WIND = 40

# Constant velocities (m/s)
U_CONST = 0.5
V_CONST = 0.5

# Wind velocities (m/s) — set to zero so only currents contribute
U_WIND = 0.0
V_WIND = 0.0

# SST in Kelvin (ignored for current-only tests, but must be present)
SST_K = 288.15

# Date the tile belongs to — must match the CLI config's start_date
# Format: YYYY/MM/DD directory structure
TILE_DATE = (2026, 9, 19)

# Output location — change this to your local tile serving root
OUT_DIR = Path("./synthetic-tiles/tiles")


# ---------------------------------------------------------------------------
# Write the tile
# ---------------------------------------------------------------------------
def write_tile(out_path: Path):
    out_path.parent.mkdir(parents=True, exist_ok=True)

    with open(out_path, "wb") as f:
        # --- Currents header ---
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", 1))          # n_depths
        f.write(struct.pack("<f", 0.0))        # depth (surface)

        # --- Currents data: N_HOURS × (u, v) ---
        u_val = np.float16(U_CONST)
        v_val = np.float16(V_CONST)
        cells = N_LON * N_LAT
        u_arr = np.full(cells, u_val, dtype=np.float16)
        v_arr = np.full(cells, v_val, dtype=np.float16)
        for _ in range(N_HOURS):
            u_arr.tofile(f)
            v_arr.tofile(f)

        # --- Wind header ---
        f.write(struct.pack("<I", N_LON_WIND))
        f.write(struct.pack("<I", N_LAT_WIND))
        f.write(struct.pack("<I", N_WIND_STEPS))

        # --- Wind + SST: N_WIND_STEPS × (u, v, sst) ---
        uw = np.full(N_LON_WIND * N_LAT_WIND, np.float16(U_WIND), dtype=np.float16)
        vw = np.full(N_LON_WIND * N_LAT_WIND, np.float16(V_WIND), dtype=np.float16)
        sst = np.full(N_LON_WIND * N_LAT_WIND, np.float16(SST_K), dtype=np.float16)
        for _ in range(N_WIND_STEPS):
            uw.tofile(f)
            vw.tofile(f)
            sst.tofile(f)

    size = out_path.stat().st_size
    print(f"Wrote {out_path} ({size} bytes, {size/1024:.1f} KB)")


def main():
    year, month, day = TILE_DATE
    out_path = OUT_DIR / f"{year:04d}/{month:02d}/{day:02d}/000_000.bin"
    write_tile(out_path)

    # Expected size:
    #   header:       4 + 4 + 4 + 4 = 16
    #   currents:     25 * 121 * 121 * 2 * 2 = 1,464,100
    #   wind header:  12
    #   wind:         5 * 121 * 121 * 2 * 3 = 439,230
    #   total:        ~1,903,358 bytes
    expected = 16 + N_HOURS * 14400 * 2 * 2 + 12 + N_WIND_STEPS * 1600 * 2 * 3
    print(f"Expected size: {expected} bytes")


if __name__ == "__main__":
    main()