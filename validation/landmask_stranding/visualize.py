#!/usr/bin/env python3
"""visualize.py plots all released particles and cumulative stranded counts."""

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from norkyst_leeway.visualize import animate_particles


def visualize(directory):
    """reuse the coastal comparison renderer with hourly particle snapshots."""
    directory = Path(directory).resolve()
    config = json.loads((directory / "config.json").read_text())
    animate_particles(directory / "particles_hourly.jsonl", directory / "opendrift_output.nc",
                      directory, "deterministic stranding", config, config["particles"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", nargs="?", default="run_000")
    args = parser.parse_args()
    visualize(HERE / "runs" / args.run)
