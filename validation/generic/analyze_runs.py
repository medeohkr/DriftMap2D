#!/usr/bin/env python3
"""
Load all runs from runs/, compute the per-step error between DriftMap
and OpenDrift centroids, and plot:

  - Top panel:    normalized error (% of cumulative path length)
  - Bottom panel: absolute error (km)

Normalization:
    error_pct[n] = 100 * haversine(dm[n], od[n]) / path_length(od)[n]

where path_length is the cumulative distance traveled by the OpenDrift
centroid, not the net displacement from release. This avoids inflation
for looping trajectories that return near their start.

Reads:
    runs/run_XXX/opendrift_centroids.json
    runs/run_XXX/proteus_centroids.json

Writes:
    error_vs_step.png
    error_vs_step_pct.csv
    error_vs_step_abs.csv
"""
import json
from pathlib import Path

import numpy as np
import matplotlib.pyplot as plt


RUNS_DIR = Path("runs")
OUT_PNG = Path("error_vs_step.png")
OUT_CSV_PCT = Path("error_vs_step_pct.csv")
OUT_CSV_ABS = Path("error_vs_step_abs.csv")


# ---------------------------------------------------------------------------
# Haversine
# ---------------------------------------------------------------------------
def haversine_km(lon1, lat1, lon2, lat2):
    R = 6371.0088
    lon1, lat1, lon2, lat2 = map(np.radians, [lon1, lat1, lon2, lat2])
    dlon = lon2 - lon1
    dlat = lat2 - lat1
    a = np.sin(dlat / 2.0) ** 2 + np.cos(lat1) * np.cos(lat2) * np.sin(dlon / 2.0) ** 2
    return R * 2.0 * np.arcsin(np.sqrt(a))


def cumulative_path_length(lon, lat):
    """Cumulative distance traveled up to each step. result[0] = 0."""
    n = len(lon)
    out = np.zeros(n)
    for i in range(1, n):
        out[i] = out[i - 1] + haversine_km(lon[i-1], lat[i-1], lon[i], lat[i])
    return out


# ---------------------------------------------------------------------------
# Load centroids from JSON
# ---------------------------------------------------------------------------
def load_centroids(path):
    with open(path) as f:
        data = json.load(f)
    steps = np.array([row["step"] for row in data])
    lons = np.array([row["lon"] for row in data])
    lats = np.array([row["lat"] for row in data])
    order = np.argsort(steps)
    return steps[order], lons[order], lats[order]


# ---------------------------------------------------------------------------
# Per-run error series
# ---------------------------------------------------------------------------
def run_error_series(od_lon, od_lat, dm_lon, dm_lat):
    n = min(len(od_lon), len(dm_lon))
    od_lon, od_lat = od_lon[:n], od_lat[:n]
    dm_lon, dm_lat = dm_lon[:n], dm_lat[:n]

    abs_err_km = haversine_km(dm_lon, dm_lat, od_lon, od_lat)
    path_km = cumulative_path_length(od_lon, od_lat)

    err_pct = np.full(n, np.nan)
    mask = np.isfinite(abs_err_km) & np.isfinite(path_km) & (path_km > 1e-6)
    err_pct[mask] = 100.0 * abs_err_km[mask] / path_km[mask]

    return err_pct, abs_err_km, path_km


# ---------------------------------------------------------------------------
# Aggregate + write CSV
# ---------------------------------------------------------------------------
def aggregate(per_run, max_len):
    padded = np.full((len(per_run), max_len), np.nan)
    for i, e in enumerate(per_run):
        padded[i, :len(e)] = e

    return {
        "padded": padded,
        "mean": np.nanmean(padded, axis=0),
        "std":  np.nanstd(padded, axis=0),
        "p10":  np.nanpercentile(padded, 10, axis=0),
        "p90":  np.nanpercentile(padded, 90, axis=0),
    }


def write_csv(path, agg, max_len, value_col, value_units, extra_cols=None):
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


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    run_dirs = sorted(p for p in RUNS_DIR.iterdir() if p.is_dir())
    print(f"Found {len(run_dirs)} runs in {RUNS_DIR}/")

    per_run_pct = []
    per_run_abs = []
    per_run_path = []

    for run_dir in run_dirs:
        od_path = run_dir / "opendrift_centroids.json"
        dm_path = run_dir / "proteus_centroids.json"

        if not od_path.exists() or not dm_path.exists():
            print(f"  {run_dir.name}: skipping (missing centroids)")
            continue

        try:
            _, od_lon, od_lat = load_centroids(od_path)
            _, dm_lon, dm_lat = load_centroids(dm_path)
        except Exception as e:
            print(f"  {run_dir.name}: parse error: {e}")
            continue

        err_pct, abs_km, path_km = run_error_series(od_lon, od_lat, dm_lon, dm_lat)

        per_run_pct.append(err_pct)
        per_run_abs.append(abs_km)
        per_run_path.append(path_km)

        print(
            f"  {run_dir.name}: {len(err_pct)} steps, "
            f"final path {path_km[-1]:.2f} km, "
            f"final abs err {abs_km[-1]:.3f} km, "
            f"final pct err {err_pct[-1]:.3f}%"
        )

    if not per_run_pct:
        print("No usable runs.")
        return

    max_len = max(len(e) for e in per_run_pct)

    agg_pct  = aggregate(per_run_pct,  max_len)
    agg_abs  = aggregate(per_run_abs,  max_len)
    agg_path = aggregate(per_run_path, max_len)

    # CSVs
    write_csv(
        OUT_CSV_PCT, agg_pct, max_len, "pct",
        "% of path length",
    )
    print(f"\nWrote {OUT_CSV_PCT}")

    write_csv(
        OUT_CSV_ABS, agg_abs, max_len, "km",
        "km",
        extra_cols={"mean_path_km": agg_path["mean"]},
    )
    print(f"Wrote {OUT_CSV_ABS}")

    # Plot
    steps = np.arange(max_len)

    fig, (ax_pct, ax_abs) = plt.subplots(
        2, 1, figsize=(11, 8), sharex=True,
        gridspec_kw={"height_ratios": [1, 1]},
    )

    # --- Top: normalized error ---
    ax_pct.plot(steps, agg_pct["mean"], color="C0", lw=2, label="mean")
    ax_pct.fill_between(
        steps, agg_pct["p10"], agg_pct["p90"],
        color="C0", alpha=0.15, label="10th–90th percentile",
    )
    ax_pct.set_ylabel("centroid error (% of path length)")
    ax_pct.set_title(
        f"DriftMap Generic Drift vs OpenDrift OceanDrift: centroid error across {len(per_run_pct)} runs"
    )
    ax_pct.grid(True, alpha=0.3)
    ax_pct.legend(loc="upper left")
    ax_pct.axhline(0, color="k", lw=0.5, alpha=0.3)

    # --- Bottom: absolute error ---
    ax_abs.plot(steps, agg_abs["mean"], color="C1", lw=2, label="mean")
    ax_abs.fill_between(
        steps, agg_abs["p10"], agg_abs["p90"],
        color="C1", alpha=0.15, label="10th–90th percentile",
    )
    ax_abs.set_xlabel("simulation step (hours since release)")
    ax_abs.set_ylabel("absolute centroid error (km)")
    ax_abs.grid(True, alpha=0.3)
    ax_abs.legend(loc="upper left")
    ax_abs.axhline(0, color="k", lw=0.5, alpha=0.3)

    plt.tight_layout()
    plt.savefig(OUT_PNG, dpi=150)
    print(f"Wrote {OUT_PNG}")

    # Summary
    final_pct = agg_pct["padded"][:, -1]
    final_pct = final_pct[~np.isnan(final_pct)]
    final_abs = agg_abs["padded"][:, -1]
    final_abs = final_abs[~np.isnan(final_abs)]

    print()
    print("Final-step summary")
    print("------------------")
    print(f"  runs contributing:      {len(final_pct)}")
    print(f"  mean absolute error:    {np.mean(final_abs):.3f} km")
    print(f"  median absolute error:  {np.median(final_abs):.3f} km")
    print(f"  max absolute error:     {np.max(final_abs):.3f} km")
    print(f"  mean normalized error:  {np.mean(final_pct):.4f}%")
    print(f"  median normalized:      {np.median(final_pct):.4f}%")
    print(f"  max normalized:         {np.max(final_pct):.4f}%")


if __name__ == "__main__":
    main()