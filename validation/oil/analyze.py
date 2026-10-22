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
    # One row per metric and output time, aggregated across all valid runs.
    fields = ["step", "hours", "metric", "mean", "std", "p10", "p90", "rmse", "n_runs"]
    aggregated = {"pct": [], "abs": []}
    n_runs = len({row["run"] for row in rows})
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), sharex=True)
    for index, metric in enumerate(METRICS):
        selected = [row for row in rows if row["metric"] == metric]
        for hours in sorted({row["hours"] for row in selected}):
            samples = [row for row in selected if row["hours"] == hours]
            absolute = np.abs([row["error_percentage_points"] for row in samples])
            reference = np.asarray([row["opendrift_percent"] for row in samples])
            # Relative error is undefined when OpenDrift's fraction is zero.
            relative = 100 * absolute[reference > 0] / reference[reference > 0]
            for kind, values in (("abs", absolute), ("pct", relative)):
                stats = {"step": samples[0]["step"], "hours": hours, "metric": metric,
                         "mean": float(np.mean(values)) if len(values) else float("nan"),
                         "std": float(np.std(values)) if len(values) else float("nan"),
                         "p10": float(np.percentile(values, 10)) if len(values) else float("nan"),
                         "p90": float(np.percentile(values, 90)) if len(values) else float("nan"),
                         "rmse": float(np.sqrt(np.mean(values**2))) if len(values) else float("nan"),
                         "n_runs": len(values)}
                aggregated[kind].append(stats)
        for column, kind in enumerate(("pct", "abs")):
            stats = [row for row in aggregated[kind] if row["metric"] == metric]
            ax = axes[index, column]
            hours = [r["hours"] for r in stats]
            color = "C0" if kind == "pct" else "C1"
            ax.plot(hours, [r["mean"] for r in stats], color=color, lw=2,
                    label="Mean across runs")
            ax.fill_between(hours, [r["p10"] for r in stats],
                            [r["p90"] for r in stats], color=color, alpha=0.15,
                            label="10th–90th percentile")
            ax.set_title(metric.replace("_percent", "").capitalize())
            ax.set_ylabel("Absolute error (percentage points)" if kind == "abs"
                          else "Relative error (%)")
            ax.set_xlabel("Hours since release")
            ax.grid(alpha=0.3)
            ax.legend(loc="upper left")
            ax.axhline(0, color="k", lw=0.5, alpha=0.3)
    for kind, stats in aggregated.items():
        path = HERE / f"error_vs_step_{kind}.csv"
        unit = "pp" if kind == "abs" else "pct"
        names = {key: f"{key}_{unit}" for key in ("mean", "std", "p10", "p90", "rmse")}
        with path.open("w", newline="") as file:
            writer = csv.DictWriter(file, fieldnames=[names.get(key, key) for key in fields])
            writer.writeheader()
            writer.writerows({names.get(key, key): value for key, value in row.items()}
                             for row in stats)
        print(f"Wrote {path}")
    fig.suptitle(f"Proteus vs OpenDrift oil budget: mean errors across {n_runs} runs")
    fig.tight_layout()
    fig.savefig(HERE / "error_vs_step.png", dpi=160)
    plt.close(fig)
    print(f"Analyzed {n_runs} runs. Relative errors exclude zero OpenDrift values.")



if __name__ == "__main__":
    main()
