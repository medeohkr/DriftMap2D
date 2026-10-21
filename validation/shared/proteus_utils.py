import json
import subprocess
from pathlib import Path

# Path to the CLI binary, relative to this module's parent (validation/shared/)
# → repo root is ../../ from validation/shared/
CLI_BIN = Path(__file__).resolve().parent.parent.parent / "target" / "release" / "proteus.exe"


def write_config(run_dir, config):
    path = Path(run_dir) / "config.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(config, f, indent=2)
    return path


def run_cli(run_dir, config):
    """Run the proteus CLI on a config, return (elapsed_sec, ok)."""
    import time
    cfg = write_config(run_dir, config)
    t0 = time.time()
    result = subprocess.run(
        [str(CLI_BIN.resolve()), str(cfg.resolve())],
        cwd=str(Path(run_dir).resolve()),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return time.time() - t0, result