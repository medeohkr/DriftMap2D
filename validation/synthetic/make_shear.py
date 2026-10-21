#!/usr/bin/env python3
"""Spatially varying synthetic forcing: u = U0 + SHEAR*lat, v = V0."""
from pathlib import Path
import numpy as np
from _synthetic import write_netcdf, write_tile, TILE_DATE

U0, SHEAR, V0 = 0.3, 0.02, 0.05
OUT = Path(__file__).resolve().parent / "shear"

def u_field(lats, lons):
    u = U0 + SHEAR * lats[:, None]
    return np.broadcast_to(u, (len(lats), len(lons))).copy()

def v_field(lats, lons):
    return np.full((len(lats), len(lons)), V0)

if __name__ == "__main__":
    write_netcdf(u_field, v_field, OUT / "input.nc")
    write_tile(u_field, v_field, OUT / "tiles" / f"{TILE_DATE[0]:04d}/{TILE_DATE[1]:02d}/{TILE_DATE[2]:02d}/000_000.bin")
    print(f"Wrote synthetic shear case to {OUT}")