import json
from pathlib import Path
import numpy as np
import xarray as xr


def prepare_ecmwf_netcdf(grib_path, nc_path):
    """Convert ECMWF GRIB2 to CF NetCDF (once)."""
    from opendrift.readers import add_standard_name_for_surface_grib_variables

    if Path(nc_path).exists():
        return
    ds = xr.open_dataset(grib_path, engine="cfgrib")
    ds = ds.drop_vars("time", errors="ignore")
    ds = ds.swap_dims({"step": "valid_time"}).rename({"valid_time": "time"})
    ds = ds.drop_vars(["step", "heightAboveGround", "surface"], errors="ignore")
    ds = add_standard_name_for_surface_grib_variables(ds)
    ds = ds.drop_vars("skt", errors="ignore")
    ds.to_netcdf(nc_path)


def save_centroids_from_nc(nc_path, json_path):
    """Read OpenDrift NetCDF output, write {step, lon, lat, time, n_active} JSON."""
    ds = xr.open_dataset(nc_path)
    lon_name = next((n for n in ("lon", "longitude") if n in ds), None)
    lat_name = next((n for n in ("lat", "latitude") if n in ds), None)
    time_name = next((n for n in ds.coords if "time" in n.lower()), "time")
    if lon_name is None or lat_name is None:
        ds.close()
        raise RuntimeError(f"no lon/lat variables in {nc_path}")

    lons = ds[lon_name].values
    lats = ds[lat_name].values
    times = ds[time_name].values
    ds.close()

    if lons.shape[0] == 1:
        lons = lons[0]
        lats = lats[0]
    if lons.ndim == 2 and lons.shape[0] > lons.shape[1]:
        lons = lons.T
        lats = lats.T

    out = []
    for i in range(lons.shape[0]):
        slon, slat = lons[i], lats[i]
        mask = ~np.isnan(slon) & ~np.isnan(slat)
        if mask.any():
            out.append({
                "step": i,
                "lon": float(np.mean(slon[mask])),
                "lat": float(np.mean(slat[mask])),
                "time": str(times[i]) if i < len(times) else None,
                "n_active": int(mask.sum()),
            })

    Path(json_path).parent.mkdir(parents=True, exist_ok=True)
    with open(json_path, "w") as f:
        json.dump(out, f, indent=2)