#!/usr/bin/env python3
"""
Download 72h SMOC + ECMWF forecasts, then tile both into the
Proteus binary format. The same NetCDF/GRIB2 files are preserved
for OpenDrift cross-validation. Spatial grids include a one-cell halo
on each side and preserve NaNs. Tiles use zstd level 3 compression.

Full tiles use 25 hourly current samples and 5 six-hourly wind samples.
The final day keeps the available hourly samples. Consecutive full days
overlap at midnight for temporal interpolation.
"""
import argparse
import io
import struct
import numpy as np
import xarray as xr
from pathlib import Path
from datetime import datetime, timedelta
import zstandard as zstd


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
TILE_SIZE = 10.0
N_LON_TILES = 36
N_LAT_TILES = 17

# 3 days of output + 1 extra hour of currents + 1 extra wind step
# so each day tile has 25h / 5 steps and consecutive days overlap.
FORECAST_DAYS = 3
SMOC_START = "2026-09-19T00:00:00"
SMOC_END = "2026-09-22T06:00:00"
ECMWF_DATE = "2026-09-19"
ECMWF_TIME = "00"
ECMWF_LAST_STEP = 78                    # 0..78 every 6h -> 14 steps (indices 0..13)

# Per-day tile layout (must match proteus-wasm / proteus-core)
SMOC_HOURS_PER_DAY = 25                 # overlap by 1h across days
WIND_STEPS_PER_DAY = 5                  # overlap by 1 step across days
WIND_STEPS_PER_SMOC_DAY = 4             # 24h / 6h = 4 new steps per day

BASE_DIR = Path(__file__).resolve().parents[2] / "validation/forcing"
CURRENTS_DIR = BASE_DIR / "currents"
WIND_DIR = BASE_DIR / "wind"
TILES_DIR = BASE_DIR / "tiles"

SMOC_NC = CURRENTS_DIR / f"smoc_{SMOC_START[:10].replace('-', '')}.nc"
ECMWF_GRIB = WIND_DIR / f"ecmwf_{ECMWF_DATE}_00z.grib2"


# ---------------------------------------------------------------------------
# Step 1: SMOC download
# ---------------------------------------------------------------------------
def download_smoc():
    CURRENTS_DIR.mkdir(parents=True, exist_ok=True)

    import copernicusmarine

    copernicusmarine.login()

    copernicusmarine.subset(
        dataset_id="cmems_mod_glo_phy_anfc_merged-uv_PT1H-i",
        variables=["utotal", "vtotal"],
        minimum_longitude=-180,
        maximum_longitude=179.9166717529297,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=SMOC_START,
        end_datetime=SMOC_END,
        minimum_depth=0.49402499198913574,
        maximum_depth=0.49402499198913574,
        output_filename=str(SMOC_NC),
        force_download=True,
    )

    if SMOC_NC.exists() and SMOC_NC.stat().st_size > 0:
        print(f"  ✅ SMOC: {SMOC_NC.stat().st_size / 1e6:.1f} MB")
        return SMOC_NC
    print("  ❌ SMOC download failed")
    return None


# ---------------------------------------------------------------------------
# Step 2: ECMWF download
# ---------------------------------------------------------------------------
def download_ecmwf():
    from ecmwf.opendata import Client

    WIND_DIR.mkdir(parents=True, exist_ok=True)

    for source in ["ecmwf", "aws"]:
        try:
            client = Client(source=source)
            client.retrieve(
                date=ECMWF_DATE,
                time=ECMWF_TIME,
                # Download one step beyond the last day's window:
                # day 2 needs wind indices 8..12 (48..72h). Index 12 = 72h.
                # To have a full 5-step window on day 2 we need index 12 to exist,
                # which it does with step=72. But we download to 78 so the
                # *next* test window (if extended) has its boundary step.
                step=list(range(0, ECMWF_LAST_STEP + 1, 6)),
                param=["10u", "10v", "skt"],
                target=str(ECMWF_GRIB),
                type="fc",
                levtype="sfc",
            )
            if ECMWF_GRIB.exists():
                print(f"  ✅ ECMWF: {ECMWF_GRIB.stat().st_size / 1e6:.1f} MB")
                return ECMWF_GRIB
        except Exception as e:
            print(f"  {source} source failed: {e}")

    print("  ❌ Failed to get ECMWF data")
    return None


# ---------------------------------------------------------------------------
# Step 3: Tile both files into Proteus binary format
# ---------------------------------------------------------------------------
def load_forcing(smoc_path: Path, ecmwf_path: Path):
    """keep source fields lazy and normalize both grids to ascending coordinates."""
    current = xr.open_dataset(smoc_path).sortby("latitude").sortby("longitude")
    wind = xr.open_dataset(ecmwf_path, **({"engine": "cfgrib"} if ecmwf_path.suffix == ".grib2" else {}))
    wind = wind.assign_coords(longitude=(wind.longitude + 180) % 360 - 180)
    wind = wind.sortby("latitude").sortby("longitude")
    if "skt" not in wind:
        raise ValueError("wind source must include skin temperature")
    return {
        "u_day": current.utotal.isel(depth=0), "v_day": current.vtotal.isel(depth=0),
        "cur_lons": current.longitude.values, "cur_lats": current.latitude.values,
        "u_wind": wind.u10, "v_wind": wind.v10, "sst": wind.skt,
        "wind_lons": wind.longitude.values, "wind_lats": wind.latitude.values,
    }


def halo_indices(coords, lower, upper, periodic=False):
    """include one source cell on either side, wrapping longitude at the dateline."""
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


def tile_day(date: datetime, forcing: dict, day_offset: int) -> int:
    """read each latitude strip once and encode padded daily surface tiles."""
    day_dir = TILES_DIR / date.strftime("%Y/%m/%d")
    day_dir.mkdir(parents=True, exist_ok=True)
    h_start = day_offset * 24
    h_end = min(h_start + SMOC_HOURS_PER_DAY, forcing["u_day"].shape[0])
    n_hours = h_end - h_start
    if n_hours < 2:
        raise ValueError("at least two hourly current samples are required")
    w_start = day_offset * WIND_STEPS_PER_SMOC_DAY
    w_end = min(w_start + WIND_STEPS_PER_DAY, forcing["u_wind"].shape[0])
    n_wind = w_end - w_start
    if n_wind < 2 or (n_wind - 1) * 6 < n_hours - 1:
        raise ValueError("wind source does not cover the daily current window")
    compressor = zstd.ZstdCompressor(level=3)
    tiles = 0
    for tiley in range(N_LAT_TILES):
        lower = -80.0 + TILE_SIZE * tiley
        lat_idx, lat_origin = halo_indices(forcing["cur_lats"], lower, lower + TILE_SIZE)
        wlat_idx, wlat_origin = halo_indices(forcing["wind_lats"], lower, lower + TILE_SIZE)
        if not len(lat_idx) or not len(wlat_idx):
            continue
        # retain only one latitude strip to bound memory and avoid repeated source reads
        currents = [forcing[k].isel(time=slice(h_start, h_end), latitude=lat_idx).values
                    for k in ("u_day", "v_day")]
        atmosphere = [forcing[k].isel({forcing[k].dims[0]: slice(w_start, w_end), "latitude": wlat_idx}).values
                      for k in ("u_wind", "v_wind", "sst")]
        for tilex in range(N_LON_TILES):
            lower_lon = -180.0 + TILE_SIZE * tilex
            lon_idx, lon_origin = halo_indices(forcing["cur_lons"], lower_lon, lower_lon + TILE_SIZE, True)
            wlon_idx, wlon_origin = halo_indices(forcing["wind_lons"], lower_lon, lower_lon + TILE_SIZE, True)
            if not len(lon_idx) or not len(wlon_idx):
                continue
            with io.BytesIO() as output:
                output.write(b"PRT3" + struct.pack("<ffffIIIIf", lon_origin, lat_origin,
                             wlon_origin, wlat_origin, n_hours, len(lon_idx), len(lat_idx), 1, 0.0))
                for h in range(n_hours):
                    for field in currents:
                        output.write(field[h][:, lon_idx].astype("<f2").tobytes())
                output.write(struct.pack("<III", len(wlon_idx), len(wlat_idx), n_wind))
                for h in range(n_wind):
                    for field in atmosphere:
                        output.write(field[h][:, wlon_idx].astype("<f2").tobytes())
                path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"
                path.write_bytes(compressor.compress(output.getvalue()))
            tiles += 1
        print(f"{date.date()} completed latitude strip {tiley + 1}/{N_LAT_TILES}", flush=True)
    return tiles


def main():
    """rebuild the saved validation forcing without fetching a different forecast."""
    global TILES_DIR
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--currents", type=Path, default=SMOC_NC)
    saved_wind = WIND_DIR / f"ecmwf_{ECMWF_DATE}_00z_oil.nc"
    parser.add_argument("--wind", type=Path, default=saved_wind if saved_wind.exists() else ECMWF_GRIB)
    parser.add_argument("--output", type=Path, default=TILES_DIR)
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    if args.download:
        if not download_smoc() or not download_ecmwf():
            raise SystemExit("forcing download failed")
        args.currents, args.wind = SMOC_NC, ECMWF_GRIB
    TILES_DIR = args.output.resolve()
    forcing = load_forcing(args.currents, args.wind)
    start_date = datetime.strptime(SMOC_START[:10], "%Y-%m-%d")
    total = 0
    for day_offset in range(FORECAST_DAYS):
        date = start_date + timedelta(days=day_offset)
        count = tile_day(date, forcing, day_offset)
        print(f"{date.date()} generated {count} padded tiles", flush=True)
        total += count
    print(f"generated {total} zstd level 3 tiles in {TILES_DIR}")


if __name__ == "__main__":
    main()
