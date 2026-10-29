#!/usr/bin/env python3
"""run.py compares deterministic coastal stranding with a 24 hour release."""

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import timedelta
from pathlib import Path

import numpy as np
import xarray as xr
from opendrift.models.oceandrift import OceanDrift
from scipy.ndimage import uniform_filter

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))

from norkyst_generic.run import START, END, SOURCE, TILE_URL, make_reader
from shared.proteus_utils import run_cli

RUNS = HERE / "runs"
DT_MINUTES = 15
STEPS = 192


def mean_dilation(data):
    """use OpenDrift commit e313d779 mean dilation without changing the installation."""
    valid = np.isfinite(data)
    numerator = uniform_filter(np.where(valid, data, 0).astype(np.float64),
                               size=3, mode="constant", cval=0)
    denominator = uniform_filter(valid.astype(np.float64), size=3, mode="constant", cval=0)
    fillable = ~valid & (denominator > 1e-12)
    data[fillable] = (numerator[fillable] / denominator[fillable]).astype(data.dtype)


def make_config(lon, lat, particles, tile_url, landmask_url):
    """disable random displacement and emit a constant amount over 24 hours."""
    config = {
        "start_date": START.strftime("%Y-%m-%d %H:%M"),
        "forcing_model": "norkyst", "tile_url": tile_url,
        "tracer_type": "generic",
        "tracer_json": {"wind_factor": 0.02, "wind_deflection": 0.0},
        "releases": [{"lon": lon, "lat": lat, "radius": 0.0,
                      "schedule": [{"amount": 1.0, "duration": 24.0}]}],
        "particles": particles, "time_step_minutes": DT_MINUTES,
        "advection": "euler", "diffusion": "constant", "diffusion_coeffs": [0, 0],
        "steps": STEPS, "output": "proteus_centroids.json",
        "particle_output": "particles.jsonl", "snapshot_every": 1,
    }
    if landmask_url:
        config["landmask_url"] = landmask_url
    return config


def read_cli(directory, particles):
    """record exact birth batches and retain hourly snapshots for plotting."""
    births = np.full(particles, -1, dtype=int)
    counts = []
    with (directory / "particles.jsonl").open() as source, \
            (directory / "particles_hourly.jsonl").open("w") as hourly:
        for line in source:
            snapshot = json.loads(line)
            step = snapshot["step"]
            for particle in snapshot["particles"]:
                identity = particle["id"]
                if births[identity] < 0:
                    # step n seeds immediately before advancing from n-1 to n
                    births[identity] = max(step - 1, 0)
            if step % 4 == 0:
                hourly.write(line)
                counts.append({"hour": step // 4, "released": len(snapshot["particles"]),
                               "proteus_stranded": sum(p["stranded"] for p in snapshot["particles"])})
    if (births < 0).any() or len(counts) != 49:
        raise ValueError("incomplete particle release or snapshot sequence")
    return births, counts


def run_opendrift(directory, config, reader, births, output_seconds=3600):
    """mirror CLI birth times so release discretization cannot change stranding counts."""
    from opendrift.readers.interpolation import interpolators

    interpolators.expand_numpy_array = mean_dilation
    model = OceanDrift(loglevel=30)
    model.add_reader(reader)
    for key, value in {
        "drift:advection_scheme": "euler",
        "drift:current_uncertainty": 0.0,
        "drift:current_uncertainty_uniform": 0.0,
        "drift:wind_uncertainty": 0.0,
        "environment:constant:horizontal_diffusivity": 0.0,
        "drift:vertical_mixing": False, "drift:vertical_advection": False,
        "drift:stokes_drift": False, "general:coastline_action": "stranding",
    }.items():
        model.set_config(key, value)
    release = config["releases"][0]
    for step, number in zip(*np.unique(births, return_counts=True)):
        model.seed_elements(lon=release["lon"], lat=release["lat"], radius=0,
                            number=int(number), z=0, wind_drift_factor=0.02,
                            time=START + timedelta(minutes=int(step) * DT_MINUTES))
    target = directory / "opendrift_output.nc"
    target.unlink(missing_ok=True)
    model.run(duration=END - START, time_step=DT_MINUTES * 60, time_step_output=output_seconds,
              export_variables=["lon", "lat", "status"], outfile=str(target))
    with xr.open_dataset(target) as dataset:
        status = dataset.status.transpose("time", "trajectory").values
        stranded = np.maximum.accumulate(status == 1, axis=0).sum(axis=1)
        if np.any(np.isfinite(status) & (status > 1)):
            raise ValueError("OpenDrift deactivated particles for reasons other than stranding")
    return stranded


def main():
    """try nearby water releases until both models show partial stranding."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--particles", type=int, default=9600)
    parser.add_argument("--lon", type=float, default=5.1)
    parser.add_argument("--lat", type=float, default=59.0)
    parser.add_argument("--source", default=SOURCE)
    parser.add_argument("--tile-url", default=TILE_URL)
    parser.add_argument("--landmask-url")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if args.particles < 96 or args.particles % 96:
        parser.error("particles must be a positive multiple of 96")
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "proteus-cli"], cwd=ROOT, check=True)
    RUNS.mkdir(exist_ok=True)
    from roaring_landmask import RoaringLandmask
    mask = RoaringLandmask.new()
    for attempt, offset in enumerate([0, -0.05, 0.05, -0.1, 0.1, -0.2, 0.2]):
        lon, lat = args.lon + offset, args.lat
        if mask.contains(lon, lat):
            continue
        directory = RUNS / f"run_{attempt:03d}"
        directory.mkdir(exist_ok=True)
        config = make_config(lon, lat, args.particles, args.tile_url, args.landmask_url)
        print(f"running {lon:.4f} E {lat:.4f} N", flush=True)
        _, result = run_cli(directory, config)
        (directory / "proteus.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        if result.returncode:
            raise RuntimeError(f"CLI failed, see {directory / 'proteus.log'}")
        births, counts = read_cli(directory, args.particles)
        first_hash = hashlib.sha256((directory / "particles.jsonl").read_bytes()).hexdigest()
        _, replay = run_cli(directory, config)
        if replay.returncode or hashlib.sha256((directory / "particles.jsonl").read_bytes()).hexdigest() != first_hash:
            raise ValueError("CLI replay was not deterministic")
        # reuse the existing coastal source snapshot rather than fetching a new revision
        cache = ROOT / "validation/norkyst_leeway/runs"
        reader = make_reader(args.source, [(lon, lat)], include_wind=True, cache_dir=cache)
        od_counts = run_opendrift(directory, config, reader, births)
        if len(od_counts) != len(counts):
            raise ValueError("OpenDrift output does not cover all 49 hours")
        for row, stranded in zip(counts, od_counts):
            row["opendrift_stranded"] = int(stranded)
        import csv
        with (directory / "stranding_counts.csv").open("w", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=list(counts[0]))
            writer.writeheader()
            writer.writerows(counts)
        summary = {"config": config, "source": args.source, "cli_replay_identical": True,
                   "cli_particle_sha256": first_hash, "opendrift_dilation": "mean",
                   "max_hourly_stranded_count_difference": max(
                       abs(row["proteus_stranded"] - row["opendrift_stranded"]) for row in counts),
                   "release_batches": [{"step": int(step), "count": int(number)}
                                       for step, number in zip(*np.unique(births, return_counts=True))],
                   "final": counts[-1]}
        (directory / "summary.json").write_text(json.dumps(summary, indent=2))
        print(counts[-1], flush=True)
        if all(0 < counts[-1][key] < args.particles
               for key in ("proteus_stranded", "opendrift_stranded")):
            from landmask_stranding.visualize import visualize
            visualize(directory)
            return
        print("trying a nearby release to obtain partial stranding", flush=True)
    raise RuntimeError("no partially stranded case found, supply another --lon and --lat")


if __name__ == "__main__":
    main()
