#!/usr/bin/env python3
"""Compare evaporation and emulsion water fraction at matching output times."""
import argparse
import csv
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

HERE = Path(__file__).resolve().parent
METRICS = ("evaporation_percent", "emulsification_percent")


def compare_run(run_dir):
    meta = json.loads((run_dir / "meta.json").read_text())
    if not meta.get("success"):
        raise ValueError(meta.get("error", "run did not complete"))
    od = json.loads((run_dir / "opendrift_budget.json").read_text())
    cli = json.loads((run_dir / "proteus_centroids.json").read_text())
    expected = int(meta["duration_hours"] * 3600 / meta["time_step_seconds"]) + 1
    if len(od) != expected or len(cli) != expected:
        raise ValueError(f"expected {expected} samples including step zero")
    rows = []
    for step, (reference, candidate) in enumerate(zip(od, cli)):
        if reference["step"] != step or candidate["step"] != step:
            raise ValueError(f"misaligned step {step}")
        for metric in METRICS:
            a, b = float(reference[metric]), float(candidate[metric])
            if not np.isfinite([a, b]).all():
                raise ValueError(f"nonfinite {metric} at step {step}")
            rows.append({"run": run_dir.name, "step": step,
                         "hours": step * meta["time_step_seconds"] / 3600,
                         "metric": metric, "opendrift_percent": a,
                         "proteus_percent": b, "error_percentage_points": b - a})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs-dir", type=Path, default=HERE / "runs")
    args = parser.parse_args()
    rows = []
    for run_dir in sorted(args.runs_dir.glob("run_*")):
        try:
            rows.extend(compare_run(run_dir))
        except (OSError, ValueError, KeyError, TypeError) as exc:
            print(f"Skipping {run_dir.name}: {exc}")
    if not rows:
        raise SystemExit("No complete oil comparisons found. Run run.py first.")
    with (HERE / "budget_comparison.csv").open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), sharex=True)
    summary = {}
    for index, metric in enumerate(METRICS):
        selected = [row for row in rows if row["metric"] == metric]
        hours = sorted({row["hours"] for row in selected})
        for key, label in (("opendrift_percent", "OpenDrift"), ("proteus_percent", "Proteus")):
            axes[index, 0].plot(hours, [np.mean([r[key] for r in selected if r["hours"] == h])
                                       for h in hours], label=label)
        errors = [[r["error_percentage_points"] for r in selected if r["hours"] == h]
                  for h in hours]
        axes[index, 1].plot(hours, [np.mean(np.abs(e)) for e in errors], label="Mean absolute error")
        axes[index, 1].plot(hours, [np.max(np.abs(e)) for e in errors], label="Maximum absolute error")
        axes[index, 0].set_ylabel(metric.replace("_percent", "").replace("_", " ") + " (%)")
        axes[index, 1].set_ylabel("Error (percentage points)")
        values = np.asarray([r["error_percentage_points"] for r in selected])
        summary[metric] = {"mean_absolute_error_pp": float(np.mean(np.abs(values))),
                           "max_absolute_error_pp": float(np.max(np.abs(values))),
                           "rmse_pp": float(np.sqrt(np.mean(values**2)))}
    for ax in axes.flat:
        ax.grid(alpha=0.3)
        ax.legend()
        ax.set_xlabel("Hours since release")
    n_runs = len({row["run"] for row in rows})
    fig.suptitle(f"Proteus vs OpenDrift oil budget: {n_runs} runs")
    fig.tight_layout()
    fig.savefig(HERE / "budget_vs_step.png", dpi=160)
    plt.close(fig)
    (HERE / "budget_summary.json").write_text(json.dumps({"n_runs": n_runs, **summary}, indent=2))
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
