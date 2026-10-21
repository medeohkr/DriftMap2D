#!/usr/bin/env python3
"""Time-varying synthetic forcing: u(t) = U0 + A sin(2π t / T), v = 0."""
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

N_HOURS = 25
N_WIND_STEPS = 5

U0 = 0.5
AMP = 0.3
PERIOD_HOURS = 6.0

START_TIME = "2026-09-19T00:00:00"
TILE_DATE = (2026, 9, 19)

OUT = Path(__file__).resolve().parent / "time_varying"
NC_PATH = OUT / "input.nc"
TILE_PATH = OUT / "tiles" / f"{TILE_DATE[0]:04d}/{TILE_DATE[1]:02d}/{TILE_DATE[2]:02d}/000_008.bin"


def u_at(t_hours):
    """u(t) in m/s, t in hours."""
    return U0 + AMP * np.sin(2 * np.pi * t_hours / PERIOD_HOURS)


def write_netcdf():
    """CF NetCDF with hourly u(t), v = 0."""
    # Global grid (for OpenDrift)
    lons = np.arange(-180.0, 180.0, STEP)
    lats = np.arange(-80.0, 90.0, STEP)

    # Hourly time steps
    t0 = np.datetime64(START_TIME)
    times = np.array([t0 + np.timedelta64(h, "h") for h in range(N_HOURS)])

    # u(t, lat, lon) = u(t), v = 0
    u_val = np.array([u_at(float(h)) for h in range(N_HOURS)], dtype=np.float16).astype(np.float32)
    u_3d = np.broadcast_to(u_val[:, None, None], (N_HOURS, len(lats), len(lons))).copy()
    v_3d = np.zeros_like(u_3d, dtype=np.float32)

    ds = xr.Dataset(
        {
            "utotal": (("time", "latitude", "longitude"), u_3d, {
                "standard_name": "x_sea_water_velocity",
                "units": "m s-1",
            }),
            "vtotal": (("time", "latitude", "longitude"), v_3d, {
                "standard_name": "y_sea_water_velocity",
                "units": "m s-1",
            }),
        },
        coords={
            "time": ("time", times),
            "latitude": ("latitude", lats, {"units": "degrees_north"}),
            "longitude": ("longitude", lons, {"units": "degrees_east"}),
        },
    )
    OUT.mkdir(parents=True, exist_ok=True)
    ds.to_netcdf(NC_PATH)
    print(f"Wrote {NC_PATH} ({NC_PATH.stat().st_size / 1e6:.1f} MB)")
    print(f"  u(t=0h)  = {u_at(0):.6f}")
    print(f"  u(t=1.5h) = {u_at(1.5):.6f}")
    print(f"  u(t=3h)  = {u_at(3):.6f}")


def write_tile():
    """Tile with 25 hourly steps of u(t), v = 0, at lat [0, 10], lon [-180, -170]."""
    TILE_PATH.parent.mkdir(parents=True, exist_ok=True)

    N = N_LAT * N_LON
    with open(TILE_PATH, "wb") as f:
        # Currents header
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", 1))
        f.write(struct.pack("<f", 0.0))

        # Currents: 25 hours, u(t) then v = 0
        for h in range(N_HOURS):
            u_val = np.float16(u_at(float(h)))
            u_arr = np.full(N, u_val, dtype=np.float16)
            v_arr = np.zeros(N, dtype=np.float16)
            u_arr.tofile(f)
            v_arr.tofile(f)

        # Wind header (zero)
        f.write(struct.pack("<I", N_LON))
        f.write(struct.pack("<I", N_LAT))
        f.write(struct.pack("<I", N_WIND_STEPS))
        uw = np.zeros(N, dtype=np.float16)
        vw = np.zeros(N, dtype=np.float16)
        sst = np.full(N, np.float16(288.15))
        for _ in range(N_WIND_STEPS):
            uw.tofile(f)
            vw.tofile(f)
            sst.tofile(f)

    print(f"Wrote {TILE_PATH} ({TILE_PATH.stat().st_size / 1e6:.2f} MB)")


def verify():
    """Check the tile's hourly u values against the analytic u(t)."""
    import struct as st
    with open(TILE_PATH, "rb") as f:
        n_lon = st.unpack("<I", f.read(4))[0]
        n_lat = st.unpack("<I", f.read(4))[0]
        n_depths = st.unpack("<I", f.read(4))[0]
        for _ in range(n_depths):
            f.read(4)
        cells = n_lon * n_lat
        for h in range(5):
            u_arr = np.frombuffer(f.read(cells * 2), dtype=np.float16).astype(np.float32)
            f.read(cells * 2)  # skip v
            expected = u_at(float(h))
            got = float(u_arr[0])
            match = abs(got - expected) < 1e-3
            print(f"  hour {h}: expected {expected:.4f}, tile {got:.4f} {'OK' if match else 'MISMATCH'}")
    return True


if __name__ == "__main__":
    import struct
    write_netcdf()
    write_tile()
    print("\nVerifying first 5 hours:")
    verify()
    print("Done.")