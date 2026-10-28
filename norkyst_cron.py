#!/usr/bin/env python3
"""norkyst_cron.py publishes surface forcing on the native projected grid."""

import argparse
import hashlib
import io
import json
import logging
import math
import os
import struct
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

import netCDF4
import numpy as np
import zstandard as zstd


SOURCE = 'https://thredds.met.no/thredds/dodsC/fou-hi/norkystv3_800m_m00_be'
ROOT = Path(__file__).resolve().parent
CORE = 120
HALO = 1
WIDTH = CORE + HALO * 2
MAX_READ_VALUES = 2_500_000
MAX_READ_SPLITS = 4
FIELDS = {
    'u': 'u_eastward',
    'v': 'v_northward',
    'wind_u': 'Uwind_eastward',
    'wind_v': 'Vwind_northward',
    'temperature_k': 'temperature',
}


def plain(value):
    """convert netCDF attributes to JSON values."""
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, np.generic):
        return value.item()
    return value


def forecast_reference_time(dataset):
    """decode the source cycle separately from its forecast valid times."""
    if 'forecast_reference_time' not in dataset.variables:
        raise ValueError('source has no forecast_reference_time')
    variable = dataset.variables['forecast_reference_time']
    values = np.ma.asarray(variable[...])
    if values.size != 1 or np.ma.getmaskarray(values).any() or not np.isfinite(values).all():
        raise ValueError('expected one finite forecast reference time')
    reference = netCDF4.num2date(
        float(values.item()), variable.units,
        calendar=getattr(variable, 'calendar', 'standard'),
        only_use_cftime_datetimes=False, only_use_python_datetimes=True,
    )
    return reference.replace(tzinfo=timezone.utc)


def open_fresh_source(source, expected_date, wait_seconds=7200, poll_seconds=900):
    """reopen stale aggregations until today's cycle appears or the wait expires."""
    deadline = time.monotonic() + wait_seconds
    while True:
        dataset = netCDF4.Dataset(source)
        try:
            reference = forecast_reference_time(dataset)
            logging.info('source forecast_reference_time=%s expected_date=%s',
                         reference.isoformat(), expected_date)
            if reference.date() == expected_date:
                return dataset
            if reference.date() > expected_date:
                raise ValueError(f'forecast reference {reference.isoformat()} is after expected date {expected_date}')
        except Exception:
            dataset.close()
            raise
        dataset.close()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError(f'NorKyst forecast is stale, reference={reference.isoformat()}, '
                               f'expected={expected_date}. no tiles published')
        delay = min(poll_seconds, remaining)
        logging.warning('forecast is stale, retrying in %.0f seconds', delay)
        time.sleep(delay)


def read(variable, index):
    """read bounded contiguous requests and preserve reordered time selections."""
    index = index if isinstance(index, tuple) else (index,)
    index += (slice(None),) * (variable.ndim - len(index))
    for axis, selection in enumerate(index):
        if isinstance(selection, (list, np.ndarray)):
            values = np.asarray(selection)
            if values.ndim != 1 or not len(values) or not np.issubdtype(values.dtype, np.integer):
                raise ValueError('expected nonempty integer source indices')
            # OPeNDAP receives slices rather than a disjoint or reversed index vector
            boundaries = np.r_[0, np.flatnonzero(np.diff(values) != 1) + 1, len(values)]
            parts = []
            for first, last in zip(boundaries[:-1], boundaries[1:]):
                subset = list(index)
                subset[axis] = slice(int(values[first]), int(values[last - 1]) + 1)
                parts.append(read(variable, tuple(subset)))
            output_axis = sum(not isinstance(s, (int, np.integer)) for s in index[:axis])
            return np.concatenate(parts, axis=output_axis)
    return read_block(variable, index)


def read_block(variable, index, splits=0):
    """retry a bounded request and bisect persistent failures without filling data."""
    lengths = [(axis, len(range(*selection.indices(variable.shape[axis]))))
               for axis, selection in enumerate(index) if isinstance(selection, slice)]
    candidates = [(axis, count) for axis, count in lengths if count > 1]

    def smaller_requests(next_splits):
        # prefer splitting time so each request touches fewer forecast records
        axis, count = next((pair for pair in candidates if pair[0] == 0), max(candidates, key=lambda p: p[1]))
        selection = index[axis]
        middle = range(*selection.indices(variable.shape[axis]))[count // 2]
        left, right = list(index), list(index)
        left[axis] = slice(selection.start, middle, selection.step)
        right[axis] = slice(middle, selection.stop, selection.step)
        output_axis = sum(isinstance(s, slice) for s in index[:axis])
        return np.concatenate([read_block(variable, tuple(left), next_splits),
                               read_block(variable, tuple(right), next_splits)], axis=output_axis)

    if math.prod(count for _, count in lengths) > MAX_READ_VALUES and candidates:
        return smaller_requests(splits)
    for attempt in range(4):
        try:
            return np.ma.asarray(variable[index], dtype=np.float32).filled(np.nan)
        except (OSError, RuntimeError) as error:
            name = getattr(variable, 'name', 'array')
            logging.warning('NetCDF read failed field=%s index=%s attempt=%s error=%s',
                            name, index, attempt + 1, error)
            if attempt == 3:
                if candidates and splits < MAX_READ_SPLITS:
                    logging.warning('retrying smaller NetCDF requests field=%s split=%s', name, splits + 1)
                    return smaller_requests(splits + 1)
                raise RuntimeError(f'NetCDF read failed for {name} at {index} after retries and smaller requests') from error
            time.sleep(2 ** attempt)


def describe(dataset):
    """validate native axes and record the actual source timeline."""
    axes = [read(dataset.variables[name], slice(None)).astype(np.float64) for name in ('X', 'Y')]
    for name, axis in zip(('X', 'Y'), axes):
        if len(axis) < 2 or not np.all(np.isfinite(axis)) or not np.allclose(np.diff(axis), 800, atol=0.1):
            raise ValueError(f'{name} must be an ascending regular 800 m axis')
    depth = read(dataset.variables['depth'], slice(None))
    if not np.all(np.isfinite(depth)) or np.any(depth < 0):
        raise ValueError('expected finite nonnegative depths')
    surface = int(np.argmin(depth))
    clock = dataset.variables['time']
    clock_values = np.ma.asarray(clock[:])
    if clock_values.ndim != 1 or np.ma.getmaskarray(clock_values).any() or not np.isfinite(clock_values).all():
        raise ValueError('expected a finite unmasked source timeline')
    dates = netCDF4.num2date(clock_values, clock.units, calendar=getattr(clock, 'calendar', 'standard'),
                            only_use_cftime_datetimes=False, only_use_python_datetimes=True)
    seconds = np.array([int(d.replace(tzinfo=timezone.utc).timestamp()) for d in dates], dtype=np.int64)
    if len(seconds) < 2:
        raise ValueError('expected at least two source timestamps')
    xdim, ydim = dataset.variables['X'].dimensions[0], dataset.variables['Y'].dimensions[0]
    tdim, zdim = clock.dimensions[0], dataset.variables['depth'].dimensions[0]
    for field, name in FIELDS.items():
        expected = (tdim, zdim, ydim, xdim) if field in ('u', 'v', 'temperature_k') else (tdim, ydim, xdim)
        if dataset.variables[name].dimensions != expected:
            raise ValueError(f'unexpected dimensions for {name}')
    if dataset.variables['temperature'].units.lower() not in ('celsius', 'degree_celsius', 'degrees_celsius', 'degc'):
        raise ValueError('expected source temperature in Celsius')
    projection = dataset.variables['projection_stere']
    metadata = {
        'format': 'NKT1', 'source': SOURCE, 'core_cells': CORE, 'halo_cells': HALO,
        'tile_shape': [WIDTH, WIDTH], 'grid_shape': [len(axes[1]), len(axes[0])],
        'grid_origin_m': [float(axes[0][0]), float(axes[1][0])], 'grid_spacing_m': [800, 800],
        'projection': {key: plain(projection.getncattr(key)) for key in projection.ncattrs()},
        'surface_depth_m': float(depth[surface]), 'vector_basis': 'east_north',
        'fields': list(FIELDS), 'field_units': ['m/s', 'm/s', 'm/s', 'm/s', 'K'],
        'dtype': '<f2', 'array_order': 'field,time,y,x', 'compression': 'zstd', 'compression_level': 3,
        'source_first_time': int(seconds.min()), 'source_last_time': int(seconds.max()),
        'attribution': 'Norwegian Meteorological Institute (MET Norway)',
        'license': 'https://spdx.org/licenses/CC-BY-4.0.html',
    }
    return metadata, seconds, surface


def ordered_timeline(seconds):
    """sort timestamps and retain the last source record for duplicate times."""
    seconds = np.asarray(seconds)
    if seconds.ndim != 1 or len(seconds) < 2 or not np.isfinite(seconds).all():
        raise ValueError('expected at least two finite source timestamps')
    ordered, reversed_indices = np.unique(seconds[::-1], return_index=True)
    source_indices = len(seconds) - 1 - reversed_indices
    if len(ordered) < 2:
        raise ValueError('expected at least two distinct source timestamps')
    backwards = int(np.sum(np.diff(seconds) < 0))
    duplicates = len(seconds) - len(ordered)
    if backwards or duplicates:
        logging.warning('ordering source timeline backwards=%s duplicates=%s', backwards, duplicates)
    return ordered, source_indices


def day_windows(seconds, today):
    """include seven prior UTC dates and every available forecast interval."""
    seconds, source_indices = ordered_timeline(seconds)
    first = datetime.combine(today - timedelta(days=7), datetime.min.time(), timezone.utc)
    if seconds[0] > int(first.timestamp()):
        raise ValueError('source does not cover the requested seven-day hindcast')
    last = datetime.fromtimestamp(int(seconds[-1]), timezone.utc).date()
    day = first.date()
    while day <= last:
        start = int(datetime.combine(day, datetime.min.time(), timezone.utc).timestamp())
        indices = np.flatnonzero((seconds >= start) & (seconds <= start + 86400))
        if len(indices) >= 2:
            if seconds[indices[0]] != start:
                raise ValueError(f'missing start of day {day}')
            if np.any(np.diff(seconds[indices]) != 3600):
                raise ValueError(f'missing hourly samples on {day}')
            if day < last and seconds[indices[-1]] != start + 86400:
                raise ValueError(f'missing end of day {day}')
            source = source_indices[indices]
            # contiguous days keep efficient slices, reordered records use explicit indices
            window = slice(int(source[0]), int(source[-1]) + 1) if np.all(np.diff(source) == 1) else source
            yield day, window, seconds[indices].tolist()
        elif day < last:
            raise ValueError(f'missing forcing day {day}')
        day += timedelta(days=1)


def tile_array(strip, row_start, col_start, grid_shape):
    """keep a fixed tile shape and mark space outside the source domain as missing."""
    height, width = grid_shape
    left, right = max(0, col_start - HALO), min(width, col_start + CORE + HALO)
    top = max(0, row_start - HALO)
    result = np.full((strip.shape[0], WIDTH, WIDTH), np.nan, dtype='<f2')
    yoff, xoff = top - (row_start - HALO), left - (col_start - HALO)
    result[:, yoff:yoff + strip.shape[1], xoff:xoff + right - left] = strip[:, :, left:right]
    return result


def encode_tile(header, arrays):
    """wrap a JSON header and field-major float16 arrays in a zstd frame."""
    encoded = json.dumps(header, separators=(',', ':'), allow_nan=False).encode()
    buffer = io.BytesIO()
    buffer.write(b'NKT1')
    buffer.write(struct.pack('<I', len(encoded)))
    buffer.write(encoded)
    for array in arrays:
        buffer.write(array.astype('<f2', copy=False).tobytes(order='C'))
    return zstd.ZstdCompressor(level=3).compress(buffer.getvalue())


def r2_client():
    """use the existing R2 credentials without exposing their values."""
    import boto3
    names = ('ACCOUNT_ID', 'ACCESS_KEY', 'SECRET_KEY')
    if not all(os.environ.get(name) for name in names):
        raise ValueError('missing R2 credentials')
    return boto3.client('s3', endpoint_url=f'https://{os.environ["ACCOUNT_ID"]}.r2.cloudflarestorage.com',
                        aws_access_key_id=os.environ['ACCESS_KEY'],
                        aws_secret_access_key=os.environ['SECRET_KEY'], region_name='auto')


def write_json(path, value):
    """write strict JSON metadata."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, allow_nan=False), encoding='utf-8')


def coverage_rows(mask):
    """encode true cells as sorted half-open column intervals for each row."""
    rows = []
    for row in mask:
        edges = np.flatnonzero(np.diff(np.r_[False, row, False].astype(np.int8)))
        rows.append(edges.reshape(-1, 2).tolist())
    return rows


def coverage_mask(rows, shape):
    """restore a saved mask without accepting malformed intervals."""
    if len(rows) != shape[0]:
        raise ValueError('invalid coverage row count')
    mask = np.zeros(shape, dtype=bool)
    for y, intervals in enumerate(rows):
        previous = 0
        for left, right in intervals:
            if not all(type(v) is int for v in (left, right)) or not previous <= left < right <= shape[1]:
                raise ValueError('invalid coverage interval')
            mask[y, left:right] = True
            previous = right
    return mask


def read_current_coverage(dataset, metadata, surface, window):
    """collect observed currents across all hours of a retained hindcast day."""
    height, width = metadata['grid_shape']
    mask = np.zeros((height, width), dtype=bool)
    for row in range(0, height, CORE):
        index = (window, surface, slice(row, min(height, row + CORE)), slice(None))
        u = read(dataset.variables[FIELDS['u']], index)
        v = read(dataset.variables[FIELDS['v']], index)
        mask[row:row + CORE] = np.any(np.isfinite(u) & np.isfinite(v), axis=0)
    return mask


def make_coverage(metadata, observed, landmask=None):
    """precompute interpolation coverage and unsupported ocean cells on the native grid."""
    from pyproj import CRS, Transformer
    if not observed.any():
        raise ValueError('no finite currents available to establish NorKyst coverage')
    # allow one coastal expansion without treating deeper missing regions as coverage
    padded = np.pad(observed, 1)
    expanded = np.zeros_like(observed)
    height, width = observed.shape
    for dy in range(3):
        for dx in range(3):
            expanded |= padded[dy:dy + height, dx:dx + width]
    valid = expanded[:-1, :-1] & expanded[1:, :-1] & expanded[:-1, 1:] & expanded[1:, 1:]
    if landmask is None:
        from roaring_landmask import RoaringLandmask
        landmask = RoaringLandmask.new()
    transform = Transformer.from_crs(CRS.from_cf(metadata['projection']), 'EPSG:4326', always_xy=True)
    water = np.zeros_like(valid)
    for row in range(0, height - 1, CORE):
        yy, xx = np.nonzero(~valid[row:row + CORE])
        x = metadata['grid_origin_m'][0] + (xx + 0.5) * metadata['grid_spacing_m'][0]
        y = metadata['grid_origin_m'][1] + (yy + row + 0.5) * metadata['grid_spacing_m'][1]
        lon, lat = transform.transform(x, y)
        if len(xx):
            water[yy + row, xx] = ~np.asarray(landmask.contains_many(lon, lat), dtype=bool)
    return dict(schema_version=1, grid={key: metadata[key] for key in
        ('grid_shape', 'grid_origin_m', 'grid_spacing_m', 'projection')}, buffer_cells=1,
        valid_rows=coverage_rows(valid), unavailable_water_rows=coverage_rows(water))


def generate_day(dataset, metadata, surface, window, timestamps, directory, publish):
    """fetch one row of tiles at a time to bound memory and remote requests."""
    height, width = metadata['grid_shape']
    observed = np.zeros((height, width), dtype=bool)
    tiles = []
    for row in range(0, height, CORE):
        strips = []
        y = slice(max(0, row - HALO), min(height, row + CORE + HALO))
        for field, name in FIELDS.items():
            variable = dataset.variables[name]
            index = (window, surface, y, slice(None)) if variable.ndim == 4 else (window, y, slice(None))
            strip = read(variable, index)
            if field == 'temperature_k':
                strip += 273.15
            strips.append(strip)
        observed[y] |= np.any(np.isfinite(strips[0]) & np.isfinite(strips[1]), axis=0)
        for col in range(0, width, CORE):
            name = f'{col // CORE:03}_{row // CORE:03}.bin'
            header = {
                'tile_index': [col // CORE, row // CORE], 'shape': [len(timestamps), WIDTH, WIDTH],
                'core_shape': [min(CORE, height - row), min(CORE, width - col)],
                'origin_m': [metadata['grid_origin_m'][0] + (col - HALO) * 800,
                             metadata['grid_origin_m'][1] + (row - HALO) * 800],
                'timestamps': timestamps, 'fields': list(FIELDS),
            }
            payload = encode_tile(header, [tile_array(s, row, col, (height, width)) for s in strips])
            path = directory / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
            publish(path, 'application/zstd')
            tiles.append(name)
            # hosted runs retain only the current tile on disk
            if publish.remote:
                path.unlink()
        logging.info('completed tile row %s of %s', row // CORE + 1, math.ceil(height / CORE))
    return dict(timestamps=timestamps, tiles=tiles, coverage_rows=coverage_rows(observed))


def cleanup(client, bucket, prefix, cutoff):
    """remove only dated NorKyst objects outside the hindcast window."""
    for page in client.get_paginator('list_objects_v2').paginate(Bucket=bucket, Prefix=prefix + '/'):
        expired = []
        for obj in page.get('Contents', []):
            relative = obj['Key'][len(prefix) + 1:]
            try:
                day = datetime.strptime('/'.join(relative.split('/')[:3]), '%Y/%m/%d').date()
            except ValueError:
                continue
            if day < cutoff:
                expired.append({'Key': obj['Key']})
        if expired:
            response = client.delete_objects(Bucket=bucket, Delete={'Objects': expired})
            if response.get('Errors'):
                raise RuntimeError('R2 failed to delete some expired objects')


def main():
    """generate forcing locally or publish complete days to the NorKyst prefix."""
    from scripts.forecast_updates import published_manifest
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', default=SOURCE)
    parser.add_argument('--output', type=Path, default=ROOT / 'data' / 'norkyst')
    parser.add_argument('--local-only', action='store_true')
    parser.add_argument('--inspect', action='store_true')
    parser.add_argument('--refresh-hindcast', action='store_true')
    parser.add_argument('--check-for-update', action='store_true',
                        help='check once and skip stale or already published source cycles')
    parser.add_argument('--forecast-wait-minutes', type=int, default=120,
                        help="wait for today's source cycle before generation, default 120 minutes")
    args = parser.parse_args()
    if args.forecast_wait_minutes < 0:
        parser.error('forecast wait minutes must be nonnegative')
    logging.basicConfig(level=logging.INFO, format='%(asctime)s %(message)s')
    today = datetime.now(timezone.utc).date()
    opener = (netCDF4.Dataset(args.source) if args.inspect or args.check_for_update else
              open_fresh_source(args.source, today, args.forecast_wait_minutes * 60))
    with opener as dataset:
        reference = forecast_reference_time(dataset)
        client = None
        if args.check_for_update and not args.inspect:
            if reference.date() < today:
                logging.info('source cycle %s is stale, checking again next hour', reference.isoformat())
                return
            if reference.date() > today:
                raise ValueError('NorKyst forecast reference date is in the future')
            if not args.local_only:
                client = r2_client()
                previous = published_manifest(client, 'driftmap-tiles', 'norkyst/manifest.json')
                if (not args.refresh_hindcast and previous.get('grid', {}).get('source') == args.source
                        and previous.get('grid', {}).get('source_forecast_reference_time') == reference.isoformat()):
                    logging.info('source cycle %s is already published, no tiles changed', reference.isoformat())
                    return
        metadata, seconds, surface = describe(dataset)
        metadata['source'] = args.source
        metadata['source_forecast_reference_time'] = reference.isoformat()
        windows = list(day_windows(seconds, today))
        logging.info('grid=%s days=%s surface_depth=%s', metadata['grid_shape'], len(windows), metadata['surface_depth_m'])
        if args.inspect:
            print(json.dumps(metadata, indent=2))
            print([(str(day), len(times)) for day, _, times in windows])
            return
        bucket, prefix = 'driftmap-tiles', 'norkyst'
        if client is None and not args.local_only:
            client = r2_client()

        def publish(path, content_type):
            if client:
                key = prefix + '/' + path.relative_to(args.output).as_posix()
                client.upload_file(str(path), bucket, key,
                                   ExtraArgs={'ContentType': content_type, 'CacheControl': 'no-cache'})

        publish.remote = client is not None
        days = []
        observed = np.zeros(metadata['grid_shape'], dtype=bool)
        for day, window, times in windows:
            directory = args.output / day.strftime('%Y/%m/%d')
            key = f'{prefix}/{day:%Y/%m/%d}/manifest.json'
            previous = None
            if client and not args.refresh_hindcast and day < today - timedelta(days=1):
                from botocore.exceptions import ClientError
                try:
                    previous = json.loads(client.get_object(Bucket=bucket, Key=key)['Body'].read())
                except ClientError as error:
                    if error.response['Error']['Code'] not in ('NoSuchKey', '404'):
                        raise
            stable_grid = lambda grid: {k: v for k, v in grid.items()
                                        if k not in ('source_first_time', 'source_last_time',
                                                     'source_forecast_reference_time')}
            if previous and stable_grid(previous.get('grid', {})) == stable_grid(metadata) and previous.get('timestamps') == times:
                logging.info('retaining complete hindcast day %s', day)
                details = previous
                if 'coverage_rows' not in details:
                    details['coverage_rows'] = coverage_rows(read_current_coverage(dataset, metadata, surface, window))
                    manifest = directory / 'manifest.json'
                    write_json(manifest, details)
                    publish(manifest, 'application/json')
            else:
                logging.info('generating forcing day %s samples=%s', day, len(times))
                details = generate_day(dataset, metadata, surface, window, times, directory, publish)
                manifest = directory / 'manifest.json'
                write_json(manifest, dict(grid=metadata, **details))
                publish(manifest, 'application/json')
            observed |= coverage_mask(details['coverage_rows'], tuple(metadata['grid_shape']))
            days.append(dict(date=str(day), manifest=f'{day:%Y/%m/%d}/manifest.json', timestamps=times))
        coverage = make_coverage(metadata, observed)
        payload = json.dumps(coverage, separators=(',', ':'), allow_nan=False).encode()
        digest = hashlib.sha256(payload).hexdigest()
        coverage_path = args.output / f'coverage-{digest[:16]}.json'
        coverage_path.write_bytes(payload)
        publish(coverage_path, 'application/json')
        manifest = args.output / 'manifest.json'
        write_json(manifest, dict(grid=metadata, generated_at=datetime.now(timezone.utc).isoformat(), days=days,
                                 coverage=dict(path=coverage_path.name, sha256=digest)))
        publish(manifest, 'application/json')
        if client:
            cleanup(client, bucket, prefix, today - timedelta(days=7))


if __name__ == '__main__':
    main()
