#!/usr/bin/env python3
"""matched_placement.py isolates the effect of the shoreline placement algorithm."""

import argparse
import csv
import json
import shutil
import sys
from pathlib import Path

import numpy as np
from roaring_landmask import RoaringLandmask

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from landmask_stranding.run import ROOT, make_reader, read_cli, run_opendrift
from landmask_stranding.diagnose import diagnose
from landmask_stranding.visualize import visualize


def segment_crossing(start, end, contains, land_side=True):
    """follow Proteus's segment search with a 0.0001 degree refinement tolerance."""
    if not np.isfinite([*start, *end]).all():
        return end
    if contains(*start):
        return start
    dx = (end[0] - start[0] + 180) % 360 - 180
    dy = end[1] - start[1]
    span = max(abs(dx), abs(dy))

    def position(fraction):
        return ((start[0] + dx * fraction + 180) % 360 - 180,
                start[1] + dy * fraction)

    samples = max(1, int(np.ceil(span / 0.001)))
    water = 0.0
    for index in range(1, samples + 1):
        land = index / samples
        if contains(*position(land)):
            while (land - water) * span > 0.0001:
                middle = (water + land) * 0.5
                if contains(*position(middle)):
                    land = middle
                else:
                    water = middle
            return position(land if land_side else water)
        water = land
    return end


def matched_crossing(lon1, lat1, lon2, lat2, step_degrees, land_side=True):
    """replace only placement while preserving OpenDrift's incoming coordinates and mask."""
    mask = RoaringLandmask.new()
    output = [segment_crossing((float(x1), float(y1)), (float(x2), float(y2)),
                               mask.contains, land_side)
              for x1, y1, x2, y2 in zip(lon1, lat1, lon2, lat2)]
    if not output:
        return np.asarray(lon2).copy(), np.asarray(lat2).copy()
    return np.asarray(output).T


def main():
    """retain the original comparison and rerun OpenDrift with matched placement."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", nargs="?", default="run_000")
    args = parser.parse_args()
    original = HERE / "runs" / args.run
    output = original / "matched_placement"
    output.mkdir(exist_ok=True)
    config = json.loads((original / "config.json").read_text())
    summary = json.loads((original / "summary.json").read_text())
    births, counts = read_cli(original, config["particles"])
    for name in ("config.json", "particles.jsonl", "particles_hourly.jsonl", "proteus_centroids.json"):
        shutil.copy2(original / name, output / name)
    release = config["releases"][0]
    reader = make_reader(summary["source"], [(release["lon"], release["lat"])],
                         include_wind=True, cache_dir=ROOT / "validation/norkyst_leeway/runs")
    import opendrift.models.basemodel as basemodel
    previous = basemodel.coastline_crossing
    try:
        basemodel.coastline_crossing = matched_crossing
        stranded = run_opendrift(output, config, reader, births)
    finally:
        basemodel.coastline_crossing = previous
    if len(stranded) != len(counts):
        raise ValueError("incomplete matched OpenDrift output")
    for row, number in zip(counts, stranded):
        row["opendrift_stranded"] = int(number)
    with (output / "stranding_counts.csv").open("w", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=list(counts[0]))
        writer.writeheader()
        writer.writerows(counts)
    summary.update(final=counts[-1], opendrift_placement="Proteus segment search and tolerance")
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    diagnose(output)
    visualize(output)


if __name__ == "__main__":
    main()
