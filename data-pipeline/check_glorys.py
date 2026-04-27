import xarray as xr
from pathlib import Path

input = Path("data\glorys_10yr_global\glorys_201001_depth_0.nc")

ds = xr.open_dataset(input)
depths = ds['depth'].values
print(ds.dims)
print(depths)