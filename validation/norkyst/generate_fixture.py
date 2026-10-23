"""generate_fixture.py builds small native tile fixtures using the cron encoder."""

import importlib.util
import json
from pathlib import Path

import numpy as np
from pyproj import Proj


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('cron', ROOT / 'norkyst_cron.py')
cron = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cron)

destination = ROOT / 'crates/proteus-core/src/forcing/norkyst/fixtures'
destination.mkdir(parents=True, exist_ok=True)
projection = dict(grid_mapping_name='polar_stereographic', straight_vertical_longitude_from_pole=70,
                  latitude_of_projection_origin=90, false_northing=1844800, false_easting=3369600,
                  semi_major_axis=6378137, semi_minor_axis=6356752.3142, standard_parallel=60)
proj = Proj('+proj=stere +lat_0=90 +lat_ts=60 +lon_0=70 +x_0=3369600 +y_0=1844800 '
            '+a=6378137 +b=6356752.3142 +units=m')
timestamps = [1792713600, 1792717200, 1792720800]
grid = dict(format='NKT1', core_cells=120, halo_cells=1, tile_shape=[122, 122],
            grid_shape=[1148, 2747], grid_origin_m=[0, 0], grid_spacing_m=[800, 800],
            projection=projection, fields=list(cron.FIELDS), field_units=['m/s'] * 4 + ['K'],
            vector_basis='east_north', dtype='<f2', array_order='field,time,y,x', compression='zstd')
(destination / 'manifest.json').write_text(json.dumps(dict(grid=grid, days=[dict(date='2026-10-23',
    timestamps=timestamps)]), indent=2), encoding='utf-8')
for col in [480, 600]:
    row = 600
    x, y = np.meshgrid(np.arange(col - 1, col + 121), np.arange(row - 1, row + 121))
    arrays = [np.stack([1 + x * .001 + y * .002 + i * .5 for i in range(3)]).astype('<f2')]
    arrays += [np.full((3, 122, 122), value, dtype='<f2') for value in [-1, 0, 4]]
    arrays += [np.stack([np.full((122, 122), 280 + i) for i in range(3)]).astype('<f2')]
    # a missing source cell exercises the shared NaN expansion on a later query
    arrays[0][:, 61, 51] = np.nan
    header = dict(tile_index=[col // 120, row // 120], shape=[3, 122, 122], core_shape=[120, 120],
                  origin_m=[(col - 1) * 800, (row - 1) * 800], timestamps=timestamps, fields=list(cron.FIELDS))
    (destination / f'{col // 120:03}_005.bin').write_bytes(cron.encode_tile(header, arrays))
    for suffix, offset, count in [('full', 0, 25), ('next', 24, 3)]:
        expanded = [np.repeat(a[:1], count, axis=0) for a in arrays]
        for i in range(count):
            expanded[4][i] = 280 + offset + i
        extended_header = dict(header, shape=[count, 122, 122],
                               timestamps=[timestamps[0] + (offset + i) * 3600 for i in range(count)])
        (destination / f'{col // 120:03}_005_{suffix}.bin').write_bytes(cron.encode_tile(extended_header, expanded))
points = []
for x, y in [(599.75, 650.25), (600.25, 650.25), (590, 660)]:
    lon, lat = proj(x * 800, y * 800, inverse=True)
    points.append(dict(lon=lon, lat=lat, x=x, y=y))
(destination / 'reference.json').write_text(json.dumps(points, indent=2), encoding='utf-8')
