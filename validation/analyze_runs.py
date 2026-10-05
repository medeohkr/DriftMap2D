#!/usr/bin/env python3
"""
Load all runs from runs/, compute the per-step normalized haversine
error between DriftMap and OpenDrift centroids, and plot it.

The normalization is:
    error_pct[n] = 100 * haversine(dm[n], od[n]) / haversine(od[n], od[0])

i.e. the centroid separation at step n, expressed as a percentage of
the distance OpenDrift has drifted since release. This makes runs at
different locations comparable — a run that drifts far but agrees
well will show a small percentage, same as a run that drifts a little.

Reads:
    runs/run_XXX/opendrift_centroids.json
    runs/run_XXX/proteus_centroids.json

Writes:
    error_vs_step_pct.png
    error_vs_step_pct.csv
"""
import json
from pathlib import Path

import numpy as np
import matplotlib.pyplot as plt


RUNS_DIR = Path("runs")
OUT_PNG = Path("error_vs_step_pct.png")
OUT_CSV = Path("error_vs_step_pct.csv")


# ---------------------------------------------------------------------------
# Haversine
# ---------------------------------------------------------------------------
def haversine_km(lon1, lat1, lon2, lat2):
    """Great-circle distance in km. Accepts scalars or broadcastable arrays."""
    R = 6371.0088
    lon1, lat1, lon2, lat2 = map(np.radians, [lon1, lat1, lon2, lat2])
    dlon = lon2 - lon1
    dlat = lat2 - lat1
    a = np.sin(dlat / 2.0) ** 2 + np.cos(lat1) * np.cos(lat2) * np.sin(dlon / 2.0) ** 2
    return R * 2.0 * np.arcsin(np.sqrt(a))


# ---------------------------------------------------------------------------
# Load centroids from JSON
# ---------------------------------------------------------------------------
def load_centroids(path):
    """Load a centroids JSON file and return (steps, lons, lats).

    Each entry is expected to be {step: int, lon: float, lat: float}.
    The OpenDrift file may also carry 'time' and 'n_active' — those
    are ignored.
    """
    with open(path) as f:
        data = json.load(f)

    steps = np.array([row["step"] for row in data])
    lons = np.array([row["lon"] for row in data])
    lats = np.array([row["lat"] for row in data])

    order = np.argsort(steps)
    return steps[order], lons[order], lats[order]


# ---------------------------------------------------------------------------
# Per-run normalized error
# ---------------------------------------------------------------------------
def run_error_series(od_lon, od_lat, dm_lon, dm_lat):
    """Return normalized error at each step.

    error_pct[n] = 100 * haversine(dm[n], od[n]) / haversine(od[n], od[0])

    Where the drift distance is zero (step 0), the error is defined as 0.
    """
    n = min(len(od_lon), len(dm_lon))
    od_lon, od_lat = od_lon[:n], od_lat[:n]
    dm_lon, dm_lat = dm_lon[:n], dm_lat[:n]

    abs_err_km = haversine_km(dm_lon, dm_lat, od_lon, od_lat)
    drift_km = haversine_km(od_lon, od_lat, od_lon[0], od_lat[0])

    # Avoid division by zero at step 0 (drift_km[0] == 0 by construction)
    err_pct = np.zeros(n)
    mask = drift_km > 1e-6
    err_pct[mask] = 100.0 * abs_err_km[mask] / drift_km[mask]

    return err_pct, abs_err_km, drift_km


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    run_dirs = sorted(p for p in RUNS_DIR.iterdir() if p.is_dir())
    print(f"Found {len(run_dirs)} runs in {RUNS_DIR}/")

    per_run_pct = []
    per_run_abs = []
    per_run_drift = []

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

        err_pct, abs_km, drift_km = run_error_series(od_lon, od_lat, dm_lon, dm_lat)

        per_run_pct.append(err_pct)
        per_run_abs.append(abs_km)
        per_run_drift.append(drift_km)

        print(
            f"  {run_dir.name}: {len(err_pct)} steps, "
            f"final drift {drift_km[-1]:.2f} km, "
            f"final abs err {abs_km[-1]:.3f} km, "
            f"final pct err {err_pct[-1]:.3f}%"
        )

    if not per_run_pct:
        print("No usable runs.")
        return

    # Pad to common length
    max_len = max(len(e) for e in per_run_pct)
    padded_pct = np.full((len(per_run_pct), max_len), np.nan)
    padded_abs = np.full((len(per_run_abs), max_len), np.nan)
    padded_drift = np.full((len(per_run_drift), max_len), np.nan)
    for i, (p, a, d) in enumerate(zip(per_run_pct, per_run_abs, per_run_drift)):
        padded_pct[i, :len(p)] = p
        padded_abs[i, :len(a)] = a
        padded_drift[i, :len(d)] = d

    steps = np.arange(max_len)

    mean_pct = np.nanmean(padded_pct, axis=0)
    std_pct = np.nanstd(padded_pct, axis=0)
    p10_pct = np.nanpercentile(padded_pct, 10, axis=0)
    p90_pct = np.nanpercentile(padded_pct, 90, axis=0)

    mean_abs = np.nanmean(padded_abs, axis=0)
    mean_drift = np.nanmean(padded_drift, axis=0)

    # CSV
    with open(OUT_CSV, "w") as f:
        f.write("step,mean_pct,std_pct,p10_pct,p90_pct,mean_abs_km,mean_drift_km,n_runs\n")
        for i in range(max_len):
            n_here = int(np.sum(~np.isnan(padded_pct[:, i])))
            f.write(
                f"{i},{mean_pct[i]:.6f},{std_pct[i]:.6f},"
                f"{p10_pct[i]:.6f},{p90_pct[i]:.6f},"
                f"{mean_abs[i]:.6f},{mean_drift[i]:.6f},{n_here}\n"
            )
    print(f"\nWrote {OUT_CSV}")

    # Plot — two panels: normalized % on top, absolute km on bottom
    fig, (ax_pct, ax_abs) = plt.subplots(
        2, 1, figsize=(11, 9), sharex=True,
        gridspec_kw={"height_ratios": [2, 1]},
    )

    # Top: percentage error
    ax_pct.plot(steps, mean_pct, color="C0", lw=2, label="mean")
    ax_pct.fill_between(
        steps, p10_pct, p90_pct,
        color="C0", alpha=0.15, label="10th–90th percentile",
    )
    ax_pct.plot(
        steps, mean_pct + std_pct,
        color="C0", lw=0.7, ls="--", alpha=0.6, label="mean ± 1 std",
    )
    ax_pct.set_ylabel("centroid error (% of drift distance)")
    ax_pct.set_title(
        "DriftMap vs OpenDrift: normalized centroid error over 72h "
        f"across {len(per_run_pct)} Pacific runs"
    )
    ax_pct.grid(True, alpha=0.3)
    ax_pct.legend(loc="upper left")
    ax_pct.axhline(0, color="k", lw=0.5, alpha=0.3)

    # Bottom: absolute km + drift reference
    ax_abs.plot(steps, mean_abs, color="C1", lw=2, label="mean absolute error (km)")
    ax_abs.plot(
        steps, mean_drift,
        color="gray", lw=1, ls=":", label="mean drift distance (km)",
    )
    ax_abs.set_xlabel("simulation step (hours since release)")
    ax_abs.set_ylabel("distance (km)")
    ax_abs.grid(True, alpha=0.3)
    ax_abs.legend(loc="upper left")

    plt.tight_layout()
    plt.savefig(OUT_PNG, dpi=150)
    print(f"Wrote {OUT_PNG}")

    # Summary
    final_pct = padded_pct[:, -1]
    final_pct = final_pct[~np.isnan(final_pct)]
    print()
    print("Final-step summary")
    print("------------------")
    print(f"  runs contributing:   {len(final_pct)}")
    print(f"  mean drift distance: {mean_drift[-1]:.2f} km")
    print(f"  mean abs error:      {mean_abs[-1]:.3f} km")
    print(f"  mean normalized:     {np.mean(final_pct):.4f}%")
    print(f"  median normalized:   {np.median(final_pct):.4f}%")
    print(f"  max normalized:      {np.max(final_pct):.4f}%")
    print(f"  min normalized:      {np.min(final_pct):.4f}%")


if __name__ == "__main__":
    main()