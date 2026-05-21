#!/usr/bin/env python3
"""Debug script: generate just tile 035_000 and inspect it."""

import sys
import struct
import numpy as np
import xarray as xr
from pathlib import Path
from datetime import datetime, timedelta

# Paths
NC_DIR = Path("./data/smoc_nc")
WIND_DIR = Path("./data/ecmwf")
OUTPUT_DIR = Path("./debug_tiles")

# Use your existing downloaded files
nc_file = NC_DIR / "smoc_20260520_(1).nc"  # Adjust date as needed
wind_file = WIND_DIR / "ecmwf_20260520_00z.grib2"  # Adjust as needed

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

# Load SMOC
print("Loading SMOC...")
ds = xr.open_dataset(str(nc_file))
lons = ds['longitude'].values
lats = ds['latitude'].values
print(f"  SMOC lons: {lons.shape}, range [{lons[0]:.6f}, {lons[-1]:.6f}]")
print(f"  SMOC lats: {lats.shape}, range [{lats[0]:.6f}, {lats[-1]:.6f}]")

# Load wind
print("Loading ECMWF...")
ds_wind = xr.open_dataset(wind_file, engine="cfgrib")
u_wind_global = ds_wind['u10'].values
v_wind_global = ds_wind['v10'].values
sst_global = ds_wind['skt'].values if 'skt' in ds_wind else np.zeros_like(u_wind_global)
wind_lons = ds_wind['longitude'].values
wind_lats = ds_wind['latitude'].values

# Reverse wind latitudes
wind_lats = wind_lats[::-1]
u_wind_global = u_wind_global[:, ::-1, :]
v_wind_global = v_wind_global[:, ::-1, :]
sst_global = sst_global[:, ::-1, :]

# Target tile
TILE_SIZE = 10.0
tilex = 35
tiley = 0

lon_min = -180 + TILE_SIZE * tilex
lon_max = lon_min + TILE_SIZE
lat_min = -80 + TILE_SIZE * tiley
lat_max = lat_min + TILE_SIZE

print(f"\nTile 035_000:")
print(f"  lon: [{lon_min}, {lon_max})")
print(f"  lat: [{lat_min}, {lat_max})")

# ---- DEBUG: Longitude mask ----
print(f"\n--- Longitude Debug ---")
print(f"All SMOC lons >= {lon_min}: {np.sum(lons >= lon_min)}")
print(f"All SMOC lons < {lon_max}: {np.sum(lons < lon_max)}")
print(f"All SMOC lons <= {lon_max}: {np.sum(lons <= lon_max)}")

mask_strict = (lons >= lon_min) & (lons < lon_max)
mask_with_eq = (lons >= lon_min) & (lons <= lon_max)

print(f"Strict mask (< {lon_max}): {np.sum(mask_strict)} cells")
print(f"With <= {lon_max}: {np.sum(mask_with_eq)} cells")

# Show the boundary cells
lons_near_boundary = lons[lons >= 179.0]
print(f"\nSMOC lons near boundary (>= 179.0):")
for i, lon in enumerate(lons_near_boundary):
    print(f"  index {np.where(lons == lon)[0][0]:4d}: {lon:.10f}")

# Apply the "last tile" fix
if tilex == 35:
    lon_idx = np.where((lons >= lon_min) & (lons <= 180.0))[0]
    print(f"\nUsing <= 180.0 mask: {len(lon_idx)} cells")
else:
    lon_idx = np.where((lons >= lon_min) & (lons < lon_max))[0]

if len(lon_idx) > 0:
    print(f"lon_idx first 5: {lons[lon_idx[:5]]}")
    print(f"lon_idx last 5: {lons[lon_idx[-5:]]}")
    print(f"lon_idx step size: {lons[lon_idx[1]] - lons[lon_idx[0]]:.10f}")

# Latitude mask
lat_idx = np.where((lats >= lat_min) & (lats < lat_max))[0]
print(f"\nLat cells: {len(lat_idx)}")
print(f"lat_idx first 5: {lats[lat_idx[:5]]}")
print(f"lat_idx last 5: {lats[lat_idx[-5:]]}")

# ---- Generate tile ----
print(f"\n--- Generating tile ---")
n_hours = 24
u_day = ds['utotal'].isel(depth=0).values
v_day = ds['vtotal'].isel(depth=0).values

# Wind mask
wind_lon_mask = (wind_lons >= lon_min) & (wind_lons < lon_max)
wind_lat_mask = (wind_lats >= lat_min) & (wind_lats < lat_max)
wind_lon_idx = np.where(wind_lon_mask)[0]
wind_lat_idx = np.where(wind_lat_mask)[0]
print(f"Wind cells: lon={len(wind_lon_idx)}, lat={len(wind_lat_idx)}")

tile_path = OUTPUT_DIR / "035_000.bin"
with open(tile_path, 'wb') as f:
    # Header
    f.write(struct.pack('<I', len(lon_idx)))
    f.write(struct.pack('<I', len(lat_idx)))
    f.write(struct.pack('<I', 1))  # n_levels
    f.write(struct.pack('<f', 0.0))
    
    # Current data
    for h in range(n_hours):
        u_tile = u_day[h, :, :][np.ix_(lat_idx, lon_idx)]
        v_tile = v_day[h, :, :][np.ix_(lat_idx, lon_idx)]
        u_tile = np.nan_to_num(u_tile, nan=0.0).astype(np.float16)
        v_tile = np.nan_to_num(v_tile, nan=0.0).astype(np.float16)
        u_tile.tofile(f)
        v_tile.tofile(f)
    
    # Wind header
    f.write(struct.pack('<I', len(wind_lon_idx)))
    f.write(struct.pack('<I', len(wind_lat_idx)))
    f.write(struct.pack('<I', 4))
    
    # Wind + SST interleaved
    for h in range(4):
        u_w = u_wind_global[h][np.ix_(wind_lat_idx, wind_lon_idx)]
        v_w = v_wind_global[h][np.ix_(wind_lat_idx, wind_lon_idx)]
        s = sst_global[h][np.ix_(wind_lat_idx, wind_lon_idx)]
        u_w = np.nan_to_num(u_w, nan=0.0).astype(np.float16)
        v_w = np.nan_to_num(v_w, nan=0.0).astype(np.float16)
        s = np.nan_to_num(s, nan=273.15).astype(np.float16)
        u_w.tofile(f)
        v_w.tofile(f)
        s.tofile(f)

file_size = Path(tile_path).stat().st_size
print(f"\n--- Result ---")
print(f"Tile saved: {tile_path}")
print(f"File size: {file_size} bytes ({file_size / 1024 / 1024:.3f} MB)")

# Read back header
with open(tile_path, 'rb') as f:
    n_lon_check = struct.unpack('<I', f.read(4))[0]
    n_lat_check = struct.unpack('<I', f.read(4))[0]
print(f"Header check: n_lon={n_lon_check}, n_lat={n_lat_check}")

if n_lon_check == 120:
    print("✅ SUCCESS: n_lon=120 as expected!")
else:
    print(f"❌ FAIL: n_lon={n_lon_check}, expected 120")

ds.close()
ds_wind.close()