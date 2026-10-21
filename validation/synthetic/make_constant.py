#!/usr/bin/env python3
"""Constant synthetic forcing: u = U0, v = V0."""
from pathlib import Path
import numpy as np
from _synthetic import write_netcdf, write_tile, TILE_DATE

U0, V0 = 0.5, 0
OUT = Path(__file__).resolve().parent / "constant"

def u_field(lats, lons):
    return np.full((len(lats), len(lons)), U0)

def v_field(lats, lons):
    return np.full((len(lats), len(lons)), V0)

if __name__ == "__main__":
    write_netcdf(u_field, v_field, OUT / "input.nc")
    write_tile(u_field, v_field, OUT / "tiles" / f"{TILE_DATE[0]:04d}/{TILE_DATE[1]:02d}/{TILE_DATE[2]:02d}/000_000.bin")
    print(f"Wrote synthetic constant case to {OUT}")