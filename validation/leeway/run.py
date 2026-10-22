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

Run from any directory; paths are relative to validation/leeway/.
"""
import json
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np

from opendrift.models.leeway import Leeway
from opendrift.readers import reader_netCDF_CF_generic

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from shared.opendrift_utils import (
    prepare_ecmwf_netcdf as prepare_ecmwf_netcdf_shared,
    save_centroids_from_nc,
)
from shared.proteus_utils import run_cli as run_cli_shared


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
SMOC_PATH = (HERE.parent / "forcing/currents/smoc_20260919.nc")
ECMWF_GRIB = (HERE.parent / "forcing/wind/ecmwf_2026-09-19_00z.grib2")
ECMWF_NC = (HERE.parent / "forcing/wind/ecmwf_2026-09-19_00z.nc")

START_TIME = datetime(2026, 9, 19, 0, 0)
DURATION_HOURS = 48
N_PARTICLES = 100000
TIME_STEP_SECONDS = 3600

GRID_LON_MIN, GRID_LON_MAX = -177.5, -147.5
GRID_LAT_MIN, GRID_LAT_MAX = -7.5, 7.5
GRID_N = 8

# CLI
CLI_STEPS = DURATION_HOURS * (3600 // TIME_STEP_SECONDS)
CLI_TIME_STEP_MINUTES = TIME_STEP_SECONDS / 60.0

RUNS_DIR = HERE / "runs"


# ---------------------------------------------------------------------------
# Step 1: Convert ECMWF GRIB2 -> CF NetCDF (once)
# ---------------------------------------------------------------------------
def prepare_ecmwf_netcdf():
    if ECMWF_NC.exists():
        print(f"  ECMWF NetCDF already exists: {ECMWF_NC}")
        return

    print("  Converting ECMWF GRIB2 -> CF NetCDF...")
    prepare_ecmwf_netcdf_shared(ECMWF_GRIB, ECMWF_NC)
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
    o.set_config('drift:advection_scheme', 'euler')
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
        time_step_output=TIME_STEP_SECONDS,
        outfile=str(out_nc),
    )
    return time.time() - t0


# ---------------------------------------------------------------------------
# CLI run
# ---------------------------------------------------------------------------
def make_cli_config(release_lon, release_lat):
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
        "advection": "euler",
        "diffusion": "constant",
        "diffusion_coeffs": [0.0, 0.0],
        "steps": CLI_STEPS,
        "tile_url": "validation-tiles",
        "output": "proteus_centroids.json",
    }
    return config


def run_cli(run_dir, release_lon, release_lat):
    elapsed, result = run_cli_shared(
        run_dir, make_cli_config(release_lon, release_lat)
    )

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
    nc_path = run_dir / "opendrift_output.nc"
    json_path = run_dir / "opendrift_centroids.json"
    save_centroids_from_nc(nc_path, json_path)
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

        print("    OpenDrift...", end="", flush=True)
        try:
            od_sec = run_opendrift(run_dir, lon, lat, reader_smoc, reader_wind)
            meta["opendrift_seconds"] = od_sec
            print(f" {od_sec:.1f}s")
        except Exception as e:
            print(f" ❌ {e}")
            meta["opendrift_error"] = str(e)

        save_opendrift_centroids(run_dir)

        print("    DriftMap...", end="", flush=True)
        cli_sec, ok = run_cli(run_dir, lon, lat)
        if ok:
            meta["cli_seconds"] = cli_sec
            print(f" {cli_sec:.1f}s")
        else:
            meta["cli_error"] = "nonzero exit"

        with open(run_dir / "meta.json", "w") as f:
            json.dump(meta, f, indent=2)

    print(f"\n✅ Done. Outputs in {RUNS_DIR}/")


if __name__ == "__main__":
    main()