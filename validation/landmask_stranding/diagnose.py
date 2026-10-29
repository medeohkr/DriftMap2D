#!/usr/bin/env python3
"""diagnose.py compares release batches and their distances from the GSHHG shoreline."""

import argparse
import csv
import json
from pathlib import Path

import numpy as np
import xarray as xr
from pyproj import Geod, Transformer
from roaring_landmask import Shapes, LandmaskProvider
from shapely import LineString, STRtree, box, points, transform, wkb

HERE = Path(__file__).resolve().parent


def shoreline_distances(coordinates):
    """measure distance in local projected meters to the original polygon boundaries."""
    region = box(coordinates[:, 0].min() - 0.05, coordinates[:, 1].min() - 0.05,
                 coordinates[:, 0].max() + 0.05, coordinates[:, 1].max() + 0.05)
    projection = Transformer.from_crs("EPSG:4326", "EPSG:32632", always_xy=True)
    polygons = list(wkb.loads(Shapes.wkb(LandmaskProvider.Gshhg)).geoms)
    tree = STRtree(polygons)
    lines = []
    for index in tree.query(region, predicate="intersects"):
        clipped = polygons[index].boundary.intersection(region)
        pieces = [clipped] if clipped.geom_type == "LineString" else list(clipped.geoms)
        for piece in pieces:
            if piece.geom_type != "LineString":
                continue
            vertices = np.asarray(piece.coords)
            for start in range(0, len(vertices) - 1, 32):
                line = LineString(vertices[start:start + 33])
                lines.append(transform(line, projection.transform, interleaved=False))
    projected = points(*projection.transform(coordinates[:, 0], coordinates[:, 1]))
    _, distances = STRtree(lines).query_nearest(projected, return_distance=True, all_matches=False)
    return distances


def diagnose(directory):
    """pair one representative per identical birth batch using stable particle IDs."""
    summary = json.loads((directory / "summary.json").read_text())
    batches = summary["release_batches"]
    identities = np.r_[0, np.cumsum([batch["count"] for batch in batches])[:-1]]
    lookup = {int(identity): index for index, identity in enumerate(identities)}
    positions, statuses = [], []
    with (directory / "particles.jsonl").open() as source:
        for line in source:
            snapshot = json.loads(line)
            position = np.full((len(identities), 2), np.nan)
            status = np.zeros(len(identities), dtype=bool)
            for particle in snapshot["particles"]:
                index = lookup.get(particle["id"])
                if index is not None:
                    position[index] = [particle["lon"], particle["lat"]]
                    status[index] = particle["stranded"]
            positions.append(position)
            statuses.append(status)
    cli, cli_status = np.asarray(positions), np.asarray(statuses)
    with xr.open_dataset(directory / "opendrift_output.nc") as dataset:
        subset = dataset.sel(trajectory=identities)
        od = np.stack([subset.lon.transpose("time", "trajectory").values,
                       subset.lat.transpose("time", "trajectory").values], axis=-1)
        od_status = np.maximum.accumulate(
            subset.status.transpose("time", "trajectory").values == 1, axis=0)
        for frame in range(1, len(od)):
            missing = ~np.isfinite(od[frame]).all(axis=1) & od_status[frame]
            od[frame, missing] = od[frame - 1, missing]
    paired = cli_status[-1] & od_status[-1]
    if not paired.any():
        raise ValueError("no paired stranded release batches")
    cli_shore = shoreline_distances(cli[-1, paired])
    od_shore = shoreline_distances(od[-1, paired])
    geod = Geod(ellps="WGS84")

    def distance(a, b):
        return float(geod.inv(*a, *b)[2])

    rows = []
    for shore_index, index in enumerate(np.flatnonzero(paired)):
        common_water = (~cli_status[::4, index] & ~od_status[:, index]
                        & np.isfinite(cli[::4, index]).all(axis=1)
                        & np.isfinite(od[:, index]).all(axis=1))
        last_water = np.flatnonzero(common_water)[-1]
        rows.append({
            "particle_id": int(identities[index]), "batch_count": batches[index]["count"],
            "birth_hour": batches[index]["step"] / 4,
            "proteus_stranding_hour": int(np.flatnonzero(cli_status[:, index])[0]) / 4,
            "opendrift_first_stranded_output_hour": int(np.flatnonzero(od_status[:, index])[0]),
            "last_shared_water_hour": int(last_water),
            "last_shared_water_separation_m": distance(cli[last_water * 4, index], od[last_water, index]),
            "stranded_separation_m": distance(cli[-1, index], od[-1, index]),
            "proteus_shore_distance_m": float(cli_shore[shore_index]),
            "opendrift_shore_distance_m": float(od_shore[shore_index]),
        })
    with (directory / "shoreline_comparison.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    report = {"paired_stranded_batches": len(rows),
              "unpaired_final_stranding_batches": int(np.count_nonzero(cli_status[-1] != od_status[-1]))}
    for field in ("last_shared_water_separation_m", "stranded_separation_m",
                  "proteus_shore_distance_m", "opendrift_shore_distance_m"):
        report[field] = dict(zip(("min", "median", "p90", "max"),
                                np.percentile([row[field] for row in rows], [0, 50, 90, 100]).tolist()))
    (directory / "shoreline_comparison.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", nargs="?", default="run_000")
    args = parser.parse_args()
    diagnose(HERE / "runs" / args.run)
