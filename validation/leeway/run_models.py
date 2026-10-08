#!/usr/bin/env python3
"""
Run a grid of cross-validation simulations: N points across the central
Pacific, each simulated once by OpenDrift and once by proteus-cli.

Each run writes:
  runs/run_<i>/
    opendrift_output.nc
    proteus_centroids.json
    config.json          (the CLI config used)
    meta.json            (release point, run index, timing)

Run from tests/opendrift_validation/.
"""
import json
import subprocess
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np
import xarray as xr

from opendrift.models.leeway import Leeway
from opendrift.readers import (
    reader_netCDF_CF_generic,
    add_standard_name_for_surface_grib_variables,
)


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
SMOC_PATH = Path("../../data/currents/smoc_20260919.nc")
ECMWF_GRIB = Path("../../data/wind/ecmwf_2026-09-19_00z.grib2")
ECMWF_NC = Path("../../data/wind/ecmwf_2026-09-19_00z.nc")

START_TIME = datetime(2026, 9, 19, 0, 0)
DURATION_HOURS = 48
N_PARTICLES = 100000
TIME_STEP_SECONDS = 900
TIME_STEP_OUTPUT_SECONDS = 3600

GRID_LON_MIN, GRID_LON_MAX = -177.5, -147.5
GRID_LAT_MIN, GRID_LAT_MAX = -7.5, 7.5
GRID_N = 8

# CLI
CLI_BIN = Path("../../target/release/proteus.exe")   # Windows
CLI_STEPS = DURATION_HOURS * (3600 // TIME_STEP_OUTPUT_SECONDS)  # 18
CLI_TIME_STEP_MINUTES = TIME_STEP_OUTPUT_SECONDS / 60.0           # 60

RUNS_DIR = Path("runs")


# ---------------------------------------------------------------------------
# Step 1: Convert ECMWF GRIB2 -> CF NetCDF (once)
# ---------------------------------------------------------------------------
def prepare_ecmwf_netcdf():
    if ECMWF_NC.exists():
        print(f"  ECMWF NetCDF already exists: {ECMWF_NC}")
        return

    print("  Converting ECMWF GRIB2 -> CF NetCDF...")
    ds = xr.open_dataset(ECMWF_GRIB, engine="cfgrib")
    ds = ds.drop_vars("time", errors="ignore")
    ds = ds.swap_dims({"step": "valid_time"}).rename({"valid_time": "time"})
    ds = ds.drop_vars(
        ["step", "heightAboveGround", "surface"], errors="ignore"
    )
    ds = add_standard_name_for_surface_grib_variables(ds)
    ds = ds.drop_vars("skt", errors="ignore")
    ds.to_netcdf(ECMWF_NC)
    print(f"  Wrote {ECMWF_NC}")


# ---------------------------------------------------------------------------
# Grid
# ---------------------------------------------------------------------------
def make_grid():
    lons = np.linspace(GRID_LON_MIN, GRID_LON_MAX, GRID_N)
    lats = np.linspace(GRID_LAT_MIN, GRID_LAT_MAX, GRID_N)
    points = [(float(lon), float(lat)) for lat in lats for lon in lons]
    assert len(points) == GRID_N * GRID_N
    return points


# ---------------------------------------------------------------------------
# OpenDrift run
# ---------------------------------------------------------------------------
def run_opendrift(run_dir, release_lon, release_lat, reader_smoc, reader_wind):
    out_nc = run_dir / "opendrift_output.nc"

    o = Leeway(loglevel=30)
    o.add_reader([reader_smoc, reader_wind])
    o.set_config('drift:advection_scheme', 'runge-kutta4')
    o.seed_elements(
        lon=release_lon,
        lat=release_lat,
        radius=0,
        number=N_PARTICLES,
        time=START_TIME,
        object_type=1,
        jibe_probability=0
    )

    t0 = time.time()

    o.run(
        duration=timedelta(hours=DURATION_HOURS),
        time_step=TIME_STEP_SECONDS,
        time_step_output=TIME_STEP_OUTPUT_SECONDS,
        outfile=str(out_nc),
    )
    return time.time() - t0


# ---------------------------------------------------------------------------
# CLI run
# ---------------------------------------------------------------------------
def write_cli_config(run_dir, release_lon, release_lat):
    config = {
        "start_date": START_TIME.strftime("%Y-%m-%d %H:%M"),
        "tracer_type": "sar",
        "tracer_json": {
            "downwind": [0.96, 0.00, 12.00],
            "right": [0.54, 0.00, 9.40],
            "left": [-0.54, 0.00, 9.40],
            "jibe_probability": 0.00,
            "random_orientation": False,
            "capsizing": False,
            "capsize_threshold": 30,
            "capsize_fraction": 0.4,     
            "capsize_sigma": 5,
        },
        "releases": [
            {
                "lon": release_lon,
                "lat": release_lat,
                "radius": 0.0,
                "schedule": [{"amount": 1.0, "duration": 0}],
            }
        ],
        "particles": N_PARTICLES,
        "time_step_minutes": CLI_TIME_STEP_MINUTES,
        "advection": "rk4",
        "diffusion": "constant",
        "diffusion_coeffs": [0.0, 0.0],
        "steps": CLI_STEPS,
        "tile_url": "validation-tiles",
        "output": "proteus_centroids.json",
    }
    cfg_path = run_dir / "config.json"
    with open(cfg_path, "w") as f:
        json.dump(config, f, indent=2)
    return cfg_path


def run_cli(run_dir, release_lon, release_lat):
    cfg_path = write_cli_config(run_dir, release_lon, release_lat)

    t0 = time.time()
    result = subprocess.run(
        [str(CLI_BIN.resolve()), str(cfg_path.resolve())],
        cwd=str(run_dir.resolve()),
        capture_output=True,
        text=True,
    )
    elapsed = time.time() - t0

    if result.returncode != 0:
        print(f"    ❌ CLI failed (exit {result.returncode})")
        print(f"    stdout: {result.stdout[-500:]}")
        print(f"    stderr: {result.stderr[-500:]}")
        return elapsed, False
    return elapsed, True

# ---------------------------------------------------------------------------
# Save OpenDrift centroids as JSON (for debugging/alignment)
# ---------------------------------------------------------------------------
def save_opendrift_centroids(run_dir):
    """Read opendrift_output.nc, write opendrift_centroids.json with
    the same schema as proteus_centroids.json: a list of
    {step, lon, lat} entries, one per output time.
    """
    nc_path = run_dir / "opendrift_output.nc"
    ds = xr.open_dataset(nc_path)

    # Locate lon/lat variables (naming varies by OpenDrift version)
    lon_name = next((n for n in ("lon", "longitude") if n in ds), None)
    lat_name = next((n for n in ("lat", "latitude") if n in ds), None)
    if lon_name is None or lat_name is None:
        ds.close()
        raise RuntimeError(f"no lon/lat variables in {nc_path}")

    lons = ds[lon_name].values
    lats = ds[lat_name].values

    # Times: OpenDrift writes a 'time' coordinate.
    time_name = next((n for n in ds.coords if "time" in n.lower()), "time")
    times = ds[time_name].values

    ds.close()

    # Normalize shape to (n_steps, n_particles)
    if lons.shape[0] == 1:
        lons = lons[0]
        lats = lats[0]
    if lons.ndim == 2 and lons.shape[0] > lons.shape[1]:
        lons = lons.T
        lats = lats.T

    n_steps = lons.shape[0]
    out = []
    for i in range(n_steps):
        slon = lons[i]
        slat = lats[i]
        mask = ~np.isnan(slon) & ~np.isnan(slat)
        if mask.any():
            out.append({
                "step": i,
                "lon": float(np.mean(slon[mask])),
                "lat": float(np.mean(slat[mask])),
                "time": str(times[i]) if i < len(times) else None,
                "n_active": int(mask.sum()),
            })

    json_path = run_dir / "opendrift_centroids.json"
    with open(json_path, "w") as f:
        json.dump(out, f, indent=2)

    return json_path
# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    print("Step 1: Preparing ECMWF NetCDF")
    prepare_ecmwf_netcdf()

    print("\nStep 2: Building readers")
    reader_smoc = reader_netCDF_CF_generic.Reader(
        str(SMOC_PATH),
        standard_name_mapping={
            "utotal": "x_sea_water_velocity",
            "vtotal": "y_sea_water_velocity",
        },
    )
    reader_wind = reader_netCDF_CF_generic.Reader(str(ECMWF_NC))

    points = make_grid()
    RUNS_DIR.mkdir(exist_ok=True)

    total = len(points)
    print(f"\nStep 3: Running {total} grid points")
    print(f"  Grid: lon [{GRID_LON_MIN}, {GRID_LON_MAX}], "
          f"lat [{GRID_LAT_MIN}, {GRID_LAT_MAX}]")
    print(f"  Points: {GRID_N} x {GRID_N} = {total}")

    for i, (lon, lat) in enumerate(points):
        run_dir = RUNS_DIR / f"run_{i:03d}"
        run_dir.mkdir(exist_ok=True)

        meta = {
            "run_index": i,
            "release_lon": lon,
            "release_lat": lat,
            "start_time": START_TIME.isoformat(),
            "duration_hours": DURATION_HOURS,
            "n_particles": N_PARTICLES,
        }

        print(f"\n[{i+1:>2}/{total}] ({lon:.2f}, {lat:.2f})")

        if not (run_dir / "opendrift_output.nc").exists():
            print("    OpenDrift...", end="", flush=True)
            try:
                od_sec = run_opendrift(run_dir, lon, lat, reader_smoc, reader_wind)
                meta["opendrift_seconds"] = od_sec
                print(f" {od_sec:.1f}s")
            except Exception as e:
                print(f" ❌ {e}")
                meta["opendrift_error"] = str(e)
        else:
            print("    OpenDrift... cached")

        # Always (re)write the OpenDrift centroids JSON from the NetCDF.
        if (run_dir / "opendrift_output.nc").exists():
            try:
                save_opendrift_centroids(run_dir)
            except Exception as e:
                print(f"    ⚠️ could not save opendrift_centroids.json: {e}")

        if not (run_dir / "proteus_centroids.json").exists():
            print("    DriftMap...", end="", flush=True)
            cli_sec, ok = run_cli(run_dir, lon, lat)
            if ok:
                meta["cli_seconds"] = cli_sec
                print(f" {cli_sec:.1f}s")
            else:
                meta["cli_error"] = "nonzero exit"
        else:
            print("    DriftMap... cached")

        with open(run_dir / "meta.json", "w") as f:
            json.dump(meta, f, indent=2)

    print(f"\n✅ Done. Outputs in {RUNS_DIR}/")


if __name__ == "__main__":
    main()