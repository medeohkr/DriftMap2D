import json
import numpy as np

from .haversine import haversine_km, cumulative_path_length


def load_centroids(path):
    with open(path) as f:
        data = json.load(f)
    steps = np.array([r["step"] for r in data])
    lons = np.array([r["lon"] for r in data])
    lats = np.array([r["lat"] for r in data])
    order = np.argsort(steps)
    return steps[order], lons[order], lats[order]


def run_error_series(od_lon, od_lat, dm_lon, dm_lat):
    """Per-step error (percent of path length), absolute error (km), path length (km)."""
    n = min(len(od_lon), len(dm_lon))
    od_lon, od_lat = od_lon[:n], od_lat[:n]
    dm_lon, dm_lat = dm_lon[:n], dm_lat[:n]

    abs_err_km = haversine_km(dm_lon, dm_lat, od_lon, od_lat)
    path_km = cumulative_path_length(od_lon, od_lat)

    err_pct = np.full(n, np.nan)
    mask = np.isfinite(abs_err_km) & np.isfinite(path_km) & (path_km > 1e-6)
    err_pct[mask] = 100.0 * abs_err_km[mask] / path_km[mask]

    return err_pct, abs_err_km, path_km


def aggregate(per_run, max_len):
    padded = np.full((len(per_run), max_len), np.nan)
    for i, e in enumerate(per_run):
        padded[i, :len(e)] = e
    return {
        "padded": padded,
        "mean": np.nanmean(padded, axis=0),
        "std": np.nanstd(padded, axis=0),
        "p10": np.nanpercentile(padded, 10, axis=0),
        "p90": np.nanpercentile(padded, 90, axis=0),
    }


def write_csv(path, agg, max_len, value_col, extra_cols=None):
    with open(path, "w") as f:
        header = f"step,mean_{value_col},std_{value_col},p10_{value_col},p90_{value_col},n_runs"
        if extra_cols:
            header += "," + ",".join(extra_cols.keys())
        f.write(header + "\n")
        for i in range(max_len):
            n_here = int(np.sum(~np.isnan(agg["padded"][:, i])))
            row = (
                f"{i},{agg['mean'][i]:.6f},{agg['std'][i]:.6f},"
                f"{agg['p10'][i]:.6f},{agg['p90'][i]:.6f},{n_here}"
            )
            if extra_cols:
                for arr in extra_cols.values():
                    row += f",{arr[i]:.6f}"
            f.write(row + "\n")