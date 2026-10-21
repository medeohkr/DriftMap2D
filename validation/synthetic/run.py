#!/usr/bin/env python3
"""
Run both simulators on a synthetic forcing case and compare.

Usage:
    python run.py shear
    python run.py constant

For each case (expected at validation/synthetic/<case>/), this:
  1. Runs OpenDrift with the local NetCDF input
  2. Runs DriftMap2D with the local tile input (via file:// URL)
  3. Saves centroids from both to the case directory
"""
import sys
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np

# Allow imports from validation/shared/
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from shared.opendrift_utils import save_centroids_from_nc
from shared.proteus_utils import run_cli


# ---------------------------------------------------------------------------
# Test config
# ---------------------------------------------------------------------------
START_TIME = datetime(2026, 9, 19, 0, 0)
if sys.argv[1] == "time_varying":
    DURATION_HOURS = 48
else:
    DURATION_HOURS = 1
N_PARTICLES = 1
TIME_STEP_SECONDS = 3600

# Release points: cell center, cell edge, cell corner
# (tile spans -180 to -170 lon, -80 to -70 lat, 1/12° cells)
STEP = 1.0 / 12.0
RELEASE_POINTS = {
    "center": (-175.0, 0.0),
    "edge_lon": (-175.0 + STEP / 2, 0.0),
    "corner": (-175.0 + STEP / 2, 0.0 + STEP / 2),
}


def run_opendrift(case_dir, release_lon, release_lat, nc_path):
    from opendrift.models.oceandrift import OceanDrift
    from opendrift.readers import reader_netCDF_CF_generic

    o = OceanDrift(loglevel=30)
    o.add_reader([reader_netCDF_CF_generic.Reader(str(nc_path))])
    o.set_config("drift:advection_scheme", "runge-kutta4")
    o.seed_elements(
        lon=release_lon, lat=release_lat, radius=0, number=N_PARTICLES,
        time=START_TIME,
    )
    out_nc = case_dir / "opendrift_output.nc"
    o.run(
        duration=timedelta(hours=DURATION_HOURS),
        time_step=TIME_STEP_SECONDS,
        time_step_output=TIME_STEP_SECONDS,
        outfile=str(out_nc),
    )
    save_centroids_from_nc(out_nc, case_dir / "opendrift_centroids.json")


def run_proteus(case_dir, release_lon, release_lat, tiles_dir):
    # Use a file:// URL pointing at the local tiles.
    tile_url = "file:///" + str(tiles_dir.resolve()).replace("\\", "/")
    config = {
        "start_date": START_TIME.strftime("%Y-%m-%d %H:%M"),
        "tracer_type": "generic",
        "tracer_json": {"wind_factor": 0.0, "wind_deflection": 0.0},
        "releases": [{
            "lon": release_lon, "lat": release_lat, "radius": 0.0,
            "schedule": [{"amount": 1.0, "duration": 0}],
        }],
        "particles": N_PARTICLES,
        "time_step_minutes": TIME_STEP_SECONDS / 60.0,
        "advection": "rk4",
        "diffusion": "constant",
        "diffusion_coeffs": [0.0, 0.0],
        "steps": int(DURATION_HOURS * 3600 / TIME_STEP_SECONDS),
        "tile_url": tile_url,
        "output": "proteus_centroids.json",
    }
    elapsed, result = run_cli(case_dir, config)
    if result.returncode != 0:
        print(f"    CLI failed ({result.returncode}):")
        print(result.stderr[-800:])
        return False
    return True


def main():
    if len(sys.argv) < 2:
        print("Usage: python run.py <case>")
        print("  case: one of the directories in validation/synthetic/ (e.g., 'shear')")
        sys.exit(1)

    case = sys.argv[1]
    case_dir = Path(__file__).resolve().parent / case
    nc_path = case_dir / "input.nc"
    tiles_dir = case_dir / "tiles"

    if not nc_path.exists():
        raise SystemExit(f"Missing {nc_path}. Run make_{case}.py first.")
    if not tiles_dir.exists():
        raise SystemExit(f"Missing {tiles_dir}. Run make_{case}.py first.")

    for name, (lon, lat) in RELEASE_POINTS.items():
        print(f"\n=== {case} / {name} ({lon:.4f}, {lat:.4f}) ===")
        point_dir = case_dir / f"run_{name}"
        point_dir.mkdir(exist_ok=True)

        print("  OpenDrift...", end="", flush=True)
        run_opendrift(point_dir, lon, lat, nc_path)
        print(" done")

        print("  DriftMap2D...", end="", flush=True)
        if not run_proteus(point_dir, lon, lat, tiles_dir):
            continue
        print(" done")

    print(f"\nAll runs complete. Analyze with: python analyze.py {case}")


if __name__ == "__main__":
    main()