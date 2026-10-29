#!/usr/bin/env python3
"""run.py compares native NorKyst trajectories from Proteus and OpenDrift."""

import argparse
import json
import subprocess
import sys
import time
import urllib.request
from datetime import datetime
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))

from shared.opendrift_utils import save_centroids_from_nc
from shared.proteus_utils import CLI_BIN, run_cli

SOURCE = "https://thredds.met.no/thredds/dodsC/fou-hi/norkystv3_800m_m00_be"
TILE_URL = "https://tiles.driftmap2d.com/norkyst"
START = datetime(2026, 10, 28)
END = datetime(2026, 10, 30)
STEPS = 48
RUNS = HERE / "runs"


def make_grid(size):
    """sample the open North Sea well west of the Norwegian coast."""
    return [(float(lon), float(lat))
            for lat in np.linspace(58.0, 60.0, size)
            for lon in np.linspace(2.0, 3.0, size)]


def make_config(lon, lat, tile_url):
    """use the regional CLI backend with deterministic surface advection."""
    return {
        "start_date": START.strftime("%Y-%m-%d %H:%M"),
        "forcing_model": "norkyst",
        "tile_url": tile_url,
        "tracer_type": "generic",
        "tracer_json": {"wind_factor": 0.0, "wind_deflection": 0.0},
        "releases": [{"lon": lon, "lat": lat, "radius": 0.0,
                      "schedule": [{"amount": 1.0, "duration": 0}]}],
        "particles": 1,
        "time_step_minutes": 60.0,
        "advection": "rk4",
        "diffusion": "constant",
        "diffusion_coeffs": [0.0, 0.0],
        "steps": STEPS,
        "output": "proteus_centroids.json",
    }


def make_reader(source, points=None, *, include_wind=False, cache_dir=None):
    """use OpenDrift's projected CF reader used by its NorKyst examples."""
    import xarray as xr
    from opendrift.readers import reader_netCDF_CF_generic
    from pyproj import CRS, Transformer

    def projected_points(data):
        projection = data.projection_stere.attrs
        crs = CRS.from_user_input(projection.get("proj4") or CRS.from_cf(projection))
        return Transformer.from_crs("EPSG:4326", crs, always_xy=True).transform(*zip(*points))

    cache_root = Path(cache_dir) if cache_dir is not None else RUNS
    suffix = "_wind" if include_wind else ""
    snapshot = cache_root / f"source_surface_{START:%Y%m%d}{suffix}.nc"
    if points is not None and snapshot.exists():
        cached = xr.open_dataset(snapshot)
        x, y = projected_points(cached)
        covers_region = all(
            min(values) - 100_000 >= float(cached[name].min())
            and max(values) + 100_000 <= float(cached[name].max())
            for name, values in (("X", x), ("Y", y))
        )
        covers_time = (np.datetime64(cached.time.values[0]) == np.datetime64(START)
                       and np.datetime64(cached.time.values[-1]) == np.datetime64(END))
        has_fields = not include_wind or all(name in cached for name in ("Uwind_eastward", "Vwind_northward"))
        if cached.attrs.get("validation_source") == source and covers_region and covers_time and has_fields:
            print(f"using cached source {snapshot}", flush=True)
            return reader_netCDF_CF_generic.Reader(cached, name="NorKyst validation")
        cached.close()
    dataset = xr.open_dataset(source)
    times = np.asarray(dataset.time.values)
    if times.ndim != 1 or np.isnat(times).any():
        raise ValueError("source timeline contains invalid timestamps")
    # match the cron's last-record policy for duplicate forecast timestamps
    _, reverse_indices = np.unique(times[::-1], return_index=True)
    indices = len(times) - 1 - reverse_indices
    indices = indices[np.argsort(times[indices])]
    selected = indices[(times[indices] >= np.datetime64(START))
                       & (times[indices] <= np.datetime64(END))]
    expected = np.arange(np.datetime64(START), np.datetime64(END) + np.timedelta64(1, "h"),
                         np.timedelta64(1, "h"))
    if not np.array_equal(times[selected], expected):
        raise ValueError(f"NorKyst source does not cover every hour from {START} to {END}")
    dataset = dataset.isel(time=selected)
    if points is not None:
        # a 100 km margin keeps the downloaded boundary well outside the tracks
        x, y = projected_points(dataset)
        subsets = {}
        for name, values in (("X", x), ("Y", y)):
            axis = dataset[name].values
            first = max(0, int(np.searchsorted(axis, min(values) - 100_000)) - 1)
            last = min(len(axis), int(np.searchsorted(axis, max(values) + 100_000)) + 1)
            subsets[dataset[name].dims[0]] = slice(first, last)
        subsets[dataset.depth.dims[0]] = slice(0, 1)
        fields = ["u_eastward", "v_northward", "projection_stere"]
        if include_wind:
            fields.extend(["Uwind_eastward", "Vwind_northward"])
        surface = dataset[fields].isel(subsets)
        surface.attrs["validation_source"] = source
        temporary = snapshot.with_suffix(".tmp.nc")
        print(f"caching native surface currents {dict(surface.sizes)}", flush=True)
        # bounded time chunks avoid one large OPeNDAP array request
        import dask
        with dask.config.set(scheduler="synchronous"):
            surface.chunk({"time": 1}).to_netcdf(temporary)
        temporary.replace(snapshot)
        dataset.close()
        dataset = xr.open_dataset(snapshot)
    reader = reader_netCDF_CF_generic.Reader(dataset, name="NorKyst validation")
    return reader


def run_opendrift(directory, lon, lat, reader):
    """advance a single surface particle with no stochastic or wave drift."""
    from opendrift.models.oceandrift import OceanDrift

    model = OceanDrift(loglevel=30)
    model.add_reader(reader)
    for key, value in {
        "drift:advection_scheme": "runge-kutta4",
        "environment:constant:horizontal_diffusivity": 0.0,
        "environment:constant:x_wind": 0.0,
        "environment:constant:y_wind": 0.0,
        "drift:current_uncertainty": 0.0,
        "drift:wind_uncertainty": 0.0,
        "drift:vertical_mixing": False,
        "drift:vertical_advection": False,
        "drift:stokes_drift": False,
    }.items():
        model.set_config(key, value)
    model.seed_elements(lon=lon, lat=lat, radius=0, number=1,
                        time=START, z=0, wind_drift_factor=0)
    model.run(duration=END - START, time_step=3600, time_step_output=3600,
              outfile=str(directory / "opendrift_output.nc"))


def check_trajectory(path):
    """reject incomplete results rather than silently comparing shorter tracks."""
    rows = json.loads(path.read_text())
    if [r["step"] for r in rows] != list(range(STEPS + 1)):
        raise ValueError(f"expected all {STEPS + 1} snapshots in {path}")
    if not all(np.isfinite([r["lon"], r["lat"]]).all() for r in rows):
        raise ValueError(f"nonfinite trajectory in {path}")
    if any(r["lon"] == 0.0 and r["lat"] == 0.0 for r in rows):
        raise ValueError(f"empty active-particle centroid in {path}")


def main():
    """build the current CLI and record paired runs with their provenance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=SOURCE, help="NorKyst OPeNDAP URL or local NetCDF")
    parser.add_argument("--tile-url", default=TILE_URL, help="complete NorKyst root, including /norkyst")
    parser.add_argument("--grid-size", type=int, default=8)
    parser.add_argument("--limit", type=int, help="run only the first N grid points")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--dry-run", action="store_true", help="write configs without fetching or running")
    args = parser.parse_args()
    if args.grid_size < 2 or (args.limit is not None and args.limit < 1):
        parser.error("grid size must be at least 2 and limit must be positive")
    points = make_grid(args.grid_size)
    if args.limit:
        points = points[:args.limit]
    RUNS.mkdir(exist_ok=True)
    if args.dry_run:
        for i, (lon, lat) in enumerate(points):
            directory = RUNS / f"run_{i:03d}"
            directory.mkdir(exist_ok=True)
            (directory / "config.json").write_text(json.dumps(make_config(lon, lat, args.tile_url), indent=2))
        print(f"wrote {len(points)} configs to {RUNS}")
        return
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "proteus-cli"], cwd=ROOT, check=True)
    if not CLI_BIN.exists():
        raise FileNotFoundError(f"build the CLI first at {CLI_BIN}")
    request = urllib.request.Request(
        args.tile_url.rstrip("/") + "/manifest.json",
        headers={"User-Agent": "Proteus-validation/1.0"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        manifest = json.load(response)
    (RUNS / "manifest.json").write_text(json.dumps(manifest, indent=2))
    reader = make_reader(args.source, make_grid(args.grid_size))
    failed = 0
    for i, (lon, lat) in enumerate(points):
        directory = RUNS / f"run_{i:03d}"
        directory.mkdir(exist_ok=True)
        # only publish comparable centroids after both simulations succeed
        for name in ("opendrift_centroids.json", "proteus_centroids.json", "opendrift_output.nc"):
            (directory / name).unlink(missing_ok=True)
        meta = {"run_index": i, "release_lon": lon, "release_lat": lat,
                "start_time": START.isoformat(), "end_time": END.isoformat(),
                "source": args.source, "tile_url": args.tile_url,
                "n_particles": 1, "time_step_seconds": 3600,
                "wind_factor": 0.0, "wind_deflection": 0.0,
                "status": "failed"}
        print(f"[{i + 1}/{len(points)}] lon={lon:.4f} lat={lat:.4f}", flush=True)
        try:
            seconds, result = run_cli(directory, make_config(lon, lat, args.tile_url))
            (directory / "proteus.log").write_text(result.stdout + result.stderr, encoding="utf-8")
            if result.returncode:
                raise RuntimeError(f"CLI failed with exit {result.returncode}, see {directory / 'proteus.log'}")
            check_trajectory(directory / "proteus_centroids.json")
            meta["cli_seconds"] = seconds
            started = time.perf_counter()
            run_opendrift(directory, lon, lat, reader)
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
        raise SystemExit(f"{failed} of {len(points)} runs failed, inspect meta.json and proteus.log")


if __name__ == "__main__":
    main()
