#!/usr/bin/env python3
"""Analyze and plot per-step centroid error for the generic validation runs."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from shared.analysis import analyze_runs


def main():
    analyze_runs(
        HERE / "runs",
        HERE / "error_vs_step_pct.csv",
        HERE / "error_vs_step_abs.csv",
        HERE / "error_vs_step.png",
        "Proteus vs Opendrift Ocean Drift: centroid error "
        "across {n_runs} runs, no windage, RK4 scheme",
        "simulation step (hours since release)",
    )


if __name__ == "__main__":
    main()
