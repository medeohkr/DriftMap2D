#!/usr/bin/env python3
"""
Cross-validation of DriftMap's advection core against OpenDrift.

Scenario:
  - Release: Strait of Hormuz (56.5°E, 26.5°N), radius 0
  - Particles: 10,000
  - Start: 2026-09-19 00:00 UTC
  - Duration: 72 hours
  - Timestep: 900s (15 min)
  - Forcing: SMOC currents + ECMWF 10m winds
  - Drift: wind_factor=0.02, no deflection, no diffusion

Output:
  - opendrift_output.nc (raw trajectory)
  - opendrift_final_positions.json (final positions for comparison)
  - side_by_side.mp4 / .gif (visualization)
"""

import json
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np
import xarray as xr
import matplotlib.pyplot as plt
import cartopy.crs as ccrs
import cartopy.feature as cfeature

from opendrift.models.oceandrift import OceanDrift
from opendrift.readers import (
    reader_netCDF_CF_generic,
    add_standard_name_for_surface_grib_variables,
)


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
SMOC_PATH = Path("./data/currents/smoc_20260919.nc")
ECMWF_GRIB = Path("./data/wind/ecmwf_2026-09-19_00z.grib2")
ECMWF_NC = Path("./data/wind/ecmwf_2026-09-19_00z.nc")

RELEASE_LON = 62.97
RELEASE_LAT = 22.68
START_TIME = datetime(2026, 9, 19, 0, 0)
DURATION_HOURS = 18
N_PARTICLES = 10000
TIME_STEP_SECONDS = 900
OUTPUT_NC = Path("opendrift_output.nc")
OUTPUT_JSON = Path("opendrift_final_positions.json")


# ---------------------------------------------------------------------------
# Step 1: Convert ECMWF GRIB2 to CF-compliant NetCDF
# ---------------------------------------------------------------------------
def prepare_ecmwf_netcdf():
    if ECMWF_NC.exists():
        print(f"  ✅ ECMWF NetCDF already exists: {ECMWF_NC}")
        return

    print("  Converting ECMWF GRIB2 → CF NetCDF...")
    ds = xr.open_dataset(ECMWF_GRIB, engine="cfgrib")

    # Drop the scalar model-run 'time' before renaming
    ds = ds.drop_vars("time", errors="ignore")

    # Promote valid_time to the time dimension
    ds = ds.swap_dims({"step": "valid_time"}).rename({"valid_time": "time"})

    # Drop remaining GRIB-specific coordinates
    ds = ds.drop_vars(
        ["step", "heightAboveGround", "surface"], errors="ignore"
    )

    # Let OpenDrift identify the wind variables
    ds = add_standard_name_for_surface_grib_variables(ds)

    # Drop skin temperature for the advection-only comparison
    ds = ds.drop_vars("skt", errors="ignore")

    ds.to_netcdf(ECMWF_NC)
    print(f"  ✅ Wrote {ECMWF_NC}")


# ---------------------------------------------------------------------------
# Step 2: Build readers
# ---------------------------------------------------------------------------
def build_readers():
    reader_smoc = reader_netCDF_CF_generic.Reader(
        str(SMOC_PATH),
        standard_name_mapping={
            "utotal": "x_sea_water_velocity",
            "vtotal": "y_sea_water_velocity",
        },
    )

    reader_wind = reader_netCDF_CF_generic.Reader(str(ECMWF_NC))

    print("\n=== SMOC reader ===")
    print(reader_smoc)
    print("\n=== Wind reader ===")
    print(reader_wind)

    return reader_smoc, reader_wind


# ---------------------------------------------------------------------------
# Step 3: Run OpenDrift
# ---------------------------------------------------------------------------
def run_opendrift(reader_smoc, reader_wind):
    o = OceanDrift(loglevel=20)
    o.add_reader([reader_smoc, reader_wind])

    o.seed_elements(
        lon=RELEASE_LON,
        lat=RELEASE_LAT,
        radius=0,
        number=N_PARTICLES,
        time=START_TIME,
    )

    print("\nRunning OpenDrift...")
    o.run(
        duration=timedelta(hours=DURATION_HOURS),
        time_step=TIME_STEP_SECONDS,
        time_step_output=3600,
        outfile=str(OUTPUT_NC),
    )

    return o


# ---------------------------------------------------------------------------
# Step 4: Extract final positions and save
# ---------------------------------------------------------------------------
def save_final_positions(o):
    lon = o.elements.lon
    lat = o.elements.lat
    status = o.elements.status

    print(f"\nFinal state: {len(lon)} particles")
    print(f"  Active:      {np.sum(status == 0)}")
    print(f"  Stranded:    {np.sum(status == 2)}")
    print(f"  Deactivated: {np.sum(status == 1)}")

    out = {
        "release": [RELEASE_LON, RELEASE_LAT],
        "start_time": START_TIME.isoformat(),
        "duration_hours": DURATION_HOURS,
        "n_particles": int(len(lon)),
        "final_positions": [
            {
                "lon": float(lon[i]),
                "lat": float(lat[i]),
                "status": int(status[i]),
            }
            for i in range(len(lon))
            if not np.isnan(lon[i])
        ],
    }

    with open(OUTPUT_JSON, "w") as f:
        json.dump(out, f)

    print(f"  ✅ Wrote {OUTPUT_JSON}")
    print(f"\nCentroid: lon={np.nanmean(lon):.4f}, lat={np.nanmean(lat):.4f}")
    print(f"Spread (std): lon={np.nanstd(lon):.4f}, lat={np.nanstd(lat):.4f}")

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    print("Step 1: Preparing ECMWF NetCDF")
    prepare_ecmwf_netcdf()

    print("\nStep 2: Building readers")
    reader_smoc, reader_wind = build_readers()

    print("\nStep 3: Running OpenDrift")
    o = run_opendrift(reader_smoc, reader_wind)

    print("\nStep 4: Saving final positions")
    save_final_positions(o)

    print("\n✅ Done.")


if __name__ == "__main__":
    main()