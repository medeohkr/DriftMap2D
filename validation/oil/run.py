#!/usr/bin/env python3
"""Run matched OpenOil/Proteus evaporation and emulsification grid cases."""
import argparse
import json
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

import numpy as np
import xarray as xr

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from shared.proteus_utils import run_cli

START_TIME = datetime(2026, 9, 19)
DURATION_HOURS = 48
TIME_STEP_SECONDS = 3600
N_PARTICLES = 1
TOTAL_MASS_KG = 1000.0
WIND_FACTOR = 0.0
WIND_DEFLECTION = 0.0
FORCING = HERE.parent / "forcing"


def prepare_wind():
    """Keep skin temperature: it is also the SST stored in validation tiles."""
    from opendrift.readers import add_standard_name_for_surface_grib_variables

    path = FORCING / "wind/ecmwf_2026-09-19_00z_oil.nc"
    if not path.exists():
        with xr.open_dataset(FORCING / "wind/ecmwf_2026-09-19_00z.grib2",
                             engine="cfgrib") as source:
            ds = source.drop_vars("time", errors="ignore")
            ds = ds.swap_dims({"step": "valid_time"}).rename({"valid_time": "time"})
            ds = ds.drop_vars(["step", "heightAboveGround", "surface"], errors="ignore")
            ds = add_standard_name_for_surface_grib_variables(ds)
            ds["skt"].attrs.update(standard_name="sea_water_temperature", units="K")
            ds.to_netcdf(path)
    return path


def make_model(oil_name, oil_source=None):
    from opendrift.models.openoil import OpenOil

    model = OpenOil(loglevel=30, weathering_model="noaa")
    settings = {
        "seed:oil_type": oil_name,
        "drift:advection_scheme": "euler",
        "environment:constant:horizontal_diffusivity": 0.0,
        "drift:current_uncertainty": 0.0,
        "drift:wind_uncertainty": 0.0,
        "drift:wind_drift_depth": 0.0,
        "drift:stokes_drift": False,
        "drift:vertical_mixing": False,
        "drift:vertical_advection": False,
        "processes:evaporation": True,
        "processes:emulsification": True,
        "processes:dispersion": False,
        "processes:biodegradation": False,
        "processes:update_oilfilm_thickness": False,
    }
    for key, value in settings.items():
        model.set_config(key, value)
    # Oil wind drift has no rotation in OpenOil; zero factor also makes its
    # direction immaterial. Keep the explicit Proteus rotation at zero below.
    if oil_source is None:
        model.set_oiltype(oil_name)
    else:
        model.set_oiltype_from_file(str(oil_source))
    return model, settings


def catalog_oil(oil, catalog_dir):
    """require the same oil ID in OpenDrift and the local production catalog."""
    path = catalog_dir / "oil_catalog.json"
    if not path.is_file():
        raise ValueError(f"Missing {path}. Run python scripts/build_catalogs.py first")
    catalog = json.loads(path.read_text(encoding="utf-8"))
    matches = [record for record in catalog["oils"] if record["oil_id"] == oil.id]
    if len(matches) != 1:
        raise ValueError(f"OpenDrift oil {oil.id} must match exactly one catalog entry")
    return matches[0]


def reference_oil_source(record, explicit=None):
    """use the raw source record so catalog and OpenDrift oil editions match."""
    if explicit is not None:
        source = explicit.resolve()
    else:
        name = record["source_file"]
        if name.startswith("opendrift:"):
            return None
        source = Path(name)
        if not source.is_absolute():
            source = HERE.parents[1] / source
    if not source.is_file():
        raise ValueError(f"Oil source record not found at {source}. Set --reference-oil-file")
    return source


def make_cli_config(lon, lat, oil_id, tile_url, catalog_dir):
    return {
        "start_date": START_TIME.strftime("%Y-%m-%d %H:%M"),
        "tracer_type": "oil", "forcing_model": "global",
        "catalog_dir": str(catalog_dir),
        "tracer_json": {"oil_id": oil_id,
                        "overrides": {"wind_factor": WIND_FACTOR,
                                      "wind_deflection": WIND_DEFLECTION}},
        "releases": [{"lon": lon, "lat": lat, "radius": 0.0,
                      "schedule": [{"amount": TOTAL_MASS_KG, "duration": 0}]}],
        "particles": N_PARTICLES, "time_step_minutes": TIME_STEP_SECONDS / 60,
        "advection": "euler", "diffusion": "constant", "diffusion_coeffs": [0.0, 0.0],
        "steps": DURATION_HOURS * 3600 // TIME_STEP_SECONDS,
        "tile_url": tile_url, "output": "proteus_centroids.json",
    }


def save_budget(nc_path, output):
    """Match CLI initial-mass weighting, including water fraction (not oil loss)."""
    rows = []
    with xr.open_dataset(nc_path) as ds:
        initial = ds.mass_oil.isel(time=0)
        for step in range(ds.sizes["time"]):
            sample = ds.isel(time=step)
            active = (sample.status == 0) & np.isfinite(sample.mass_oil)
            weights = initial.where(active)
            denominator = float(weights.sum())
            if denominator <= 0:
                raise ValueError(f"No active oil at step {step}: {nc_path}")
            rows.append({
                "step": step,
                # OpenOil's fraction_evaporated is a start-of-step diagnostic;
                # its mass budget is current at the output timestamp.
                "evaporation_percent": float(sample.mass_evaporated.where(active).sum()) / denominator * 100,
                "emulsification_percent": float((sample.water_fraction * weights).sum()) / denominator * 100,
            })
    output.write_text(json.dumps(rows, indent=2), encoding="utf-8")


def validate_cli_output(run_dir, config):
    """Reject old CLI binaries that export positions without oil weathering."""
    path = run_dir / config["output"]
    rows = json.loads(path.read_text(encoding="utf-8"))
    if len(rows) != config["steps"] + 1:
        raise ValueError(f"Incomplete CLI output: {path}")
    for step, row in enumerate(rows):
        for key in ("evaporation_percent", "emulsification_percent"):
            if key not in row:
                raise ValueError(
                    f"CLI output is missing {key}; rebuild with "
                    "`cargo build --release -p proteus-cli` before running oil validation")
            if row[key] is None or not np.isfinite(row[key]):
                raise ValueError(f"Invalid {key} at CLI step {step}")
        if row["step"] != step:
            raise ValueError(f"Misaligned CLI step {step}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oil-type", default="Generic Medium Crude")
    parser.add_argument("--grid-n", type=int, default=8)
    parser.add_argument("--tile-url", default="validation-tiles")
    parser.add_argument("--catalog-dir", type=Path, default=HERE.parents[1] / "catalogs")
    parser.add_argument("--runs-dir", type=Path, default=HERE / "runs")
    parser.add_argument("--reference-oil-file", type=Path,
                        help="raw NOAA JSON for OpenDrift, defaults to catalog source_file")
    parser.add_argument("--check-config", action="store_true",
                        help="validate OpenOil settings and local catalog selection without running simulations")
    args = parser.parse_args()
    if args.grid_n < 1:
        parser.error("--grid-n must be positive")
    # Validate shared setup before loading forcing or entering the grid loop.
    # Invalid settings should fail once, rather than repeat for every case.
    catalog_dir = args.catalog_dir.resolve()
    first_model, settings = make_model(args.oil_type)
    record = catalog_oil(first_model.oiltype, catalog_dir)
    oil_id = record["oil_id"]
    oil_source = reference_oil_source(record, args.reference_oil_file)
    if oil_source is not None:
        first_model, settings = make_model(args.oil_type, oil_source)
        if first_model.oiltype.id != oil_id:
            raise ValueError(f"Reference source oil ID must be {oil_id}")
    if args.check_config:
        print(json.dumps({"opendrift_config": settings, "reference_oil_file": str(oil_source) if oil_source else None,
                          "proteus_config": make_cli_config(0, 0, oil_id, args.tile_url, catalog_dir)}, indent=2))
        return
    from opendrift.readers import reader_netCDF_CF_generic

    currents = reader_netCDF_CF_generic.Reader(
        str(FORCING / "currents/smoc_20260919.nc"),
        standard_name_mapping={"utotal": "x_sea_water_velocity", "vtotal": "y_sea_water_velocity"})
    wind = reader_netCDF_CF_generic.Reader(str(prepare_wind()))
    failures = 0
    for index, (lat, lon) in enumerate((lat, lon)
            for lat in np.linspace(-7.5, 7.5, args.grid_n)
            for lon in np.linspace(-177.5, -147.5, args.grid_n)):
        run_dir = args.runs_dir.resolve() / f"run_{index:03d}"
        run_dir.mkdir(parents=True, exist_ok=True)
        meta = {"run_index": index, "release_lon": lon, "release_lat": lat,
                "start_time": START_TIME.isoformat(), "duration_hours": DURATION_HOURS,
                "time_step_seconds": TIME_STEP_SECONDS, "n_particles": N_PARTICLES,
                "total_mass_kg": TOTAL_MASS_KG, "oil_type": args.oil_type,
                "oil_id": oil_id, "catalog_dir": str(catalog_dir),
                "reference_oil_file": str(oil_source) if oil_source else None,
                "wind_factor": WIND_FACTOR, "wind_deflection": WIND_DEFLECTION}
        print(f"[{index + 1}/{args.grid_n**2}] ({lon:.2f}, {lat:.2f})", flush=True)
        try:
            model = first_model if index == 0 else make_model(args.oil_type, oil_source)[0]
            meta["opendrift_config"] = settings
            config = make_cli_config(lon, lat, oil_id, args.tile_url, catalog_dir)
            model.add_reader([currents, wind])
            model.seed_elements(lon=lon, lat=lat, radius=0, number=N_PARTICLES,
                                time=START_TIME, z=0, wind_drift_factor=WIND_FACTOR,
                                oil_film_thickness=0.001, mass_oil=TOTAL_MASS_KG / N_PARTICLES)
            started = time.perf_counter()
            model.run(duration=timedelta(hours=DURATION_HOURS), time_step=TIME_STEP_SECONDS,
                      time_step_output=TIME_STEP_SECONDS, outfile=str(run_dir / "opendrift_output.nc"))
            meta["opendrift_seconds"] = time.perf_counter() - started
            save_budget(run_dir / "opendrift_output.nc", run_dir / "opendrift_budget.json")
            elapsed, result = run_cli(run_dir, config)
            meta["cli_seconds"] = elapsed
            if result.returncode:
                raise RuntimeError(f"Proteus exit {result.returncode}: {result.stderr[-2000:]}")
            validate_cli_output(run_dir, config)
            meta["success"] = True
        except Exception as exc:
            meta.update(success=False, error=str(exc))
            failures += 1
            print(f"  FAILED: {exc}", flush=True)
        (run_dir / "meta.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")
    if failures:
        raise SystemExit(f"{failures} cases failed; see runs/*/meta.json")


if __name__ == "__main__":
    main()
