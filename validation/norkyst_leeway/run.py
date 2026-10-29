#!/usr/bin/env python3
"""run.py compares NorKyst leeway drift using the CLI's local object catalog."""

import argparse
import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

import numpy as np
from opendrift.models.leeway import Leeway

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))

from norkyst_generic.run import START, END, STEPS, SOURCE, TILE_URL, make_reader, check_trajectory
from shared.opendrift_utils import save_centroids_from_nc
from shared.proteus_utils import CLI_BIN, run_cli

RUNS = HERE / "runs"


def make_grid(size):
    """place water releases west of Stavanger and Karmøy for stranding comparisons."""
    from roaring_landmask import RoaringLandmask

    mask = RoaringLandmask.new()
    return [(float(lon), float(lat))
            for lat in np.linspace(59.0, 59.2, size)
            for lon in np.linspace(5.1, 5.3, size)
            if not mask.contains(float(lon), float(lat))]


def make_config(lon, lat, particles, object_id, catalog_dir, tile_url):
    """resolve coefficients by object ID instead of embedding tracer properties."""
    return {
        "start_date": START.strftime("%Y-%m-%d %H:%M"),
        "forcing_model": "norkyst", "tile_url": tile_url,
        "tracer_type": "sar", "catalog_dir": str(catalog_dir),
        "tracer_json": {"object_id": object_id, "overrides": {
            "jibe_probability": 0.0, "random_orientation": False, "capsizing": False}},
        "releases": [{"lon": lon, "lat": lat, "radius": 0.0,
                      "schedule": [{"amount": 1.0, "duration": 0}]}],
        "particles": particles, "time_step_minutes": 60,
        "advection": "euler", "diffusion": "constant", "diffusion_coeffs": [0, 0],
        "steps": STEPS, "output": "proteus_centroids.json",
    }


def validate_object(catalog_dir, object_id):
    """check that the selected local record matches OpenDrift's object definition."""
    path = catalog_dir / "object_catalog.json"
    if not path.exists():
        raise FileNotFoundError(f"missing {path}, run python scripts/build_catalogs.py")
    catalog = json.loads(path.read_text(encoding="utf-8"))
    if object_id not in catalog["objects"]:
        raise ValueError(f"unknown local object ID {object_id}")
    record = catalog["objects"][object_id]
    model = Leeway(loglevel=30)
    matches = [(index, prop) for index, prop in model.leewayprop.items()
               if prop["OBJKEY"] == object_id]
    if len(matches) != 1:
        raise ValueError(f"OpenDrift has no unique object {object_id}")
    index, prop = matches[0]
    for field, keys in {
        "downwind": ("DWSLOPE", "DWOFFSET", "DWSTD"),
        "right": ("CWRSLOPE", "CWROFFSET", "CWRSTD"),
        "left": ("CWLSLOPE", "CWLOFFSET", "CWLSTD"),
    }.items():
        if not np.allclose(record[field], [prop[key] for key in keys], rtol=0, atol=1e-6):
            raise ValueError(f"local {object_id} {field} differs from OpenDrift")
    return index, record


def run_opendrift(directory, lon, lat, reader, particles, object_type, seed):
    """retain coefficient uncertainty while disabling jibing and capsizing."""
    np.random.seed(seed)
    model = Leeway(loglevel=30)
    model.add_reader(reader)
    for key, value in {
        "drift:advection_scheme": "euler", "drift:current_uncertainty": 0,
        "drift:current_uncertainty_uniform": 0, "drift:wind_uncertainty": 0,
        "drift:stokes_drift": False, "processes:capsizing": False,
    }.items():
        model.set_config(key, value)
    model.seed_elements(lon=lon, lat=lat, radius=0, number=particles,
                        time=START, object_type=object_type, jibe_probability=0)
    model.run(duration=END - START, time_step=3600, time_step_output=3600,
              export_variables=["lon", "lat", "status"],
              outfile=str(directory / "opendrift_output.nc"))


def main():
    """build the current CLI and run paired offshore ensembles."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=SOURCE)
    parser.add_argument("--tile-url", default=TILE_URL)
    parser.add_argument("--catalog-dir", type=Path, default=ROOT / "catalogs")
    parser.add_argument("--object-id", default="PIW-1")
    parser.add_argument("--particles", type=int, default=100_000)
    parser.add_argument("--grid-size", type=int, default=8)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.grid_size < 2 or args.particles < 2 or args.particles % 2 or (args.limit is not None and args.limit < 1):
        parser.error("grid size must be at least 2, particles must be positive and even, limit must be positive")
    catalog_dir = args.catalog_dir.resolve()
    object_type, record = validate_object(catalog_dir, args.object_id)
    points = make_grid(args.grid_size)[:args.limit]
    RUNS.mkdir(exist_ok=True)
    if args.dry_run:
        for i, (lon, lat) in enumerate(points):
            directory = RUNS / f"run_{i:03d}"
            directory.mkdir(exist_ok=True)
            (directory / "config.json").write_text(json.dumps(
                make_config(lon, lat, args.particles, args.object_id, catalog_dir, args.tile_url), indent=2))
        print(f"validated {args.object_id} and wrote {len(points)} catalog-based configs")
        return
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "proteus-cli"], cwd=ROOT, check=True)
    if not CLI_BIN.exists():
        raise FileNotFoundError(f"build the CLI first at {CLI_BIN}")
    request = urllib.request.Request(args.tile_url.rstrip("/") + "/manifest.json",
                                     headers={"User-Agent": "Proteus-validation/1.0"})
    with urllib.request.urlopen(request, timeout=60) as response:
        manifest = json.load(response)
    (RUNS / "manifest.json").write_text(json.dumps(manifest, indent=2))
    (RUNS / "object_reference.json").write_text(json.dumps(
        {"object_id": args.object_id, "catalog_dir": str(catalog_dir), "record": record}, indent=2))
    reader = make_reader(args.source, make_grid(args.grid_size), include_wind=True, cache_dir=RUNS)
    failed = 0
    for i, (lon, lat) in enumerate(points):
        directory = RUNS / f"run_{i:03d}"
        directory.mkdir(exist_ok=True)
        for name in ("opendrift_centroids.json", "proteus_centroids.json", "opendrift_output.nc"):
            (directory / name).unlink(missing_ok=True)
        meta = {"run_index": i, "release_lon": lon, "release_lat": lat,
                "start_time": START.isoformat(), "end_time": END.isoformat(),
                "n_particles": args.particles, "object_id": args.object_id,
                "catalog_dir": str(catalog_dir), "source": args.source, "tile_url": args.tile_url,
                "opendrift_seed": 20261028 + i, "time_step_seconds": 3600,
                "jibe_probability": 0, "capsizing": False, "status": "failed"}
        print(f"[{i + 1}/{len(points)}] lon={lon:.4f} lat={lat:.4f}, {args.particles} particles", flush=True)
        try:
            seconds, result = run_cli(directory, make_config(
                lon, lat, args.particles, args.object_id, catalog_dir, args.tile_url))
            (directory / "proteus.log").write_text(result.stdout + result.stderr, encoding="utf-8")
            if result.returncode:
                raise RuntimeError(f"CLI failed with exit {result.returncode}, see proteus.log")
            check_trajectory(directory / "proteus_centroids.json")
            meta["cli_seconds"] = seconds
            started = time.perf_counter()
            run_opendrift(directory, lon, lat, reader, args.particles, object_type, meta["opendrift_seed"])
            save_centroids_from_nc(directory / "opendrift_output.nc", directory / "opendrift_centroids.json")
            check_trajectory(directory / "opendrift_centroids.json")
            meta["opendrift_seconds"] = time.perf_counter() - started
            meta["status"] = "complete"
            print(f"  complete, CLI {seconds:.1f}s, OpenDrift {meta['opendrift_seconds']:.1f}s", flush=True)
        except Exception as error:
            failed += 1
            meta["error"] = str(error)
            (directory / "opendrift_centroids.json").unlink(missing_ok=True)
            print(f"  failed, {error}", flush=True)
        (directory / "meta.json").write_text(json.dumps(meta, indent=2))
    if failed:
        raise SystemExit(f"{failed} of {len(points)} comparisons failed")


if __name__ == "__main__":
    main()
