#!/usr/bin/env python3
"""compare.py checks local Norway polygons against OpenDrift at 50,000 points."""

import shutil
import subprocess
import urllib.request
from pathlib import Path

import numpy as np
from roaring_landmask import RoaringLandmask

ROOT = Path(__file__).resolve().parents[2]


def main():
    """combine published rasters and generated polygons for a native Rust check."""
    output = ROOT / 'data/landmask_local'
    (output / 'polygons').mkdir(parents=True, exist_ok=True)
    for y in (14, 15):
        name = f'landmask_018_{y:03d}.bin'
        request = urllib.request.Request('https://tiles.driftmap2d.com/roaring_landmask/' + name,
                                         headers={'User-Agent': 'Proteus-validation/1.0'})
        with urllib.request.urlopen(request, timeout=60) as response:
            (output / name).write_bytes(response.read())
        shutil.copyfile(ROOT / 'data/landmask_polygons' / (name + '.zst'),
                        output / 'polygons' / (name + '.zst'))
    rng = np.random.default_rng(42)
    lon = rng.uniform(4.5, 6, 50000).astype(np.float32).astype(np.float64)
    lat = rng.uniform(58.5, 60.5, 50000).astype(np.float32).astype(np.float64)
    expected = RoaringLandmask.new().contains_many(lon, lat)
    path = output / 'reference.csv'
    with path.open('w') as handle:
        handle.write('lon,lat,land\n')
        for x, y, value in zip(lon, lat, expected):
            handle.write(f'{x},{y},{int(value)}\n')
    subprocess.run(['cargo', 'run', '--offline', '-p', 'proteus-core', '--example',
                    'check_landmask', '--', str(output), str(path)], cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
