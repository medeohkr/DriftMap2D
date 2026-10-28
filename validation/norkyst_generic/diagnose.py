#!/usr/bin/env python3
"""diagnose.py compares website tracks and source currents with published tiles."""

import argparse
import csv
import io
import json
import struct
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

import netCDF4
import numpy as np
import zstandard
from pyproj import Transformer

HERE = Path(__file__).resolve().parent
ROOT_URL = "https://tiles.driftmap2d.com/norkyst"
START = datetime(2026, 10, 27)


def fetch(url):
    """identify validation requests explicitly to the public tile server."""
    request = urllib.request.Request(url, headers={"User-Agent": "Proteus-validation/1.0"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def bilinear(array, x, y):
    """sample the four surrounding nodes without altering missing data."""
    col, row = int(np.floor(x)), int(np.floor(y))
    fx, fy = x - col, y - row
    return ((1 - fx) * (1 - fy) * array[..., row, col]
            + fx * (1 - fy) * array[..., row, col + 1]
            + (1 - fx) * fy * array[..., row + 1, col]
            + fx * fy * array[..., row + 1, col + 1])


def main():
    """compare raw forcing at identical positions to isolate input differences."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--website", type=Path)
    args = parser.parse_args()
    run = HERE / "runs/run_039"
    cli = json.loads((run / "proteus_centroids.json").read_text())
    od = json.loads((run / "opendrift_centroids.json").read_text())
    if args.website:
        website = json.loads(args.website.read_text())
        positions = [f["geometry"]["geometries"][0]["coordinates"][0]
                     for f in website["features"]]
        days = [f["properties"]["day"] for f in website["features"]]
        print("website snapshots", len(positions), "hours", [days[0] * 24, days[-1] * 24])
        differences = [max(abs(point[axis] - cli[round(day * 24)][key])
                           for axis, key in enumerate(("lon", "lat")))
                       for point, day in zip(positions, days)]
        print("website vs CLI max coordinate difference", max(differences), "degrees")
        print("website endpoint", positions[-1], "CLI endpoint", [cli[-1]["lon"], cli[-1]["lat"]])
    manifest = json.loads(fetch(ROOT_URL + "/manifest.json"))
    projection = manifest["grid"]["projection"]["proj4"]
    project = Transformer.from_crs("EPSG:4326", projection, always_xy=True)
    source = netCDF4.Dataset(HERE / "runs/source_surface.nc")
    x_axis, y_axis = source.variables["X"][:], source.variables["Y"][:]
    cache = {}
    rows = []
    for hour, position in enumerate(od):
        x, y = project.transform(position["lon"], position["lat"])
        gx, gy = x / 800, y / 800
        col, row = int(gx // 120), int(gy // 120)
        date = START + timedelta(hours=hour)
        key = (date.strftime("%Y/%m/%d"), col, row)
        if key not in cache:
            url = f"{ROOT_URL}/{key[0]}/{col:03d}_{row:03d}.bin"
            raw = zstandard.ZstdDecompressor().stream_reader(io.BytesIO(fetch(url))).read()
            size = struct.unpack_from("<I", raw, 4)[0]
            header = json.loads(raw[8:8 + size])
            values = np.frombuffer(raw, dtype="<f2", offset=8 + size).astype(np.float32)
            cache[key] = (header, values.reshape(5, *header["shape"]))
        header, tile = cache[key]
        timestamp = int(date.replace(tzinfo=timezone.utc).timestamp())
        frame = header["timestamps"].index(timestamp)
        tile_uv = bilinear(tile[:2, frame], gx - col * 120 + 1, gy - row * 120 + 1)
        sx, sy = (x - x_axis[0]) / 800, (y - y_axis[0]) / 800
        ix, iy = int(sx), int(sy)
        patches = np.array([np.ma.filled(source.variables[name][hour, 0, iy:iy+2, ix:ix+2], np.nan)
                            for name in ("u_eastward", "v_northward")])
        source_uv = bilinear(patches, sx - ix, sy - iy)
        result = {"hour": hour, "lon": position["lon"], "lat": position["lat"],
                  "tile_u": float(tile_uv[0]), "tile_v": float(tile_uv[1]),
                  "source_u": float(source_uv[0]), "source_v": float(source_uv[1]),
                  "difference_u": float(tile_uv[0] - source_uv[0]),
                  "difference_v": float(tile_uv[1] - source_uv[1])}
        rows.append(result)
        if hour % 6 == 0 or hour in (32, 34, 35, 37, 38, 39, 40):
            print(hour, "tile", tile_uv, "source", source_uv, "difference", tile_uv - source_uv, flush=True)
    output = HERE / "runs/worst_case/forcing_comparison.csv"
    with output.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    source.close()
    print("wrote", output)


if __name__ == "__main__":
    main()
