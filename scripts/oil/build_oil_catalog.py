#!/usr/bin/env python3
"""build_oil_catalog.py builds oil properties using OpenDrift's ADIOS wrapper."""
from __future__ import annotations

import argparse
import gzip
import json
from importlib.metadata import version
from importlib.resources import files
from pathlib import Path

import numpy as np
from adios_db.computation import physical_properties as pp
from opendrift.models.openoil.adios.oil import OpendriftOil

TEMPERATURES_C = np.arange(-5.0, 40.5, 0.5)


def load_water_fraction_limits(path: Path | None = None) -> dict:
    """read the Norwegian temperature limits distributed with OpenDrift."""
    resource = path or files("opendrift.models.openoil.adios").joinpath("max_water_fraction.json")
    return json.loads(resource.read_text(encoding="utf-8"))


def opendrift_name(raw: dict) -> str:
    """apply OpenDrift's year suffix for Norwegian oil names."""
    name = raw["metadata"]["name"]
    if raw["oil_id"].startswith("NO"):
        metadata = raw["metadata"]
        year = str(metadata["reference"]["year"])
        sample = str(metadata.get("sample_date", ""))
        if sample.isnumeric():
            year = sample
        elif sample[:4].isnumeric():
            year = sample[:4]
        if not name.endswith(" " + year):
            name += " " + year
    return name


def convert_record(raw: dict, source: str, water_limits: dict) -> dict:
    """extract the same fresh oil properties used by validation/oil/run.py."""
    oil = OpendriftOil(raw)
    if not oil.valid():
        raise ValueError("oil is not GNOME suitable")
    gnome = oil.gnome_oil
    density_model = pp.Density(oil.oil)
    viscosity_model = pp.KinematicViscosity(oil.oil)
    density = [float(density_model.at_temp(t + 273.15)) for t in TEMPERATURES_C]
    viscosity = [float(viscosity_model.at_temp(t + 273.15)) * rho * 1000
                 for t, rho in zip(TEMPERATURES_C, density)]
    fractions = np.asarray(oil.mass_fraction)
    sara = np.asarray(gnome["sara_type"])
    name = opendrift_name(raw)
    maximum = float(oil.emulsion_water_fraction_max)
    if not 0 <= maximum <= 0.9:
        raise ValueError("invalid GNOME maximum water fraction")
    limits = water_limits.get(name)
    table = []
    if limits:
        temperatures = limits["temperatures"]
        values = limits["max_water_fraction"]
        if len(temperatures) != len(values) or not values:
            raise ValueError("invalid Norwegian water fraction limits")
        # store the measured limit separately so runtime only interpolates and caps it
        table = sorted([[float(value), float(t)] for value, t in zip(values, temperatures)], key=lambda p: p[1])
        if any(not 0 <= value <= 0.9 or not np.isfinite(t) for value, t in table):
            raise ValueError("invalid Norwegian water fraction limit")
        if any(a[1] >= b[1] for a, b in zip(table, table[1:])):
            raise ValueError("water fraction temperatures must increase")
    record = {
        "oil_id": oil.id, "name": name, "product_type": str(oil.oil.metadata.product_type),
        "api_gravity": float(gnome["api"]),
        "density_kgm3": list(map(list, zip(density, TEMPERATURES_C.tolist()))),
        "dynamic_viscosity_cp": list(map(list, zip(viscosity, TEMPERATURES_C.tolist()))),
        "interfacial_tension_n_m": [[float(oil.oil_water_surface_tension()), 15.0]],
        "sara_mass_fractions": {key.lower(): float(fractions[sara == key].sum())
                                for key in ("Saturates", "Aromatics", "Resins", "Asphaltenes")},
        "boiling_points_c": (np.asarray(gnome["boiling_point"]) - 273.15).tolist(),
        "molecular_weights_kg_mol": (np.asarray(oil.molecular_weight) / 1000).tolist(),
        "component_mass_fractions": fractions.tolist(),
        "bullwinkle_fraction": float(oil.bullwinkle), "bulltime": float(oil.bulltime),
        "emulsion_water_fraction_max": maximum,
        "emulsion_water_fraction_max_by_temperature": table,
        "source_file": source,
    }
    if not len(fractions) or not np.isclose(fractions.sum(), 1.0, atol=1e-6):
        raise ValueError("pseudo-component fractions do not sum to one")
    json.dumps(record, allow_nan=False)
    return record


def convert_oil(path: Path, water_limits: dict | None = None) -> dict:
    """convert one NOAA JSON record without custom property estimates."""
    return convert_record(json.loads(path.read_text(encoding="utf-8")), str(path),
                          load_water_fraction_limits() if water_limits is None else water_limits)


def build_catalog(input_dir: Path, output_path: Path, water_limits_path: Path | None = None) -> dict:
    """write a catalog and reproducible gzip copy, recording unsupported oils."""
    limits = load_water_fraction_limits(water_limits_path)
    records = {}
    skipped = []
    paths = sorted(input_dir.rglob("*.json"))
    for index, path in enumerate(paths, 1):
        try:
            record = convert_oil(path, limits)
            records[record["oil_id"]] = record
        except Exception as error:
            skipped.append({"file": str(path), "error": f"{type(error).__name__}: {error}"})
        if index % 100 == 0:
            print(f"processed {index}/{len(paths)} oil records", flush=True)
    if not records:
        raise ValueError("no GNOME suitable oils were converted")
    output = {
        "schema_version": 2, "source": "NOAA ADIOS Oil Database via OpenDrift",
        "versions": {"opendrift": version("opendrift"), "adios_db": version("adios_db")},
        "units": {"density": "kg/m^3", "temperature": "degC", "dynamic_viscosity": "cP",
                  "interfacial_tension": "N/m", "molecular_weight": "kg/mol", "mass_fraction": "1", "bulltime": "s"},
        "tuple_convention": {"density_kgm3": "[value_kg_m3, temperature_c]",
                             "dynamic_viscosity_cp": "[value_cp, temperature_c]",
                             "interfacial_tension_n_m": "[value_n_m, temperature_c]",
                             "emulsion_water_fraction_max_by_temperature": "[fraction, temperature_c]"},
        "oils": sorted(records.values(), key=lambda o: (o["name"], o["oil_id"])),
        "statistics": {"oil_count": len(records), "skipped_count": len(skipped)}, "skipped": skipped,
    }
    payload = (json.dumps(output, indent=2, ensure_ascii=False, allow_nan=False) + "\n").encode("utf-8")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(payload)
    compressed = output_path.with_suffix(output_path.suffix + ".gz")
    compressed.write_bytes(gzip.compress(payload, compresslevel=9, mtime=0))
    print(f"wrote {len(records)} oils and skipped {len(skipped)} records to {output_path}", flush=True)
    return output


def main():
    """build the catalog from a directory of NOAA oil JSON files."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("-o", "--output", type=Path, default=Path("oil_catalog.json"))
    parser.add_argument("--water-fraction-limits", type=Path,
                        help="override OpenDrift's max_water_fraction.json")
    args = parser.parse_args()
    if not args.input_dir.is_dir():
        parser.error(f"not a directory {args.input_dir}")
    build_catalog(args.input_dir, args.output, args.water_fraction_limits)


if __name__ == "__main__":
    main()
