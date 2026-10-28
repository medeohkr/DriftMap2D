#!/usr/bin/env python3
"""reproduce_worst.py recreates run_039 and exports maps and a website config."""

import argparse
import csv
import json
from datetime import datetime, timedelta
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt
import numpy as np
from opendrift.models.oceandrift import OceanDrift
from opendrift.readers import reader_netCDF_CF_generic

HERE = Path(__file__).resolve().parent
LON, LAT = 3.0, 59.142857142857146
START = datetime(2026, 10, 27)
END = datetime(2026, 10, 29)


def website_config():
    """export the existing website's config-only scenario format."""
    config = {
        "tracerType": "generic", "forcingModel": "norkyst",
        "particleCount": 1, "timeStepMin": 60,
        "advectionScheme": "rk4", "diffusionScheme": "constant",
        "diffusionCoeffs": [0, 0],
        "startDate": "2026-10-27", "startTime": "00:00",
        "endDate": "2026-10-29", "endTime": "00:00", "totalDays": 2,
        "releases": [{"lon": LON, "lat": LAT, "radius": 0,
                      "schedule": [{"amount": 1, "duration": 0}]}],
        "generic": {"windFactor": 0, "windDeflection": 0},
        "oil": {"id": "GN00006", "query": "Generic Light Crude", "windFactor": 3,
                "deflectionScheme": "constant", "windDeflection": 0,
                "emulOnset": "oil", "bullwinkleFrac": 0, "bulltime": 0},
        "sar": {"id": "Person in water, unknown state (mean values)",
                "jibeProbability": 4, "capsizing": False,
                "capsizeThreshold": 30, "capsizeFraction": 40, "capsizeSigma": 5},
        "visualization": {"particleRadius": 5, "gridUpdateInterval": 100,
                          "gridSize": 0.02, "smoothLevel": 2, "snapshotInterval": 1,
                          "visualizationMode": "particles", "autoZoom": True},
    }
    return {"type": "FeatureCollection", "features": [],
            "properties": {"model": "DriftMap2D", "version": "1.0",
                           "date": START.isoformat() + "Z", "includesSnapshots": False,
                           "config": config}}


def main():
    """rerun the worst absolute-error case without changing its physics."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=str(HERE / "runs/source_surface.nc"),
                        help="cached suite snapshot, local NetCDF or NorKyst OPeNDAP URL")
    parser.add_argument("--output", type=Path, default=HERE / "runs/worst_case")
    parser.add_argument("--skip-animation", action="store_true")
    args = parser.parse_args()
    if not args.source.startswith(("http://", "https://")) and not Path(args.source).exists():
        parser.error("source snapshot is missing, run the suite or pass --source")
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "website_config.geojson").write_text(json.dumps(website_config(), indent=2))
    reader = reader_netCDF_CF_generic.Reader(args.source, name="NorKyst worst case")
    model = OceanDrift(loglevel=30)
    model.add_reader(reader)
    for key, value in {
        "drift:advection_scheme": "runge-kutta4",
        "environment:constant:horizontal_diffusivity": 0,
        "environment:constant:x_wind": 0,
        "environment:constant:y_wind": 0,
        "drift:current_uncertainty": 0, "drift:wind_uncertainty": 0,
        "drift:vertical_mixing": False, "drift:vertical_advection": False,
        "drift:stokes_drift": False,
    }.items():
        model.set_config(key, value)
    model.seed_elements(lon=LON, lat=LAT, radius=0, number=1,
                        time=START, z=0, wind_drift_factor=0)
    output_nc = args.output / "opendrift_output.nc"
    output_nc.unlink(missing_ok=True)
    model.run(duration=END - START, time_step=3600, time_step_output=3600,
              outfile=str(output_nc))
    lon = np.asarray(model.result.lon).reshape(-1)
    lat = np.asarray(model.result.lat).reshape(-1)
    if len(lon) != 49 or not np.isfinite([lon, lat]).all():
        raise RuntimeError("OpenDrift did not produce all 49 finite hourly snapshots")
    rows = [{"step": i, "time": (START + timedelta(hours=i)).isoformat() + "Z",
             "lon": float(x), "lat": float(y)} for i, (x, y) in enumerate(zip(lon, lat))]
    (args.output / "opendrift_centroids.json").write_text(json.dumps(rows, indent=2))
    with (args.output / "opendrift_hourly.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=["step", "time", "lon", "lat"])
        writer.writeheader()
        writer.writerows(rows)
    # preserve the original CLI track rather than fetching a newer forecast
    reference = HERE / "runs/run_039/proteus_centroids.json"
    if reference.exists():
        cli = json.loads(reference.read_text())
        (args.output / "proteus_reference.json").write_text(json.dumps(cli, indent=2))
        fig, ax = plt.subplots(figsize=(9, 8))
        ax.plot(lon, lat, color="#37579b", label="OpenDrift")
        ax.plot([r["lon"] for r in cli], [r["lat"] for r in cli], color="C1", label="Proteus CLI")
        ax.scatter([LON], [LAT], c="black", marker="*", s=100, label="release")
        for i in range(0, 49, 6):
            ax.scatter(lon[i], lat[i], color="#37579b", s=20)
            ax.annotate(f"{i}h", (lon[i], lat[i]), xytext=(5, 5), textcoords="offset points")
            ax.scatter(cli[i]["lon"], cli[i]["lat"], color="C1", s=20)
            ax.annotate(f"{i}h", (cli[i]["lon"], cli[i]["lat"]),
                        xytext=(5, -12), textcoords="offset points")
        ax.set_aspect(1 / np.cos(np.radians(LAT)))
        ax.set(xlabel="longitude (degrees east)", ylabel="latitude (degrees north)",
               title="run_039, 27–29 october 2026 UTC\n1 particle, hourly RK4, no windage or diffusion")
        ax.grid(alpha=0.3)
        ax.legend()
        fig.tight_layout()
        fig.savefig(args.output / "comparison.png", dpi=180)
        plt.close(fig)
    print("saving OpenDrift map", flush=True)
    model.plot(filename=str(args.output / "opendrift_map.png"), show=False,
               fast=True, linewidth=2, markersize=25)
    plt.close("all")
    if not args.skip_animation:
        print("saving OpenDrift animation", flush=True)
        model.animation(filename=str(args.output / "opendrift_animation.gif"),
                        fast=True, show_trajectories=True, trajectory_alpha=1,
                        linewidth=2, markersize=30, fps=6, frames=49)
        plt.close("all")
    print(f"release {LAT:.12f} N, {LON:.12f} E", flush=True)
    print(f"OpenDrift endpoint {lat[-1]:.12f} N, {lon[-1]:.12f} E", flush=True)
    print(f"outputs in {args.output}", flush=True)


if __name__ == "__main__":
    main()
