#!/usr/bin/env python3
"""
Compare OpenDrift and DriftMap2D centroids for a synthetic case.

Usage:
    python analyze.py shear
    python analyze.py constant
    python analyze.py time_varying

Reports per-point error (km) and the analytic solution for reference.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from shared.haversine import haversine_km


# ---------------------------------------------------------------------------
# Test config (must match run.py and the generators)
# ---------------------------------------------------------------------------
START_TIME_HOURS = 1.0   # duration of each run, in hours

# WGS84 constants at the equator (used for the analytic solution)
METERS_PER_DEGREE_LAT = 110_574.0       # meridional, at lat 0
METERS_PER_DEGREE_LON = 111_320.0       # parallel, at lat 0


# ---------------------------------------------------------------------------
# Case parameters
# ---------------------------------------------------------------------------
CASE_PARAMS = {
    "constant": {
        "type": "constant",
        "u": 0.5,
        "v": 0.5,
    },
    "shear": {
        "type": "shear",
        "u0": 0.3,
        "shear": 0.02,
        "v": 0.05,
    },
    "time_varying": {
        "type": "time_varying",
        "u0": 0.5,
        "amp": 0.3,
        "period_h": 6.0,
        "v": 0.0,
    },
}


# ---------------------------------------------------------------------------
# Analytic solutions
# ---------------------------------------------------------------------------
def analytic_constant(lon0, lat0, u, v):
    """Constant field. 1 hour of drift."""
    t = START_TIME_HOURS * 3600.0
    lat_t = lat0 + v * t / METERS_PER_DEGREE_LAT
    lon_t = lon0 + u * t / METERS_PER_DEGREE_LON
    return lon_t, lat_t


def analytic_shear(lon0, lat0, u0, shear, v):
    """u(lat) = u0 + shear*lat, v constant. 1 hour of drift."""
    t = START_TIME_HOURS * 3600.0
    lat_t = lat0 + v * t / METERS_PER_DEGREE_LAT
    dx = u0 * t + shear * (lat0 * t + 0.5 * v * t * t / METERS_PER_DEGREE_LAT)
    lon_t = lon0 + dx / METERS_PER_DEGREE_LON
    return lon_t, lat_t


def analytic_time_varying(lon0, lat0, u0, amp, period_h):
    """u(t) = u0 + amp * sin(2π t / period_h), v = 0. 1 hour of drift."""
    t = START_TIME_HOURS * 3600.0
    period_s = period_h * 3600.0
    omega = 2 * np.pi / period_s
    dx = u0 * t + (amp / omega) * (1.0 - np.cos(omega * t))
    lon_t = lon0 + dx / METERS_PER_DEGREE_LON
    lat_t = lat0
    return lon_t, lat_t


def analytic_for_case(case, lon0, lat0):
    """Dispatch to the right analytic function based on case name."""
    if case not in CASE_PARAMS:
        raise SystemExit(
            f"Unknown case '{case}'. Known cases: {', '.join(CASE_PARAMS)}"
        )
    params = CASE_PARAMS[case]
    kind = params["type"]
    if kind == "constant":
        return analytic_constant(lon0, lat0, params["u"], params["v"])
    if kind == "shear":
        return analytic_shear(lon0, lat0, params["u0"], params["shear"], params["v"])
    if kind == "time_varying":
        return analytic_time_varying(
            lon0, lat0, params["u0"], params["amp"], params["period_h"]
        )
    raise SystemExit(f"Unknown case type '{kind}' for case '{case}'")


# ---------------------------------------------------------------------------
# Load centroids
# ---------------------------------------------------------------------------
def load_centroids(path):
    with open(path) as f:
        data = json.load(f)
    return [(r["step"], r["lon"], r["lat"]) for r in data]


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    if len(sys.argv) < 2:
        print("Usage: python analyze.py <case>")
        print(f"  case: one of {', '.join(CASE_PARAMS)}")
        sys.exit(1)

    case = sys.argv[1]
    case_dir = Path(__file__).resolve().parent / case

    run_dirs = sorted(
        p for p in case_dir.iterdir() if p.is_dir() and p.name.startswith("run_")
    )
    if not run_dirs:
        raise SystemExit(f"No run_* directories in {case_dir}. Run run.py first.")

    print(
        f"{'point':<12} "
        f"{'OD final':<24} "
        f"{'DM final':<24} "
        f"{'OD-DM (m)':<12} "
        f"{'vs analytic (m)':<24}"
    )
    print("-" * 100)

    for run_dir in run_dirs:
        name = run_dir.name.replace("run_", "")

        od_path = run_dir / "opendrift_centroids.json"
        dm_path = run_dir / "proteus_centroids.json"
        if not od_path.exists() or not dm_path.exists():
            print(f"{name:<12} (missing output: {'OD' if not od_path.exists() else 'DM'})")
            continue

        od = load_centroids(od_path)
        dm = load_centroids(dm_path)

        od_final = od[-1]
        dm_final = dm[-1]
        _, lon0, lat0 = od[0]  # release point

        sep_m = haversine_km(od_final[1], od_final[2], dm_final[1], dm_final[2]) * 1000.0

        ana_lon, ana_lat = analytic_for_case(case, lon0, lat0)
        od_vs_ana_m = haversine_km(od_final[1], od_final[2], ana_lon, ana_lat) * 1000.0
        dm_vs_ana_m = haversine_km(dm_final[1], dm_final[2], ana_lon, ana_lat) * 1000.0

        print(
            f"{name:<12} "
            f"({od_final[1]:>10.6f}, {od_final[2]:>9.6f})  "
            f"({dm_final[1]:>10.6f}, {dm_final[2]:>9.6f})  "
            f"{sep_m:<12.4f} "
            f"OD:{od_vs_ana_m:.4f} DM:{dm_vs_ana_m:.4f}"
        )


if __name__ == "__main__":
    main()