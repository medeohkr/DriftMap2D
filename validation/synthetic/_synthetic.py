"""Shared helpers for synthetic field generators."""
import struct
from pathlib import Path
import numpy as np
import xarray as xr

STEP = 1.0 / 12.0
LON_MIN = -180.0
LAT_MIN = 0.0
LON_MAX = -170.0
LAT_MAX = 10.0
N_LON = int(round((LON_MAX - LON_MIN) / STEP)) + 1
N_LAT = int(round((LAT_MAX - LAT_MIN) / STEP)) + 1
N_HOURS = 25
N_WIND_STEPS = 5
START_TIME = "2026-09-19T00:00:00"
TILE_DATE = (2026, 9, 19)


def write_netcdf(u_fn, v_fn, out_path):
    """u_fn(lats, lons) -> (n_lat, n_lon), same for v_fn."""
    lons = np.arange(-180.0, 180.0, STEP)
    lats = np.arange(-80.0, 90.0, STEP)
    u = u_fn(lats, lons).astype(np.float16).astype(np.float32)
    v = v_fn(lats, lons).astype(np.float16).astype(np.float32)
    times = np.array([np.datetime64(START_TIME), np.datetime64(START_TIME) + np.timedelta64(1, "h")])
    u_3d = np.broadcast_to(u[None, :, :], (2, len(lats), len(lons)))
    v_3d = np.broadcast_to(v[None, :, :], (2, len(lats), len(lons)))
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
    Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    ds.to_netcdf(out_path)


def write_tile(u_fn, v_fn, out_path):
    Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    lats = np.arange(LAT_MIN, LAT_MAX + STEP / 2, STEP)[:N_LAT]
    lons = np.arange(LON_MIN, LON_MAX + STEP / 2, STEP)[:N_LON]
    u_f16 = u_fn(lats, lons).astype(np.float16)
    v_f16 = v_fn(lats, lons).astype(np.float16)
    with open(out_path, "wb") as f:
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", 1))
        f.write(struct.pack("<f", 0.0))
        for _ in range(N_HOURS):
            u_f16.tofile(f)
            v_f16.tofile(f)
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", N_WIND_STEPS))
        uw = np.zeros((N_LAT, N_LON), dtype=np.float16)
        vw = np.zeros((N_LAT, N_LON), dtype=np.float16)
        sst = np.full((N_LAT, N_LON), np.float16(288.15))
        for _ in range(N_WIND_STEPS):
            uw.tofile(f)
            vw.tofile(f)
            sst.tofile(f)