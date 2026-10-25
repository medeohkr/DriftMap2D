#!/usr/bin/env python3
"""
Daily forecast update: download ECMWF once, then SMOC day by day,
tile immediately, upload to R2.

Temporal interpolation support:
  - Full day tiles contain 25 SMOC samples with a midnight overlap.
    The final forecast tile may contain fewer hourly samples.
  - Each day tile contains 5 wind steps (step N*4 .. N*4+4),
    overlapping the next day by 1 step.
"""
import os
import io
import json
import math
import sys
import struct
import shutil
import numpy as np
import zstandard as zstd
import xarray as xr
import boto3
from pathlib import Path
from datetime import datetime, timedelta
from concurrent.futures import ThreadPoolExecutor, as_completed

# ===== CONFIG =====

if sys.platform == "linux":
    BASE_DIR = Path("./data")
else:
    BASE_DIR = Path("D:/projects/driftmap2d/data")

BUCKET = "driftmap-tiles"
HINDCAST_DAYS = 7
SMOC_DATASET = "cmems_mod_glo_phy_anfc_merged-uv_PT1H-i"
ECMWF_FORECAST_HOURS = 360

TILE_SIZE = 10.0
N_LON_TILES = 36
N_LAT_TILES = 17

# --- Temporal config ---
SMOC_HOURS_PER_DAY = 25      # 25 hours per day tile (overlap by 1)
WIND_STEPS_PER_DAY = 5       # 5 wind steps per day tile (overlap by 1)
WIND_STEP_HOURS = 6          # ECMWF step interval in hours
WIND_STEPS_PER_SMOC_DAY = 4  # 24h / 6h = 4 new steps per day

NC_DIR = BASE_DIR / "smoc_nc"
WIND_DIR = BASE_DIR / "ecmwf"
TILES_DIR = BASE_DIR / "forecast_tiles"

# R2 credentials
ACCOUNT_ID = os.environ.get("ACCOUNT_ID", "").strip()
ACCESS_KEY = os.environ.get("ACCESS_KEY", "").strip()
SECRET_KEY = os.environ.get("SECRET_KEY", "").strip()
CMEMS_USER = os.environ.get("CMEMS_USER", "").strip()
CMEMS_PASS = os.environ.get("CMEMS_PASS", "").strip()

if not all([ACCOUNT_ID, ACCESS_KEY, SECRET_KEY]):
    print("❌ ERROR: Missing R2 credentials")
    sys.exit(1)

s3 = boto3.client(
    's3',
    endpoint_url=f'https://{ACCOUNT_ID}.r2.cloudflarestorage.com',
    aws_access_key_id=ACCESS_KEY,
    aws_secret_access_key=SECRET_KEY,
    region_name='auto',
)


def delete_prefix(prefix):
    deleted = 0
    paginator = s3.get_paginator('list_objects_v2')
    for page in paginator.paginate(Bucket=BUCKET, Prefix=prefix):
        if 'Contents' in page:
            objects = [{'Key': obj['Key']} for obj in page['Contents']]
            s3.delete_objects(Bucket=BUCKET, Delete={'Objects': objects})
            deleted += len(objects)
    return deleted


def delete_oldest_day():
    cutoff = datetime.utcnow().date() - timedelta(days=HINDCAST_DAYS)
    expired = set()
    paginator = s3.get_paginator('list_objects_v2')
    for page in paginator.paginate(Bucket=BUCKET, Prefix='tiles/'):
        for obj in page.get('Contents', []):
            parts = obj['Key'].split('/')
            if len(parts) < 5:
                continue
            try:
                day = datetime.strptime('/'.join(parts[1:4]), '%Y/%m/%d').date()
            except ValueError:
                continue
            if day < cutoff:
                expired.add(day)
    for oldest in sorted(expired):
        prefix = f"tiles/{oldest.strftime('%Y/%m/%d')}/"
        deleted = delete_prefix(prefix)
        if deleted > 0:
            print(f"  Deleted expired day {oldest}: {deleted} tiles")


def smoc_timeline():
    """read source timestamps without downloading the global current fields."""
    import copernicusmarine
    with copernicusmarine.open_dataset(dataset_id=SMOC_DATASET, variables=["utotal", "vtotal"],
                                      username=CMEMS_USER or ".", password=CMEMS_PASS or ".") as dataset:
        return dataset['time'].values.astype('datetime64[s]')


def ecmwf_steps(reference, last_current):
    """include the wind sample bracketing the current endpoint, up to the product horizon."""
    hours = (last_current - reference).total_seconds() / 3600
    last = min(ECMWF_FORECAST_HOURS, math.ceil(hours / WIND_STEP_HOURS) * WIND_STEP_HOURS)
    if last < WIND_STEP_HOURS:
        raise ValueError('insufficient common current and wind forecast coverage')
    return list(range(0, last + 1, WIND_STEP_HOURS))


def forecast_windows(times, reference, wind_end):
    """select every complete or partial day jointly covered by currents and wind."""
    times = np.asarray(times).astype('datetime64[s]')
    if times.ndim != 1 or len(times) < 2 or np.isnat(times).any():
        raise ValueError('invalid SMOC source timeline')
    selected = times[(times >= np.datetime64(reference, 's')) & (times <= np.datetime64(wind_end, 's'))]
    if len(selected) < 2 or selected[0] != np.datetime64(reference, 's'):
        raise ValueError('source does not cover the requested start day')
    if np.any(np.diff(selected) != np.timedelta64(1, 'h')):
        raise ValueError('missing or unordered hourly current samples')
    day = reference
    last = selected[-1]
    while np.datetime64(day, 's') < last:
        midnight = np.datetime64(day, 's')
        samples = selected[(selected >= midnight) & (selected <= midnight + np.timedelta64(1, 'D'))]
        current_sample_count(samples, day)
        yield day, samples
        day += timedelta(days=1)


def download_ecmwf(reference, last_current):
    """download six-hourly wind and temperature through the required forecast horizon."""
    from ecmwf.opendata import Client

    date_str = reference.strftime("%Y-%m-%d")

    WIND_DIR.mkdir(parents=True, exist_ok=True)
    out_file = str(WIND_DIR / f"ecmwf_{date_str.replace('-', '')}_00z.grib2")

    print(f"📡 Downloading ECMWF forecast (initialized {date_str})...")

    for source in ["ecmwf", "aws"]:
        try:
            client = Client(source=source)
            client.retrieve(
                date=date_str,
                time="00",
                step=ecmwf_steps(reference, last_current),
                param=["10u", "10v", "skt"],
                target=out_file,
                type="fc",
                levtype="sfc",
            )
            if Path(out_file).exists():
                print(f"  ✅ ECMWF: {Path(out_file).stat().st_size / 1e6:.1f} MB")
                return out_file
        except Exception as e:
            print(f"  {source} source failed: {e}")

    print("  ❌ Failed to get ECMWF data")
    return None


def download_smoc_day(date, end_dt):
    """download hourly currents through the selected full or partial day endpoint."""
    import copernicusmarine

    if CMEMS_USER and CMEMS_PASS:
        copernicusmarine.login(username=CMEMS_USER, password=CMEMS_PASS)
    else:
        copernicusmarine.login(username=".", password=".")

    NC_DIR.mkdir(parents=True, exist_ok=True)
    out_file = str(NC_DIR / f"smoc_{date.strftime('%Y%m%d')}.nc")

    start_dt = date.replace(hour=0, minute=0, second=0, microsecond=0)

    sample_count = int((end_dt - start_dt).total_seconds() / 3600) + 1
    print(f"    Downloading SMOC {date.date()} ({sample_count} samples)...")

    copernicusmarine.subset(
        dataset_id=SMOC_DATASET,
        variables=["utotal", "vtotal"],
        minimum_longitude=-180,
        maximum_longitude=179.9166717529297,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=start_dt.strftime("%Y-%m-%dT%H:%M:%S"),
        end_datetime=end_dt.strftime("%Y-%m-%dT%H:%M:%S"),
        minimum_depth=0.49402499198913574,
        maximum_depth=0.49402499198913574,
        output_filename=out_file,
        force_download=True,
    )

    if Path(out_file).exists() and Path(out_file).stat().st_size > 0:
        print(f"    ✅ SMOC: {Path(out_file).stat().st_size / 1e6:.1f} MB")
        return out_file
    else:
        print(f"    ❌ SMOC download failed")
        return None


def halo_indices(coords, lower, upper, periodic=False):
    """select a tile core with one neighboring source cell on each side"""
    size = len(coords)
    if periodic and np.isclose(coords[-1] - coords[0], 360.0, rtol=0, atol=1e-4):
        size -= 1
    core = np.flatnonzero((coords[:size] >= lower) & (coords[:size] < upper))
    if not len(core):
        return core, 0.0
    first, last = int(core[0]) - 1, int(core[-1]) + 1
    if periodic:
        indices = np.arange(first, last + 1) % size
        origin = float(coords[indices[0]]) - (360.0 if first < 0 else 0.0)
    else:
        indices = np.arange(max(0, first), min(size - 1, last) + 1)
        origin = float(coords[indices[0]])
    return indices, origin


def current_sample_count(times, date):
    """validate hourly current samples starting at the tile's midnight."""
    times = np.asarray(times).astype('datetime64[s]')
    if times.ndim != 1 or not 2 <= len(times) <= SMOC_HOURS_PER_DAY:
        raise ValueError('expected 2 to 25 hourly current samples')
    midnight = np.datetime64(date.strftime('%Y-%m-%d'), 's')
    expected = midnight + np.arange(len(times)) * np.timedelta64(1, 'h')
    if not np.array_equal(times, expected):
        raise ValueError('current samples must be hourly and start at midnight')
    return len(times)


def encode_tile_header(current_origin, wind_origin, n_lon, n_lat, n_hours):
    """encode padded origins and the actual hourly sample count."""
    if n_lon < 2 or n_lat < 2 or not 2 <= n_hours <= SMOC_HOURS_PER_DAY:
        raise ValueError('invalid global tile dimensions or sample count')
    return b'PRT3' + struct.pack('<ffffIIIIf', *current_origin, *wind_origin,
                                n_hours, n_lon, n_lat, 1, 0.0)


def tile_and_upload_day(date, wind_file, wind_data_cache, expected_times=None):
    """Tile a single day and upload immediately, then delete.

    Day tile contents:
      - SMOC: up to 25 hourly samples, beginning at midnight.
      - Wind: 5 6-hourly steps, starting at day_offset*4 within the
        global ECMWF timeline.

    Zstandard level 3 wraps the complete tile without changing its .bin URL.
    Decompressed tile binary layout version 3:
      [PRT3][float32 current_lon_origin][float32 current_lat_origin]
        [float32 wind_lon_origin][float32 wind_lat_origin][uint32 n_hours]
      Each grid includes a one-cell halo where source data is available.
      [uint32 n_lon][uint32 n_lat][uint32 n_levels][float32 level]
        n_hours * (n_lat*n_lon float16 u, n_lat*n_lon float16 v)
      [uint32 wind_nlon][uint32 wind_nlat][uint32 n_wind_steps]
        n_wind_steps * (wind_nlat*wind_nlon float16 u, ... v, ... sst)
    """

    nc_file = NC_DIR / f"smoc_{date.strftime('%Y%m%d')}.nc"
    if not nc_file.exists():
        raise FileNotFoundError(f'SMOC file not found: {nc_file}')

    # the final source day may end before the next midnight
    ds = xr.open_dataset(str(nc_file))
    lons = ds['longitude'].values
    lats = ds['latitude'].values

    u_day = ds['utotal'].isel(depth=0).values[:SMOC_HOURS_PER_DAY]
    v_day = ds['vtotal'].isel(depth=0).values[:SMOC_HOURS_PER_DAY]
    n_hours = current_sample_count(ds['time'].values[:SMOC_HOURS_PER_DAY], date)
    if expected_times is not None and not np.array_equal(ds['time'].values.astype('datetime64[s]'), expected_times):
        raise ValueError('downloaded currents do not match the planned forecast window')
    if u_day.shape != v_day.shape or u_day.shape[0] != n_hours:
        raise ValueError('current fields do not match the source timeline')
    if n_hours < SMOC_HOURS_PER_DAY:
        print(f"    Partial forecast day: {n_hours} samples, ending at hour {n_hours - 1}")

    # Wind step range for this day: start at day_offset*4, take 5 steps
    day_offset = (date - wind_data_cache['reference_date']).days
    wind_start_step = day_offset * WIND_STEPS_PER_SMOC_DAY
    wind_end_step = wind_start_step + WIND_STEPS_PER_DAY
    print(f"    Wind steps: {wind_start_step} to {wind_end_step - 1} "
          f"({WIND_STEPS_PER_DAY} steps), day offset: {day_offset}")

    u_wind_global = wind_data_cache['u_wind'][wind_start_step:wind_end_step]
    v_wind_global = wind_data_cache['v_wind'][wind_start_step:wind_end_step]
    sst_global = wind_data_cache['sst'][wind_start_step:wind_end_step]
    wind_lons = wind_data_cache['lons']
    wind_lats = wind_data_cache['lats']
    n_wind_steps = u_wind_global.shape[0]
    if n_wind_steps < 2:
        raise ValueError('at least two six-hourly wind samples are required')

    if n_wind_steps < WIND_STEPS_PER_DAY:
        print(f"    ⚠️ Expected {WIND_STEPS_PER_DAY} wind steps, got {n_wind_steps}")

    # FIX 1: Reverse latitude dimension to ascending order (-90 to 90)
    # ECMWF provides latitudes descending (90 to -90), but we need ascending
    wind_lats = wind_lats[::-1]  # Now -90 to 90
    u_wind_global = u_wind_global[:, ::-1, :]  # Reverse lat dimension for all steps
    v_wind_global = v_wind_global[:, ::-1, :]
    sst_global = sst_global[:, ::-1, :]

    print(f"    Normalized wind coords: lon[{wind_lons[0]:.1f} to {wind_lons[-1]:.1f}], "
          f"lat[{wind_lats[0]:.1f} to {wind_lats[-1]:.1f}]")

    day_dir = TILES_DIR / date.strftime("%Y/%m/%d")
    day_dir.mkdir(parents=True, exist_ok=True)
    tiles = 0
    failed = 0

    for tilex in range(N_LON_TILES):
        lon_min = -180 + TILE_SIZE * tilex
        lon_max = lon_min + TILE_SIZE
        lon_idx, current_lon_origin = halo_indices(lons, lon_min, lon_max, periodic=True)
        if len(lon_idx) == 0:
            continue

        for tiley in range(N_LAT_TILES):
            lat_min = -80 + TILE_SIZE * tiley
            lat_max = lat_min + TILE_SIZE
            lat_idx, current_lat_origin = halo_indices(lats, lat_min, lat_max)
            if len(lat_idx) == 0:
                continue

            # Extract wind for this tile
            wind_lon_indices, wind_lon_origin = halo_indices(wind_lons, lon_min, lon_max, periodic=True)
            wind_lat_indices, wind_lat_origin = halo_indices(wind_lats, lat_min, lat_max)

            if len(wind_lon_indices) == 0 or len(wind_lat_indices) == 0:
                print(f"      ⚠️ No wind data for tile ({tilex},{tiley})")
                wind_nlon = 0
                wind_nlat = 0
            else:
                wind_nlon = len(wind_lon_indices)
                wind_nlat = len(wind_lat_indices)

            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"

            try:
                with io.BytesIO() as f:
                    f.write(encode_tile_header(
                        (current_lon_origin, current_lat_origin),
                        (wind_lon_origin, wind_lat_origin), len(lon_idx), len(lat_idx), n_hours))

                    # current slices follow the sample count in the header
                    for h in range(n_hours):
                        u_tile = u_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        v_tile = v_day[h, :, :][np.ix_(lat_idx, lon_idx)]
                        f.write(u_tile.astype('<f2').tobytes())
                        f.write(v_tile.astype('<f2').tobytes())

                    # Wind header
                    f.write(struct.pack('<I', wind_nlon))
                    f.write(struct.pack('<I', wind_nlat))
                    f.write(struct.pack('<I', n_wind_steps))

                    if wind_nlon > 0 and wind_nlat > 0:
                        # Interleave wind and SST by step (u, v, sst for each step)
                        for h in range(n_wind_steps):
                            u_w = u_wind_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]
                            v_w = v_wind_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]
                            s = sst_global[h][np.ix_(wind_lat_indices, wind_lon_indices)]

                            f.write(u_w.astype('<f2').tobytes())
                            f.write(v_w.astype('<f2').tobytes())
                            f.write(s.astype('<f2').tobytes())

                    tile_path.write_bytes(zstd.ZstdCompressor(level=3).compress(f.getvalue()))

                tiles += 1
            except Exception as e:
                failed += 1
                print(f"      ⚠️ Tile ({tilex},{tiley}) failed: {e}")
                import traceback
                traceback.print_exc()
                if tile_path.exists():
                    tile_path.unlink()

    ds.close()
    if failed:
        raise RuntimeError(f'{failed} tiles failed to generate for {date.date()}')

    # Upload tiles for this day to R2
    print(f"    📤 Uploading {tiles} tiles to R2...")
    uploaded = 0
    for bin_file in day_dir.glob("*.bin"):
        key = f"tiles/{date.strftime('%Y/%m/%d')}/{bin_file.name}"
        try:
            s3.upload_file(str(bin_file), BUCKET, key)
            uploaded += 1
        except Exception as e:
            print(f"      ❌ Upload failed: {bin_file.name} - {e}")

    print(f"    ✅ Tiled and uploaded: {uploaded}/{tiles} tiles")
    if tiles == 0 or uploaded != tiles:
        raise RuntimeError(f'incomplete upload for {date.date()}: {uploaded}/{tiles} tiles')

    # Clean up local files
    shutil.rmtree(day_dir)
    if nc_file.exists():
        os.remove(nc_file)

    return uploaded


def main():
    print(f"\n{'='*60}")
    print(f"🌊 Daily tile update: {datetime.utcnow().strftime('%Y-%m-%d %H:%M UTC')}")
    print(f"{'='*60}")

    # Delete oldest day from R2
    print("\n🗑️  Step 1: Removing days outside rolling window...")
    delete_oldest_day()

    # Prepare directories
    TILES_DIR.mkdir(parents=True, exist_ok=True)
    NC_DIR.mkdir(parents=True, exist_ok=True)
    WIND_DIR.mkdir(parents=True, exist_ok=True)

    today = datetime.utcnow().replace(hour=0, minute=0, second=0, microsecond=0)
    reference = today - timedelta(days=1)
    times = smoc_timeline()
    if len(times) < 2 or np.isnat(times).any():
        raise ValueError('invalid SMOC source timeline')
    last_current = datetime.utcfromtimestamp(int(times.max().astype('int64')))

    # Step 2: Download ECMWF once
    print("\n📡 Step 2: Downloading ECMWF forecast...")
    wind_file = download_ecmwf(reference, last_current)
    if not wind_file:
        print("  ❌ Failed to get ECMWF data. Exiting.")
        sys.exit(1)

    # Step 3: Load wind data
    print("\n📂 Step 3: Loading ECMWF data into memory...")
    ds_wind = xr.open_dataset(wind_file, engine="cfgrib")
    u_wind_global = ds_wind['u10'].values
    v_wind_global = ds_wind['v10'].values
    sst_global = ds_wind['skt'].values
    wind_lons = ds_wind['longitude'].values
    wind_lats = ds_wind['latitude'].values
    wind_times = ds_wind['valid_time'].values.astype('datetime64[s]')
    expected_wind_times = np.datetime64(reference, 's') + np.arange(len(wind_times)) * np.timedelta64(WIND_STEP_HOURS, 'h')
    if len(wind_times) < 2 or not np.array_equal(wind_times, expected_wind_times):
        raise ValueError('wind samples must be six-hourly and start at the selected initialization')
    if u_wind_global.shape != v_wind_global.shape or u_wind_global.shape != sst_global.shape or u_wind_global.shape[0] != len(wind_times):
        raise ValueError('wind fields do not match the forecast timeline')

    wind_data_cache = {
        'u_wind': u_wind_global,
        'v_wind': v_wind_global,
        'sst': sst_global,
        'lons': wind_lons,
        'lats': wind_lats,
        'reference_date': reference,
    }
    ds_wind.close()
    print(f"  ✅ Wind data loaded: {u_wind_global.shape[0]} steps, {len(wind_lats)} lat, {len(wind_lons)} lon")

    # Step 4: Process each day
    wind_end = datetime.utcfromtimestamp(int(wind_times[-1].astype('int64')))
    windows = list(forecast_windows(times, reference, wind_end))

    total_tiles = 0
    print(f"\n📡 Step 4: Processing {len(windows)} days through {windows[-1][1][-1]} UTC...")

    for day_offset, (day_date, samples) in enumerate(windows):
        print(f"\n  Day {day_offset+1}/{len(windows)}: {day_date.date()}")

        nc_file = download_smoc_day(day_date, datetime.utcfromtimestamp(int(samples[-1].astype('int64'))))
        if not nc_file:
            raise RuntimeError(f'SMOC download failed for {day_date.date()}')
        tiles = tile_and_upload_day(day_date, wind_file, wind_data_cache, samples)
        total_tiles += tiles
        print(f"    Cumulative tiles: {total_tiles}")

    manifest = {
        'schema_version': 1,
        'generated_at': datetime.utcnow().isoformat(timespec='seconds') + 'Z',
        'hindcast_days': HINDCAST_DAYS,
        'days': [{'date': date.strftime('%Y-%m-%d'), 'timestamps': samples.astype('int64').tolist()}
                 for date, samples in windows],
    }
    manifest_path = TILES_DIR / 'manifest.json'
    manifest_path.write_text(json.dumps(manifest), encoding='utf-8')
    s3.upload_file(str(manifest_path), BUCKET, 'tiles/manifest.json',
                   ExtraArgs={'ContentType': 'application/json', 'CacheControl': 'no-cache'})

    # Clean up
    if wind_file and Path(wind_file).exists():
        os.remove(wind_file)
        print(f"\n🧹 Cleaned up wind file")

    print(f"\n✅ Complete: {total_tiles} tiles uploaded")


if __name__ == "__main__":
    main()
