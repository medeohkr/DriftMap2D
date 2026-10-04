#!/usr/bin/env python3
"""
Download 72h SMOC + ECMWF forecasts, then tile both into the
Proteus binary format. The same NetCDF/GRIB2 files are preserved
for OpenDrift cross-validation.

Tiling uses 25 hourly current steps and 5 6-hourly wind steps per day,
so consecutive days overlap by 1 hour / 1 step. That overlap is what
lets the reader temporally interpolate across day boundaries.
"""
import os
import sys
import struct
import numpy as np
import xarray as xr
from pathlib import Path
from datetime import datetime, timedelta
import copernicusmarine
from ecmwf.opendata import Client


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
SMOC_END = "2026-09-22T06:00:00"       # 72 hourly steps (0..72 inclusive)
ECMWF_DATE = "2026-09-19"
ECMWF_TIME = "00"
ECMWF_LAST_STEP = 78                    # 0..78 every 6h -> 14 steps (indices 0..13)

# Per-day tile layout (must match proteus-wasm / proteus-core)
SMOC_HOURS_PER_DAY = 25                 # overlap by 1h across days
WIND_STEPS_PER_DAY = 5                  # overlap by 1 step across days
WIND_STEPS_PER_SMOC_DAY = 4             # 24h / 6h = 4 new steps per day

BASE_DIR = Path("D:/projects/driftmap/tests/opendrift_validation/data")
CURRENTS_DIR = BASE_DIR / "currents"
WIND_DIR = BASE_DIR / "wind"
TILES_DIR = BASE_DIR / "tiles"

SMOC_NC = CURRENTS_DIR / f"smoc_{SMOC_START[:10].replace('-', '')}_(1).nc"
ECMWF_GRIB = WIND_DIR / f"ecmwf_{ECMWF_DATE}_00z.grib2"


# ---------------------------------------------------------------------------
# Step 1: SMOC download
# ---------------------------------------------------------------------------
def download_smoc():
    CURRENTS_DIR.mkdir(parents=True, exist_ok=True)

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
    """Open both files and normalize coordinates to ascending latitude."""
    ds_cur = xr.open_dataset(str(smoc_path))
    ds_wind = xr.open_dataset(str(ecmwf_path), engine="cfgrib")

    u_day = ds_cur["utotal"].isel(depth=0).values  # (time, lat, lon)
    v_day = ds_cur["vtotal"].isel(depth=0).values
    cur_lons = ds_cur["longitude"].values
    cur_lats = ds_cur["latitude"].values

    u_wind = ds_wind["u10"].values                # (time, lat, lon)
    v_wind = ds_wind["v10"].values
    sst = ds_wind["skt"].values if "skt" in ds_wind else np.zeros_like(u_wind)
    wind_lons = ds_wind["longitude"].values
    wind_lats = ds_wind["latitude"].values

    # ECMWF latitudes are descending (90 -> -90). Flip to ascending.
    if wind_lats[0] > wind_lats[-1]:
        wind_lats = wind_lats[::-1]
        u_wind = u_wind[:, ::-1, :]
        v_wind = v_wind[:, ::-1, :]
        sst = sst[:, ::-1, :]

    n_hours = u_day.shape[0]
    n_wind_steps = u_wind.shape[0]

    print(f"  Currents: {n_hours} hourly steps, "
          f"{len(cur_lats)} lat × {len(cur_lons)} lon")
    print(f"  Wind:     {n_wind_steps} 6-hourly steps, "
          f"{len(wind_lats)} lat × {len(wind_lons)} lon")

    # Sanity: need at least FORECAST_DAYS * 24 + 1 hours of currents
    required_hours = FORECAST_DAYS * 24 + 1
    if n_hours < required_hours:
        print(f"  ⚠️ Only {n_hours} current hours available, "
              f"need {required_hours} for {FORECAST_DAYS} days with overlap")

    required_wind_steps = (FORECAST_DAYS - 1) * WIND_STEPS_PER_SMOC_DAY + WIND_STEPS_PER_DAY
    if n_wind_steps < required_wind_steps:
        print(f"  ⚠️ Only {n_wind_steps} wind steps available, "
              f"need {required_wind_steps} for {FORECAST_DAYS} days with overlap")

    return {
        "u_day": u_day, "v_day": v_day,
        "cur_lons": cur_lons, "cur_lats": cur_lats,
        "u_wind": u_wind, "v_wind": v_wind, "sst": sst,
        "wind_lons": wind_lons, "wind_lats": wind_lats,
        "n_hours": n_hours, "n_wind_steps": n_wind_steps,
    }


def tile_day(date: datetime, forcing: dict, day_offset: int) -> int:
    """Tile one day of forcing with overlapping boundaries.

    Currents:  25 hourly steps, starting at day_offset * 24
               -> hours [d*24 .. d*24+24] inclusive
    Wind:      5 six-hourly steps, starting at day_offset * 4
               -> steps [d*4 .. d*4+4] inclusive
    """
    day_dir = TILES_DIR / date.strftime("%Y/%m/%d")
    day_dir.mkdir(parents=True, exist_ok=True)

    u_day = forcing["u_day"]
    v_day = forcing["v_day"]
    cur_lons = forcing["cur_lons"]
    cur_lats = forcing["cur_lats"]

    u_wind = forcing["u_wind"]
    v_wind = forcing["v_wind"]
    sst = forcing["sst"]
    wind_lons = forcing["wind_lons"]
    wind_lats = forcing["wind_lats"]

    # --- Currents slice: 25 hours, starting at day_offset * 24 ---
    h_start = day_offset * 24
    h_end = h_start + SMOC_HOURS_PER_DAY          # +25, inclusive of boundary
    h_end = min(h_end, u_day.shape[0])
    n_hours = h_end - h_start

    if n_hours < SMOC_HOURS_PER_DAY:
        print(f"    ⚠️ Day {day_offset}: only {n_hours} current hours "
              f"(expected {SMOC_HOURS_PER_DAY})")

    u_day_slice = u_day[h_start:h_end]
    v_day_slice = v_day[h_start:h_end]

    # --- Wind slice: 5 steps, starting at day_offset * 4 ---
    w_start = day_offset * WIND_STEPS_PER_SMOC_DAY
    w_end = w_start + WIND_STEPS_PER_DAY
    w_end = min(w_end, u_wind.shape[0])
    n_wind_steps_day = w_end - w_start

    if n_wind_steps_day < WIND_STEPS_PER_DAY:
        print(f"    ⚠️ Day {day_offset}: only {n_wind_steps_day} wind steps "
              f"(expected {WIND_STEPS_PER_DAY})")

    u_wind_slice = u_wind[w_start:w_end]
    v_wind_slice = v_wind[w_start:w_end]
    sst_slice = sst[w_start:w_end]

    tiles = 0
    for tilex in range(N_LON_TILES):
        lon_min = -180.0 + TILE_SIZE * tilex
        lon_max = lon_min + TILE_SIZE

        if tilex == N_LON_TILES - 1:
            lon_idx = np.where((cur_lons >= lon_min) & (cur_lons <= 180.0))[0]
        else:
            lon_idx = np.where((cur_lons >= lon_min) & (cur_lons < lon_max))[0]
        if len(lon_idx) == 0:
            continue

        for tiley in range(N_LAT_TILES):
            lat_min = -80.0 + TILE_SIZE * tiley
            lat_max = lat_min + TILE_SIZE
            lat_idx = np.where((cur_lats >= lat_min) & (cur_lats < lat_max))[0]
            if len(lat_idx) == 0:
                continue

            wind_lon_idx = np.where(
                (wind_lons >= lon_min) & (wind_lons < lon_max)
            )[0]
            wind_lat_idx = np.where(
                (wind_lats >= lat_min) & (wind_lats < lat_max)
            )[0]

            wind_nlon = len(wind_lon_idx)
            wind_nlat = len(wind_lat_idx)

            tile_path = day_dir / f"{tilex:03d}_{tiley:03d}.bin"

            try:
                with open(tile_path, "wb") as f:
                    # ----- Currents header -----
                    f.write(struct.pack("<I", len(lon_idx)))
                    f.write(struct.pack("<I", len(lat_idx)))
                    f.write(struct.pack("<I", 1))       # n_depths
                    f.write(struct.pack("<f", 0.0))     # depth

                    # ----- Currents: SMOC_HOURS_PER_DAY hours -----
                    for h in range(n_hours):
                        u_tile = u_day_slice[h][np.ix_(lat_idx, lon_idx)]
                        v_tile = v_day_slice[h][np.ix_(lat_idx, lon_idx)]
                        u_tile = np.nan_to_num(u_tile, nan=0.0).astype(np.float16)
                        v_tile = np.nan_to_num(v_tile, nan=0.0).astype(np.float16)
                        u_tile.tofile(f)
                        v_tile.tofile(f)

                    # ----- Wind header -----
                    f.write(struct.pack("<I", wind_nlon))
                    f.write(struct.pack("<I", wind_nlat))
                    f.write(struct.pack("<I", n_wind_steps_day))

                    # ----- Wind + SST -----
                    if wind_nlon > 0 and wind_nlat > 0:
                        for h in range(n_wind_steps_day):
                            u_w = u_wind_slice[h][np.ix_(wind_lat_idx, wind_lon_idx)]
                            v_w = v_wind_slice[h][np.ix_(wind_lat_idx, wind_lon_idx)]
                            s = sst_slice[h][np.ix_(wind_lat_idx, wind_lon_idx)]

                            u_w = np.nan_to_num(u_w, nan=0.0).astype(np.float16)
                            v_w = np.nan_to_num(v_w, nan=0.0).astype(np.float16)
                            s = np.nan_to_num(s, nan=273.15).astype(np.float16)

                            u_w.tofile(f)
                            v_w.tofile(f)
                            s.tofile(f)

                tiles += 1
            except Exception as e:
                print(f"      ⚠️ Tile ({tilex},{tiley}) failed: {e}")
                if tile_path.exists():
                    tile_path.unlink()

    return tiles


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    print("📡 Step 1: Downloading SMOC (72h hourly currents)...")
    # smoc_path = download_smoc()
    # if not smoc_path:
    #     sys.exit(1)

    # print("\n📡 Step 2: Downloading ECMWF (72h 6-hourly wind + SST)...")
    # ecmwf_path = download_ecmwf()
    # if not ecmwf_path:
    #     sys.exit(1)

    forcing = load_forcing(SMOC_NC, ECMWF_GRIB)

    start_date = datetime.strptime(SMOC_START[:10], "%Y-%m-%d")

    total = 0
    for day_offset in range(FORECAST_DAYS):
        day_date = start_date + timedelta(days=day_offset)
        print(f"\nTiling day {day_offset}: {day_date.date()}")
        n = tile_day(day_date, forcing, day_offset)
        print(f"  {n} tiles written")
        total += n

    print(f"\n✅ Done: {total} tiles across {FORECAST_DAYS} days")


if __name__ == "__main__":
    main()