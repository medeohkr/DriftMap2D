#!/usr/bin/env python3
"""visualize.py compares any saved NorKyst leeway run on matching map axes."""

import argparse
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt
from matplotlib.animation import FuncAnimation, PillowWriter
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from shared.metrics import load_centroids, run_error_series


def all_particles_stranded(path):
    """allow a finished stranding run to retain its terminal positions."""
    import xarray as xr

    with xr.open_dataset(path) as dataset:
        return bool((dataset.status == 1).any(dim="time").all())


def reproduce(directory, output, config, meta):
    """optionally recreate both models using this run's catalog and settings."""
    from norkyst_leeway.run import START, RUNS, make_grid, make_reader, run_opendrift, validate_object, deterministic_overrides
    from shared.opendrift_utils import save_centroids_from_nc
    from shared.proteus_utils import run_cli

    if (config["start_date"] != START.strftime("%Y-%m-%d %H:%M") or
            config["steps"] != 48 or config["time_step_minutes"] != 60):
        raise ValueError("rerun supports the suite's october 28–30 hourly setup")
    selected = config["tracer_json"]
    if isinstance(selected, str):
        selected = json.loads(selected)
    object_id = selected["object_id"]
    catalog_dir = (directory / config.get("catalog_dir", "../../../../catalogs")).resolve()
    object_type, _ = validate_object(catalog_dir, object_id)
    copy = dict(config, catalog_dir=str(catalog_dir), particle_output="particles.jsonl", snapshot_every=1)
    copy["tracer_json"] = {"object_id": object_id,
                           "overrides": deterministic_overrides(catalog_dir, object_id)}
    elapsed, result = run_cli(output, copy)
    (output / "proteus.log").write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError("CLI failed, see visualization/proteus.log")
    release = config["releases"][0]
    reader = make_reader(meta["source"], [(release["lon"], release["lat"])],
                         include_wind=True, cache_dir=RUNS)
    (output / "opendrift_output.nc").unlink(missing_ok=True)
    run_opendrift(output, release["lon"], release["lat"], reader, config["particles"],
                   object_type, meta.get("opendrift_seed", 20261028 + meta["run_index"]))
    save_centroids_from_nc(output / "opendrift_output.nc", output / "opendrift_centroids.json")
    print(f"recreated both models, CLI {elapsed:.1f}s", flush=True)


def main():
    """save side-by-side, overlaid, and animated centroid comparisons."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", help="run number or directory name, such as 0 or run_039")
    parser.add_argument("--runs-dir", type=Path, default=HERE / "runs")
    parser.add_argument("--rerun", action="store_true", help="recreate both models into the visualization directory")
    parser.add_argument("--skip-animation", action="store_true")
    parser.add_argument("--sample-particles", type=int, default=1000,
                        help="stable random sample per model for the particle cloud GIF")
    args = parser.parse_args()
    if args.sample_particles < 1:
        parser.error("sample particles must be positive")
    try:
        name = f"run_{int(args.run.removeprefix('run_')):03d}"
    except ValueError:
        parser.error("run must be a number or run_NNN")
    directory = (args.runs_dir / name).resolve()
    if not (directory / "config.json").exists() or not (directory / "meta.json").exists():
        parser.error(f"missing config or metadata in {directory}")
    meta = json.loads((directory / "meta.json").read_text())
    config = json.loads((directory / "config.json").read_text())
    output = directory / "visualization"
    output.mkdir(exist_ok=True)
    inputs = output if args.rerun else directory
    if not args.rerun and not (inputs / "opendrift_centroids.json").exists():
        if all((output / filename).exists() for filename in
               ("opendrift_centroids.json", "proteus_centroids.json")):
            inputs = output
    if args.rerun:
        reproduce(directory, output, config, meta)
    for filename in ("opendrift_centroids.json", "proteus_centroids.json"):
        if not (inputs / filename).exists():
            parser.error(f"missing {inputs / filename}, use --rerun to recreate both models")
    steps, od_lon, od_lat = load_centroids(inputs / "opendrift_centroids.json")
    cli_steps, cli_lon, cli_lat = load_centroids(inputs / "proteus_centroids.json")
    if (len(steps) < len(cli_steps) and np.array_equal(steps, cli_steps[:len(steps)])
            and all_particles_stranded(inputs / "opendrift_output.nc")):
        od_lon = np.pad(od_lon, (0, len(cli_steps) - len(steps)), mode="edge")
        od_lat = np.pad(od_lat, (0, len(cli_steps) - len(steps)), mode="edge")
        steps = cli_steps.copy()
    if not np.array_equal(steps, cli_steps) or len(steps) != config["steps"] + 1:
        raise ValueError("trajectories have incomplete or mismatched steps")
    if not np.isfinite([od_lon, od_lat, cli_lon, cli_lat]).all():
        raise ValueError("trajectories contain nonfinite centroids")
    hours = steps * config["time_step_minutes"] / 60
    _, separation, _ = run_error_series(od_lon, od_lat, cli_lon, cli_lat)
    release = config["releases"][0]
    xs, ys = np.r_[od_lon, cli_lon], np.r_[od_lat, cli_lat]
    dx, dy = max(np.ptp(xs) * 0.12, 0.01), max(np.ptp(ys) * 0.12, 0.01)
    extent = (xs.min() - dx, xs.max() + dx, ys.min() - dy, ys.max() + dy)
    tracks = [("OpenDrift", od_lon, od_lat, "#37579b"),
              ("Proteus CLI", cli_lon, cli_lat, "#e27a23")]
    title = (f"{name}, {meta['object_id']}, {config['particles']:,} particles\n"
             f"{meta['start_time']} to {meta['end_time']} UTC, final separation {separation[-1] * 1000:.0f} m")

    def style(ax):
        ax.set(xlim=extent[:2], ylim=extent[2:], xlabel="longitude (degrees east)",
               ylabel="latitude (degrees north)")
        ax.set_aspect(1 / np.cos(np.radians(release["lat"])))
        ax.grid(alpha=0.25)
        ax.scatter(release["lon"], release["lat"], marker="*", color="black", s=90, zorder=5)

    fig, axes = plt.subplots(1, 2, figsize=(13, 7), layout="constrained")
    for ax, (label, lon, lat, color) in zip(axes, tracks):
        style(ax)
        ax.set_title(label)
        ax.plot(lon, lat, color=color, linewidth=2)
        for i in range(0, len(steps), 6):
            ax.scatter(lon[i], lat[i], color=color, s=20)
            ax.annotate(f"{hours[i]:g}h", (lon[i], lat[i]), xytext=(5, 5), textcoords="offset points")
        ax.scatter(lon[-1], lat[-1], color=color, marker="s", s=40)
    fig.suptitle(title)
    fig.savefig(output / "side_by_side.png", dpi=180)
    plt.close(fig)
    fig, ax = plt.subplots(figsize=(10, 8), layout="constrained")
    style(ax)
    for label, lon, lat, color in tracks:
        ax.plot(lon, lat, color=color, linewidth=2, label=label)
        ax.scatter(lon[-1], lat[-1], color=color, marker="s", s=40)
    ax.set_title(title)
    ax.legend()
    fig.savefig(output / "overlay.png", dpi=180)
    plt.close(fig)
    if not args.skip_animation:
        fig, axes = plt.subplots(1, 2, figsize=(12, 7), layout="constrained")
        artists = []
        for ax, (label, lon, lat, color) in zip(axes, tracks):
            style(ax)
            ax.set_title(label)
            line, = ax.plot([], [], color=color, linewidth=2)
            dot, = ax.plot([], [], "o", color=color, markersize=7)
            artists.append((line, dot, lon, lat))
        heading = fig.suptitle(title)

        def update(frame):
            for line, dot, lon, lat in artists:
                line.set_data(lon[:frame + 1], lat[:frame + 1])
                dot.set_data([lon[frame]], [lat[frame]])
            heading.set_text(f"{name}, hour {hours[frame]:g}, separation {separation[frame] * 1000:.0f} m")

        animation = FuncAnimation(fig, update, frames=len(steps), interval=170)
        animation.save(output / "side_by_side.gif", writer=PillowWriter(fps=6))
        plt.close(fig)
        particle_path = inputs / "particles.jsonl"
        netcdf_path = inputs / "opendrift_output.nc"
        if particle_path.exists() and netcdf_path.exists():
            animate_particles(particle_path, netcdf_path, output, name, config,
                              args.sample_particles)
        else:
            print("particle cloud GIF requires particles.jsonl and OpenDrift NetCDF, use --rerun")
    print(f"saved comparison outputs in {output}")


def animate_particles(particle_path, netcdf_path, output, name, config, sample_count):
    """stream CLI snapshots and retain a fixed random subset of stable particle IDs."""
    import xarray as xr

    count = min(sample_count, config["particles"])
    # random selection avoids selecting only one alternating leeway orientation
    selected = np.sort(np.random.default_rng(0).choice(config["particles"], count, replace=False))
    lookup = {int(particle_id): i for i, particle_id in enumerate(selected)}
    positions, statuses, steps, times = [], [], [], []
    with particle_path.open() as source:
        for line in source:
            snapshot = json.loads(line)
            points = np.full((count, 2), np.nan)
            stranded = np.zeros(count, dtype=bool)
            for particle in snapshot["particles"]:
                index = lookup.get(particle["id"])
                if index is not None:
                    points[index] = [particle["lon"], particle["lat"]]
                    stranded[index] = particle["stranded"]
            positions.append(points)
            statuses.append(stranded)
            steps.append(snapshot["step"])
            times.append(snapshot["time"])
    if not positions:
        raise ValueError("particle output is empty")
    cli = np.asarray(positions)
    cli_stranded = np.asarray(statuses)
    with xr.open_dataset(netcdf_path) as dataset:
        size = dataset.sizes["trajectory"]
        ids = np.sort(np.random.default_rng(1).choice(size, min(count, size), replace=False))
        subset = dataset.isel(trajectory=ids)
        od_times = np.asarray(dataset.time.values).astype("datetime64[s]")
        frame_times = np.asarray([np.datetime64(value.removesuffix('Z'), 's') for value in times])
        indices = np.searchsorted(od_times, frame_times)
        beyond = indices >= len(od_times)
        if beyond.any() and not all_particles_stranded(netcdf_path):
            raise ValueError("OpenDrift stopped before every particle stranded")
        indices = np.minimum(indices, len(od_times) - 1)
        if not np.array_equal(od_times[indices[~beyond]], frame_times[~beyond]):
            raise ValueError("particle timestamps do not match OpenDrift output")
        subset = subset.isel(time=indices)
        od = np.stack([subset.lon.transpose("time", "trajectory").values,
                       subset.lat.transpose("time", "trajectory").values], axis=-1)
        od_stranded = np.maximum.accumulate(
            subset.status.transpose("time", "trajectory").values == 1, axis=0)
        # OpenDrift masks deactivated particles after their stranding frame
        for frame in range(1, len(od)):
            missing = ~np.isfinite(od[frame]).all(axis=1) & od_stranded[frame]
            od[frame, missing] = od[frame - 1, missing]
    all_points = np.concatenate([cli.reshape(-1, 2), od.reshape(-1, 2)])
    all_points = all_points[np.isfinite(all_points).all(axis=1)]
    if not len(all_points):
        raise ValueError("no finite sampled particle positions")
    low, high = all_points.min(axis=0), all_points.max(axis=0)
    margin = np.maximum((high - low) * 0.08, 0.01)
    from roaring_landmask import RoaringLandmask
    map_x = np.linspace(low[0] - margin[0], high[0] + margin[0], 400)
    map_y = np.linspace(low[1] - margin[1], high[1] + margin[1], 400)
    map_lon, map_lat = np.meshgrid(map_x, map_y)
    land_mask = np.asarray(RoaringLandmask.new().contains_many(
        map_lon.ravel(), map_lat.ravel())).reshape(map_lon.shape)
    fig, axes = plt.subplots(1, 2, figsize=(12, 7), layout="constrained")
    artists = []
    for ax, label, data, stranded, color in zip(
            axes, ("OpenDrift", "Proteus CLI"), (od, cli), (od_stranded, cli_stranded),
            ("#37579b", "#e27a23")):
        ax.set(xlim=(low[0] - margin[0], high[0] + margin[0]),
               ylim=(low[1] - margin[1], high[1] + margin[1]),
               xlabel="longitude (degrees east)", ylabel="latitude (degrees north)", title=label)
        ax.set_aspect(1 / np.cos(np.radians(config["releases"][0]["lat"])))
        ax.contourf(map_x, map_y, land_mask, levels=[0.5, 1.5], colors=["#dedede"], zorder=0)
        if land_mask.any() and not land_mask.all():
            ax.contour(map_x, map_y, land_mask, levels=[0.5], colors=["#777777"], linewidths=0.4)
        ax.grid(alpha=0.25)
        marker_size = 4 if count >= 10000 else 8
        opacity = 0.6
        active = ax.scatter([], [], s=marker_size, alpha=opacity, color=color,
                            edgecolors="none", linewidths=0, label="active")
        land = ax.scatter([], [], s=max(marker_size, 5), alpha=0.75, color="black",
                          edgecolors="none", linewidths=0, label="stranded")
        legend = ax.legend(loc="upper left", fontsize=11, markerscale=2)
        for handle in legend.legend_handles:
            handle.set_alpha(1.0)
        artists.append((active, land, data, stranded))
    heading = fig.suptitle("")

    def update(frame):
        counts = []
        for active, land, data, stranded in artists:
            finite = np.isfinite(data[frame]).all(axis=1)
            active.set_offsets(data[frame][finite & ~stranded[frame]])
            land.set_offsets(data[frame][finite & stranded[frame]])
            counts.append(int(np.count_nonzero(finite & stranded[frame])))
        hour = steps[frame] * config["time_step_minutes"] / 60
        heading.set_text(f"{name}, {config['particles']:,} simulated particles per model\n"
                         f"hour {hour:g}, stranded shown OD {counts[0]:,}, CLI {counts[1]:,}")

    print(f"rendering particle clouds with up to {count} stable IDs per model", flush=True)
    animation = FuncAnimation(fig, update, frames=len(steps), interval=170)
    animation.save(output / "particle_clouds.gif", writer=PillowWriter(fps=6))
    update(len(steps) - 1)
    fig.savefig(output / "particle_clouds_final.png", dpi=150)
    print(f"final sampled stranded counts, OpenDrift {od_stranded[-1].sum()}, CLI {cli_stranded[-1].sum()}")
    plt.close(fig)


if __name__ == "__main__":
    main()
