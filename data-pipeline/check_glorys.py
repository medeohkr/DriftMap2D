import xarray as xr
from pathlib import Path

input = Path("data/forecast/glorys_202505.nc")

ds = xr.open_dataset(input)
time = ds['time'].values
lon = ds['longitude'].values
print(ds.dims)
print(time)
print(lon)