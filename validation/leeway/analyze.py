#!/usr/bin/env python3
"""Analyze and plot per-step centroid error for the leeway validation runs."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from shared.analysis import analyze_runs


def main():
    analyze_runs(
        Path("runs"),
        Path("error_vs_step_pct.csv"),
        Path("error_vs_step_abs.csv"),
        Path("error_vs_step.png"),
        "DriftMap Search and Rescue vs OpenDrift Leeway: centroid error "
        "across {n_runs} runs, 100,000 particles, Euler",
        "simulation step (hours since release)",
    )


if __name__ == "__main__":
    main()
