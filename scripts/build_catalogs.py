#!/usr/bin/env python3
"""build_catalogs.py generates local oil and leeway catalogs for the CLI."""
import argparse
import json
import math
from importlib.resources import files
from pathlib import Path

from oil.build_oil_catalog import build_catalog

ROOT = Path(__file__).resolve().parents[1]


def build_objects(source: Path | None, output: Path) -> dict:
    """read the existing asset JSON or regenerate stable IDs from OBJECTPROP.DAT."""
    resource = source or files("opendrift.models").joinpath("OBJECTPROP.DAT")
    text = resource.read_text(encoding="utf-8")
    if str(resource).endswith(".json"):
        objects = json.loads(text)["objects"]
    else:
        objects = {}
        lines = text.splitlines()
        for i in range(0, len(lines), 3):
            if not lines[i].strip():
                break
            key = lines[i].split()[0]
            coefficients = list(map(float, lines[i + 2].split()))
            if len(coefficients) != 9 or key in objects:
                raise ValueError(f"invalid leeway record {key}")
            objects[key] = {"name": lines[i + 1].strip(), "downwind": coefficients[:3],
                            "right": coefficients[3:6], "left": coefficients[6:]}
    if not objects:
        raise ValueError("object catalog is empty")
    for name, obj in objects.items():
        for field in ("downwind", "right", "left"):
            if len(obj[field]) != 3 or not all(math.isfinite(v) for v in obj[field]):
                raise ValueError(f"invalid {field} coefficients for {name}")
    catalog = {"schema_version": 1, "source": str(resource), "objects": objects}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(catalog, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"wrote {len(objects)} objects to {output}", flush=True)
    return catalog


def main():
    """generate both catalogs in the project directory unless explicitly overridden."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "catalogs")
    parser.add_argument("--oil-source", type=Path, help="NOAA JSON directory instead of OpenDrift's archive")
    parser.add_argument("--object-source", type=Path, help="OBJECTPROP.DAT or existing object_catalog.json")
    parser.add_argument("--water-fraction-limits", type=Path)
    args = parser.parse_args()
    build_objects(args.object_source, args.output_dir / "object_catalog.json")
    build_catalog(args.oil_source, args.output_dir / "oil_catalog.json", args.water_fraction_limits)


if __name__ == "__main__":
    main()
