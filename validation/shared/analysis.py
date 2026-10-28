from pathlib import Path

import numpy as np

from .metrics import aggregate, load_centroids, run_error_series, write_csv
from .plot import plot_error_vs_step


def analyze_runs(runs_dir, out_csv_pct, out_csv_abs, out_png, title, x_label, *, max_step=None):
    run_dirs = sorted(p for p in Path(runs_dir).iterdir() if p.is_dir())
    print(f"Found {len(run_dirs)} runs in {runs_dir}/")

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
        if max_step is not None:
            err_pct = err_pct[:max_step + 1]
            abs_km = abs_km[:max_step + 1]
            path_km = path_km[:max_step + 1]
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

    max_len = max(len(errors) for errors in per_run_pct)
    agg_pct = aggregate(per_run_pct, max_len)
    agg_abs = aggregate(per_run_abs, max_len)
    agg_path = aggregate(per_run_path, max_len)

    write_csv(out_csv_pct, agg_pct, max_len, "pct")
    print(f"\nWrote {out_csv_pct}")

    write_csv(
        out_csv_abs,
        agg_abs,
        max_len,
        "km",
        extra_cols={"mean_path_km": agg_path["mean"]},
    )
    print(f"Wrote {out_csv_abs}")

    plot_error_vs_step(
        np.arange(max_len),
        agg_pct,
        agg_abs,
        title.format(n_runs=len(per_run_pct)),
        out_png,
        x_label=x_label,
    )
    print(f"Wrote {out_png}")

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
