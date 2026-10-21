#!/usr/bin/env python3
"""Time-varying synthetic forcing over 48 hours: u(t) = U0 + A sin(2π t / T), v = 0.

Writes:
  input.nc                          (49 hourly steps, CF NetCDF)
  tiles/2026/09/19/000_008.bin      (day 0: hours 0..24)
  tiles/2026/09/20/000_008.bin      (day 1: hours 24..48)
"""
import struct
from pathlib import Path
import numpy as np
import xarray as xr

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
STEP = 1.0 / 12.0
LON_MIN, LON_MAX = -180.0, -170.0
LAT_MIN, LAT_MAX = 0.0, 10.0
N_LON = int(round((LON_MAX - LON_MIN) / STEP)) + 1
N_LAT = int(round((LAT_MAX - LAT_MIN) / STEP)) + 1

N_HOURS = 49                              # hours 0..48 inclusive
N_WIND_STEPS = 5

U0 = 0.5
AMP = 0.3
PERIOD_HOURS = 6.0

START_TIME = "2026-09-19T00:00:00"
TILE_DATES = [(2026, 9, 19), (2026, 9, 20)]

OUT = Path(__file__).resolve().parent / "time_varying"
NC_PATH = OUT / "input.nc"


def u_at(t_hours):
    return U0 + AMP * np.sin(2 * np.pi * t_hours / PERIOD_HOURS)


def write_netcdf():
    lons = np.arange(-180.0, 180.0, STEP)
    lats = np.arange(-80.0, 90.0, STEP)

    t0 = np.datetime64(START_TIME)
    times = np.array([t0 + np.timedelta64(h, "h") for h in range(N_HOURS)])

    u_val = np.array([u_at(float(h)) for h in range(N_HOURS)], dtype=np.float16).astype(np.float32)
    u_3d = np.broadcast_to(u_val[:, None, None], (N_HOURS, len(lats), len(lons))).copy()
    v_3d = np.zeros_like(u_3d, dtype=np.float32)

    ds = xr.Dataset(
        {
            "utotal": (("time", "latitude", "longitude"), u_3d, {
                "standard_name": "x_sea_water_velocity", "units": "m s-1"}),
            "vtotal": (("time", "latitude", "longitude"), v_3d, {
                "standard_name": "y_sea_water_velocity", "units": "m s-1"}),
        },
        coords={
            "time": ("time", times),
            "latitude": ("latitude", lats, {"units": "degrees_north"}),
            "longitude": ("longitude", lons, {"units": "degrees_east"}),
        },
    )
    OUT.mkdir(parents=True, exist_ok=True)
    ds.to_netcdf(NC_PATH)
    print(f"Wrote {NC_PATH} ({NC_PATH.stat().st_size / 1e6:.1f} MB, {N_HOURS} hours)")


def write_tile(day_offset, out_path):
    """Write one tile covering 25 hourly steps starting at day_offset * 24."""
    out_path.parent.mkdir(parents=True, exist_ok=True)

    cells = N_LAT * N_LON
    with open(out_path, "wb") as f:
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", 1))
        f.write(struct.pack("<f", 0.0))

        for h in range(25):
            t = day_offset * 24 + h
            u_val = np.float16(u_at(float(t)))
            u_arr = np.full(cells, u_val, dtype=np.float16)
            v_arr = np.zeros(cells, dtype=np.float16)
            u_arr.tofile(f)
            v_arr.tofile(f)

        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", N_WIND_STEPS))
        uw = np.zeros(cells, dtype=np.float16)
        vw = np.zeros(cells, dtype=np.float16)
        sst = np.full(cells, np.float16(288.15))
        for _ in range(N_WIND_STEPS):
            uw.tofile(f)
            vw.tofile(f)
            sst.tofile(f)

    print(f"Wrote {out_path} ({out_path.stat().st_size / 1e6:.2f} MB)")


def main():
    write_netcdf()
    for day_offset, (y, m, d) in enumerate(TILE_DATES):
        out_path = OUT / "tiles" / f"{y:04d}/{m:02d}/{d:02d}/000_008.bin"
        write_tile(day_offset, out_path)


if __name__ == "__main__":
    main()