#!/usr/bin/env python3
"""
Diagnose AVISO eddy atlas NetCDF files — check longitude coverage.
"""

import xarray as xr
import numpy as np
import matplotlib.pyplot as plt

# Your files
ANTICYC_FILE = "data/Eddy_trajectory_nrt_3.2exp_anticyclonic_20180101_20260422.nc"
CYCLONIC_FILE = "data/Eddy_trajectory_nrt_3.2exp_cyclonic_20180101_20260422.nc"

print("=" * 60)
print("Loading anticyclonic dataset...")
anticyc = xr.open_dataset(ANTICYC_FILE)
print(f"  Variables: {list(anticyc.variables)}")
print(f"  Dimensions: {dict(anticyc.dims)}")
print(f"  Time range: {anticyc.time.values[0]} to {anticyc.time.values[-1]}")

# Check longitude variable name
lon_var = None
for v in ['longitude', 'lon', 'longitude_eddy']:
    if v in anticyc:
        lon_var = v
        break

lat_var = None
for v in ['latitude', 'lat', 'latitude_eddy']:
    if v in anticyc:
        lat_var = v
        break

if lon_var is None:
    print("\nAvailable variables:", list(anticyc.variables))
    print("No longitude variable found — checking alternates...")
    anticyc.close()
    exit()

print(f"\n  Using: {lon_var}, {lat_var}")

# Get ALL longitudes from all observations
lons = anticyc[lon_var].values
lats = anticyc[lat_var].values

print(f"\n  Total observations: {len(lons):,}")
print(f"  Longitude range: {lons.min():.2f}° to {lons.max():.2f}°")
print(f"  Latitude range: {lats.min():.2f}° to {lats.max():.2f}°")

# Check if 0–360 or -180–180
print(f"\n  Min longitude: {lons.min():.2f}")
print(f"  Max longitude: {lons.max():.2f}")
if lons.min() < 0:
    print("  ✓ Convention: -180 to 180")
elif lons.max() > 180:
    print("  ✓ Convention: 0 to 360")
else:
    print("  ⚠️  Unknown convention")

# Count by hemisphere
if lons.min() < 0:
    western = (lons < 0).sum()
    eastern = (lons > 0).sum()
else:
    western = ((lons > 180) & (lons < 360)).sum()
    eastern = ((lons >= 0) & (lons <= 180)).sum()

print(f"\n  Western hemisphere obs: {western:,} ({100*western/len(lons):.1f}%)")
print(f"  Eastern hemisphere obs: {eastern:,} ({100*eastern/len(lons):.1f}%)")

# Sample a single day to check spatial distribution
import pandas as pd
times = pd.to_datetime(anticyc.time.values)
sample_date = times[0]
mask = times == sample_date
day_lons = lons[mask]
day_lats = lats[mask]

print(f"\n  Sample date: {sample_date}")
print(f"  Eddies that day: {len(day_lons)}")
if len(day_lons) > 0:
    # Convert to -180–180 for plotting if needed
    if day_lons.max() > 180:
        day_lons_plot = np.where(day_lons > 180, day_lons - 360, day_lons)
    else:
        day_lons_plot = day_lons
    
    print(f"    West of 0°: {(day_lons_plot < 0).sum()}")
    print(f"    East of 0°: {(day_lons_plot > 0).sum()}")

# Plot
fig, axes = plt.subplots(1, 2, figsize=(14, 6))

# All observations histogram
ax = axes[0]
if lons.max() > 180:
    lons_plot = np.where(lons > 180, lons - 360, lons)
    ax.set_xlabel("Longitude (-180 to 180)")
else:
    lons_plot = lons
    ax.set_xlabel("Longitude")

ax.hist(lons_plot, bins=360, color='steelblue', edgecolor='none')
ax.axvline(x=0, color='black', linestyle='--', linewidth=1)
ax.set_title(f"All observations ({len(lons):,} total)")
ax.set_ylabel("Count")
ax.grid(True, alpha=0.3)

# One day scatter plot
ax = axes[1]
if len(day_lons) > 0:
    ax.scatter(day_lons_plot, day_lats, s=1, alpha=0.5, color='red')
ax.set_xlim(-180, 180)
ax.set_ylim(-90, 90)
ax.set_xlabel("Longitude")
ax.set_ylabel("Latitude")
ax.set_title(f"Eddy locations — {sample_date.date()} ({len(day_lons)} eddies)")
ax.grid(True, alpha=0.3)

plt.tight_layout()
plt.savefig("eddy_diagnostic.png", dpi=150)
print("\n  Saved: eddy_diagnostic.png")

# Now check cyclonic too
print("\n" + "=" * 60)
print("Loading cyclonic dataset...")
cyclonic = xr.open_dataset(CYCLONIC_FILE)
lons_c = cyclonic[lon_var].values
lats_c = cyclonic[lat_var].values
print(f"  Total observations: {len(lons_c):,}")
print(f"  Longitude range: {lons_c.min():.2f}° to {lons_c.max():.2f}°")

if lons_c.min() < 0:
    western_c = (lons_c < 0).sum()
else:
    western_c = ((lons_c > 180) & (lons_c < 360)).sum()
print(f"  Western hemisphere: {western_c:,}")

anticyc.close()
cyclonic.close()