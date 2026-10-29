#!/usr/bin/env python3
"""build_landmask_polygons.py clips OpenDrift's GSHHG polygons into indexed tiles."""

import argparse
import hashlib
import json
from pathlib import Path

import zstandard
from shapely import STRtree, box, wkb

ROOT = Path(__file__).resolve().parents[1]


def polygons(geometry):
    """retain polygon components and interior rings without simplification."""
    if geometry.is_empty:
        return []
    if geometry.geom_type == 'Polygon':
        return [geometry] if geometry.area > 0 else []
    if hasattr(geometry, 'geoms'):
        return [polygon for part in geometry.geoms for polygon in polygons(part)]
    return []


def encode_tile(geometries, index):
    """index quarter-degree polygon fragments inside a ten-degree tile."""
    west, south = index[0] * 10 - 180, index[1] * 10 - 90
    cells = [[] for _ in range(1600)]
    if geometries:
        tree = STRtree(geometries)
        # clip in two stages so continent-sized rings are not scanned per bucket
        for by in range(10):
            for bx in range(10):
                bounds = box(west + bx, south + by, west + bx + 1, south + by + 1)
                parts = [part for i in tree.query(bounds, predicate='intersects')
                         for part in polygons(geometries[i].intersection(bounds))]
                if not parts:
                    continue
                subindex = STRtree(parts)
                for dy in range(4):
                    for dx in range(4):
                        x, y = west + bx + dx / 4, south + by + dy / 4
                        bounds = box(x, y, x + 0.25, y + 0.25)
                        cell = cells[(by * 4 + dy) * 40 + bx * 4 + dx]
                        for i in subindex.query(bounds, predicate='intersects'):
                            for polygon in polygons(parts[i].intersection(bounds)):
                                cell.append([list(polygon.exterior.coords),
                                             *[list(ring.coords) for ring in polygon.interiors]])
    payload = {'version': 1, 'tile_index': list(index), 'cell_size': 0.25, 'cells': cells}
    raw = b'LMP1' + json.dumps(payload, separators=(',', ':'), allow_nan=False).encode()
    if len(raw) > 64 * 1024 * 1024:
        raise ValueError(f'polygon tile {index} exceeds reader size limit')
    return zstandard.ZstdCompressor(level=3).compress(raw)


def main():
    """generate all 648 tiles or selected indices for local validation."""
    from roaring_landmask import Shapes, LandmaskProvider

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'data/landmask_polygons')
    parser.add_argument('--tile', nargs=2, type=int, action='append', metavar=('X', 'Y'))
    args = parser.parse_args()
    indices = args.tile or [(x, y) for x in range(36) for y in range(18)]
    if any(not (0 <= x < 36 and 0 <= y < 18) for x, y in indices):
        parser.error('indices must be in x 0..35 and y 0..17')
    indices = sorted(set(map(tuple, indices)))
    raw = Shapes.wkb(LandmaskProvider.Gshhg)
    digest = hashlib.sha256(raw).hexdigest()
    geometry = wkb.loads(raw)
    parts = polygons(geometry)
    index = STRtree(parts)
    args.output.mkdir(parents=True, exist_ok=True)
    records = []
    for count, (x, y) in enumerate(indices, 1):
        bounds = box(x * 10 - 180, y * 10 - 90, x * 10 - 170, y * 10 - 80)
        clipped = [part for i in index.query(bounds, predicate='intersects')
                   for part in polygons(parts[i].intersection(bounds))]
        payload = encode_tile(clipped, (x, y))
        name = f'landmask_{x:03d}_{y:03d}.bin.zst'
        (args.output / name).write_bytes(payload)
        records.append({'path': name, 'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()})
        print(f'{count}/{len(indices)} {name} {len(payload):,} bytes', flush=True)
    manifest = {'format': 'LMP1', 'source': 'roaring_landmask GSHHG full-resolution WKB',
                'source_sha256': digest, 'complete': len(indices) == 648, 'tiles': records}
    (args.output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'finished {len(indices)} tiles, {sum(row["bytes"] for row in records):,} bytes')


if __name__ == '__main__':
    main()
