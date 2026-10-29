#!/usr/bin/env python3
"""analyze.py plots relative and absolute NorKyst trajectory errors."""

import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from shared.analysis import analyze_runs


def main():
    """reuse the generic suite's mean and 10–90th percentile analysis."""
    if not (HERE / "runs").exists():
        raise SystemExit("run run.py first")
    analyze_runs(
        HERE / "runs",
        HERE / "error_vs_step_pct.csv",
        HERE / "error_vs_step_abs.csv",
        HERE / "error_vs_step.png",
        "Proteus vs OpenDrift NorKyst Ocean Drift across {n_runs} runs\n"
        "28–30 october 2026, no windage, RK4",
        "simulation step (hours since release)",
        max_step=48,
    )


if __name__ == "__main__":
    main()
