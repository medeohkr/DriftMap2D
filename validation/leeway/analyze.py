#!/usr/bin/env python3
"""Analyze and plot per-step centroid error for the leeway validation runs."""
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
        "Proteus vs OpenDrift Leeway: centroid error "
        "across {n_runs} runs, 100,000 particles, Euler scheme",
        "simulation step (hours since release)",
    )


if __name__ == "__main__":
    main()
