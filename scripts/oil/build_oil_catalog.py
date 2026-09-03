#!/usr/bin/env python3
"""
Build a consolidated oil catalog from NOAA ADIOS JSON records.

Uses adios_db.computation.* for all estimations and property extraction
instead of re-implementing them.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import adios_db.scripting as ads
from adios_db.computation import physical_properties as pp
from adios_db.computation import estimations as est
from adios_db.computation import gnome_oil as go


REFERENCE_TEMP_C = 15.0

import numpy as np

def to_json_safe(obj):
    """
    Recursively convert numpy arrays / scalars and NaNs to JSON-safe Python types.
    """
    # Handle NumPy arrays
    if isinstance(obj, np.ndarray):
        # 0-d array -> convert to scalar first
        if obj.ndim == 0:
            value = obj.item()
            if isinstance(value, (int, np.integer)):
                return int(value)
            if isinstance(value, (float, np.floating)):
                value = float(value)
                return value if math.isfinite(value) else None
            return value

        # Multi-d array -> flatten to nested lists
        return [to_json_safe(x) for x in obj.tolist()]

    # Handle NumPy scalar types explicitly
    if isinstance(obj, (np.integer, np.int64, np.int32)):
        return int(obj)

    if isinstance(obj, (np.floating, np.float64, np.float32)):
        value = float(obj)
        return value if math.isfinite(value) else None

    # Plain Python types
    if isinstance(obj, float):
        return obj if math.isfinite(obj) else None

    if isinstance(obj, dict):
        return {k: to_json_safe(v) for k, v in obj.items()}

    if isinstance(obj, (list, tuple)):
        return [to_json_safe(x) for x in obj]

    return obj

def get(obj: Any, *names: str, default=None):
    for name in names:
        if isinstance(obj, dict):
            value = obj.get(name)
        else:
            value = getattr(obj, name, None)

        if value is not None:
            return value

    return default


def number(value: Any, default=None) -> float | None:
    if value is None or isinstance(value, bool):
        return default
    try:
        value = float(value)
    except (TypeError, ValueError):
        return default
    return value if math.isfinite(value) else default


def clamp(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def fresh_sample(oil):
    samples = get(oil, "sub_samples", default=[]) or []
    for sample in samples:
        metadata = get(sample, "metadata", default={})
        fraction = get(metadata, "fraction_evaporated", default={})
        value = number(get(fraction, "value", default=0.0), 0.0)
        if value == 0.0 or value < 0.001:
            return sample
    raise ValueError("oil has no fresh subsample")


def get_api(oil) -> float:
    metadata = get(oil, "metadata", default={})
    return number(get(metadata, "api", "API", default=None), 30.0)


def extract_density_points(oil) -> tuple[list[tuple[float, float]], bool]:
    """
    Return:
        [(density_kg_m3, temperature_c), ...], estimated
    """
    try:
        table = pp.get_density_data(oil, units="kg/m^3", temp_units="K")
    except Exception:
        table = []

    result = []
    for density, temp_k in table or []:
        density = number(density)
        temp_k = number(temp_k)
        if density is None or temp_k is None:
            continue
        result.append((density, temp_k - 273.15))

    result.sort(key=lambda pair: pair[1])

    if result:
        return deduplicate_temperature_points(result), False

    # Fallback estimate from API.
    api = get_api(oil)
    density_kg_m3 = 141.5 / (api + 131.5) * 999.016
    return [(density_kg_m3, REFERENCE_TEMP_C)], True


def extract_viscosity_points(oil, density_points) -> tuple[list[tuple[float, float]], bool]:
    """
    Return dynamic viscosity as:
        [(dynamic_viscosity_cp, temperature_c), ...]
    """
    # Try dynamic viscosity first.
    try:
        table = pp.get_dynamic_viscosity_data(oil, units="Pas", temp_units="K")
    except Exception:
        table = []

    result = []
    for visc_pas, temp_k in table or []:
        visc_pas = number(visc_pas)
        temp_k = number(temp_k)
        if visc_pas is None or temp_k is None or visc_pas <= 0:
            continue
        visc_cp = visc_pas * 1000.0
        result.append((visc_cp, temp_k - 273.15))

    result.sort(key=lambda pair: pair[1])

    if result:
        return deduplicate_temperature_points(result), False

    # Fallback to kinematic viscosity and convert to dynamic.
    try:
        table = pp.get_kinematic_viscosity_data(oil, units="m^2/s", temp_units="K")
    except Exception:
        table = []

    result = []
    for kvis, temp_k in table or []:
        kvis = number(kvis)
        temp_k = number(temp_k)
        if kvis is None or temp_k is None or kvis <= 0:
            continue

        density_kg_m3 = lerp_value_first(density_points, temp_k - 273.15)
        density_g_ml = density_kg_m3 / 1000.0

        # m^2/s * 1e3 * g/mL = cP
        visc_cp = kvis * 1e6 * density_g_ml
        result.append((visc_cp, temp_k - 273.15))

    result.sort(key=lambda pair: pair[1])

    if result:
        return deduplicate_temperature_points(result), False

    # No viscosity estimate in adios_db.computation; mark as estimated.
    api = get_api(oil)
    estimated_cp = 10.0 ** clamp((35.0 - api) / 8.0, -1.0, 4.0)
    return [(estimated_cp, REFERENCE_TEMP_C)], True


def extract_interfacial_tension(oil) -> tuple[list[tuple[float, float]], bool]:
    """
    Return:
        [(interfacial_tension_N_m, temperature_c), ...]
    """
    # Prefer seawater.
    try:
        table = pp.get_interfacial_tension_seawater(oil, units="N/m", temp_units="K")
    except Exception:
        table = []

    if not table:
        try:
            table = pp.get_interfacial_tension_water(oil, units="N/m", temp_units="K")
        except Exception:
            table = []

    result = []
    for tension, temp_k in table or []:
        tension = number(tension)
        temp_k = number(temp_k)
        if tension is None or temp_k is None or tension <= 0:
            continue
        result.append((tension, temp_k - 273.15))

    result.sort(key=lambda pair: pair[1])

    if result:
        return deduplicate_temperature_points(result), False

    # API-based estimate.
    api = get_api(oil)
    value = est.oil_water_surface_tension_from_api(api)
    return [(clamp(value, 1e-5, 0.2), REFERENCE_TEMP_C)], True


def extract_distillation_cuts(oil) -> tuple[list[tuple[float, float]], bool]:
    """
    Return:
        [(cumulative_fraction, vapor_temperature_c), ...]
    """
    try:
        table = pp.get_distillation_cuts(oil, units="fraction", temp_units="K")
    except Exception:
        table = []

    result = []
    for fraction, temp_k in table or []:
        fraction = number(fraction)
        temp_k = number(temp_k)
        if fraction is None or temp_k is None:
            continue
        result.append((fraction, temp_k - 273.15))

    result.sort(key=lambda pair: pair[0])

    if result:
        return result, False

    # Use adios_db estimation from API, with safe clamping.
    api = get_api(oil)
    api = clamp(api, 1.0, 100.0)  # avoid log(0) or negative

    temps_k = est.cut_temps_from_api(api, N=10)
    n = len(temps_k)
    result = [( (i + 1) / n, t_k - 273.15 ) for i, t_k in enumerate(temps_k)]
    return result, True

def extract_sara(oil, density_points, viscosity_points) -> tuple[dict[str, float], dict[str, bool]]:
    """
    Use adios_db.computation.gnome_oil.sara_totals if available,
    otherwise fall back to estimations.
    """
    estimated = {
        "saturates": False,
        "aromatics": False,
        "resins": False,
        "asphaltenes": False,
    }

    try:
        sara = go.sara_totals(oil)
    except Exception:
        sara = None

    if sara is None:
        # Estimate from density/viscosity using adios_db estimators.
        density_15 = lerp_value_first(density_points, REFERENCE_TEMP_C)
        viscosity_15 = lerp_value_first(viscosity_points, REFERENCE_TEMP_C)

        f_other = 0.0
        f_res = est.resin_fraction(density_15, viscosity_15, f_other)
        f_asph = est.asphaltene_fraction(density_15, viscosity_15, f_other)
        f_sat = est.saturates_fraction(density_15, viscosity_15, f_other)
        f_arom = est.aromatics_fraction(f_res, f_asph, f_sat)

        sara = {
            "saturates": f_sat,
            "aromatics": f_arom,
            "resins": f_res,
            "asphaltenes": f_asph,
        }
        estimated = {k: True for k in sara}
    else:
        # sara_totals may return a dict or a tuple; normalize.
        if isinstance(sara, (list, tuple)) and len(sara) == 4:
            sara = {
                "saturates": sara[0],
                "aromatics": sara[1],
                "resins": sara[2],
                "asphaltenes": sara[3],
            }
        elif not isinstance(sara, dict):
            density_15 = lerp_value_first(density_points, REFERENCE_TEMP_C)
            viscosity_15 = lerp_value_first(viscosity_points, REFERENCE_TEMP_C)
            f_other = 0.0
            f_res = est.resin_fraction(density_15, viscosity_15, f_other)
            f_asph = est.asphaltene_fraction(density_15, viscosity_15, f_other)
            f_sat = est.saturates_fraction(density_15, viscosity_15, f_other)
            f_arom = est.aromatics_fraction(f_res, f_asph, f_sat)
            sara = {
                "saturates": f_sat,
                "aromatics": f_arom,
                "resins": f_res,
                "asphaltenes": f_asph,
            }
            estimated = {k: True for k in sara}

    # Normalize in case of rounding or estimation drift.
    total = sum(sara.values())
    if total > 0:
        sara = {k: v / total for k, v in sara.items()}

    return sara, estimated


def extract_component_vectors(
    cuts: list[tuple[float, float]],
    sara: dict[str, float],
) -> tuple[list[float], list[float], list[float]]:
    boiling_points_c = []
    molecular_weights = []
    component_mass_fractions = []

    previous_fraction = 0.0

    for cumulative_fraction, boiling_point_c in cuts:
        cut_fraction = max(0.0, cumulative_fraction - previous_fraction)
        previous_fraction = cumulative_fraction

        bp_k = boiling_point_c + 273.15

        # ── Normalize per cut ──
        cut_components = []
        for component, sara_key in (
            ("saturates", "saturates"),
            ("aromatics", "aromatics"),
            ("resins", "resins"),
            ("asphaltenes", "asphaltenes"),
        ):
            boiling_points_c.append(boiling_point_c)

            if component == "saturates":
                mw = est.saturate_mol_wt(bp_k)
            elif component == "aromatics":
                mw = est.aromatic_mol_wt(bp_k)
            elif component == "resins":
                mw = est.resin_mol_wt(bp_k)
            elif component == "asphaltenes":
                mw = est.asphaltene_mol_wt(bp_k)
            else:
                raise ValueError(f"unknown component: {component}")

            molecular_weights.append(mw / 1000)
            cut_components.append(sara[sara_key])

        # Normalize the cut components to sum to cut_fraction
        total_cut = sum(cut_components)
        if total_cut > 0:
            for comp in cut_components:
                component_mass_fractions.append(comp / total_cut * cut_fraction)
        else:
            component_mass_fractions.extend([0.0] * 4)

    # Don't normalize the whole vector—each cut is already normalized
    return boiling_points_c, molecular_weights, component_mass_fractions

def extract_bullwinkle(oil, sara: dict[str, float]) -> tuple[float, bool]:
    """
    Use adios_db.computation.physical_properties.bullwinkle_fraction if available.
    """
    try:
        value = pp.bullwinkle_fraction(oil)
        if value <= 1.0:
            return value, False
    except Exception:
        pass

    # Fallback: use the same ADIOS2-style estimate as your Rust code.
    api = get_api(oil)
    f_asph = sara.get("asphaltenes", 0.0)

    t_g = 1356.7 - 247.36 * math.log(max(api, 1.0))
    t_bp = 532.98 - 3.1295 * api
    bull_adios_1 = clamp((483.0 - t_bp) / max(t_g, 1e-6), 0.0, 0.4)

    if f_asph > 0.0:
        bullwinkle = clamp(
            0.20219 - 0.168 * math.log10(max(f_asph, 1e-12)),
            0.0,
            0.0303,
        )
    elif api < 26.0:
        bullwinkle = 0.08
    elif api > 50.0:
        bullwinkle = 0.303
    else:
        bullwinkle = -1.038 - 0.78935 * math.log10(max(1.0 / api, 1e-12))

    return 0.5 * (bullwinkle + bull_adios_1), True


def lerp_value_first(points: list[tuple[float, float]], target_temp_c: float) -> float:
    if not points:
        return 850.0

    points = sorted(points, key=lambda pair: pair[1])

    if target_temp_c <= points[0][1]:
        return points[0][0]
    if target_temp_c >= points[-1][1]:
        return points[-1][0]

    for (v0, t0), (v1, t1) in zip(points, points[1:]):
        if t0 <= target_temp_c <= t1:
            if t1 == t0:
                return v0
            fraction = (target_temp_c - t0) / (t1 - t0)
            return v0 + fraction * (v1 - v0)

    return points[-1][0]


def deduplicate_temperature_points(
    points: list[tuple[float, float]],
) -> list[tuple[float, float]]:
    result = {}
    for value, temperature in points:
        result[temperature] = value
    return [
        (value, temperature)
        for temperature, value in sorted(result.items())
    ]


def convert_oil(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as file:
        raw = json.load(file)

    oil = ads.Oil.from_py_json(raw)

    metadata = get(oil, "metadata", default={})
    oil_id = str(
        get(oil, "oil_id", "id", default=None)
        or raw.get("oil_id")
        or raw.get("id")
        or path.stem
    )

    name = (
        get(metadata, "name", default=None)
        or get(raw.get("metadata", {}), "name", default=None)
        or path.stem
    )

    api = get_api(oil)

    density_points, density_estimated = extract_density_points(oil)
    viscosity_points, viscosity_estimated = extract_viscosity_points(oil, density_points)
    tension_points, tension_estimated = extract_interfacial_tension(oil)
    cuts, distillation_estimated = extract_distillation_cuts(oil)
    sara, sara_estimated = extract_sara(oil, density_points, viscosity_points)

    boiling_points_c, molecular_weights, component_mass_fractions = (
        extract_component_vectors(cuts, sara)
    )

    bullwinkle, bullwinkle_estimated = extract_bullwinkle(oil, sara)

    return {
        "oil_id": oil_id,
        "name": name,
        "product_type": get(
            metadata,
            "product_type",
            default=get(raw.get("metadata", {}), "product_type", default=None),
        ),
        "api_gravity": api,

        "density_kgm3": [
            [value, temperature_c]
            for value, temperature_c in density_points
        ],

        "dynamic_viscosity_cp": [
            [value, temperature_c]
            for value, temperature_c in viscosity_points
        ],

        "interfacial_tension_n_m": [
            [value, temperature_c]
            for value, temperature_c in tension_points
        ],

        "sara_mass_fractions": sara,

        "distillation_cuts": [
            {
                "cumulative_fraction": fraction,
                "vapor_temperature_c": temperature_c,
            }
            for fraction, temperature_c in cuts
        ],

        "boiling_points_c": boiling_points_c,
        "molecular_weights_kg_mol": molecular_weights,
        "component_mass_fractions": component_mass_fractions,

        "bullwinkle_fraction": bullwinkle,

        "estimated": {
            "density": density_estimated,
            "dynamic_viscosity": viscosity_estimated,
            "interfacial_tension": tension_estimated,
            "sara": sara_estimated,
            "distillation": distillation_estimated,
            "bullwinkle_fraction": bullwinkle_estimated,
        },

        "source_file": str(path),
    }


def build_catalog(input_dir: Path, output_path: Path) -> None:
    records = []
    skipped = []

    for path in sorted(input_dir.rglob("*.json")):
        try:
            record = convert_oil(path)
        except Exception as exc:
            skipped.append({
                "file": str(path),
                "error": f"{type(exc).__name__}: {exc}",
            })
            continue

        records.append(record)

    unique_records = {
        record["oil_id"]: record
        for record in records
    }

    oils = sorted(
        unique_records.values(),
        key=lambda record: (
            record.get("name", ""),
            record.get("oil_id", ""),
        ),
    )

    output = {
        "schema_version": 1,
        "source": "NOAA ADIOS Oil Database",
        "units": {
            "density": "kg/m^3",
            "temperature": "degC",
            "dynamic_viscosity": "cP",
            "interfacial_tension": "N/m",
            "molecular_weight": "g/mol",
            "mass_fraction": "1",
        },
        "tuple_convention": {
            "density_kgm3": "[value_kg_m3, temperature_c]",
            "dynamic_viscosity_cp": "[value_cp, temperature_c]",
            "interfacial_tension_n_m": "[value_n_m, temperature_c]",
        },
        "oils": oils,
        "statistics": {
            "oil_count": len(oils),
            "skipped_count": len(skipped),
        },
    }

    if skipped:
        output["skipped"] = skipped

    # Convert all numpy arrays / scalars and NaNs to JSON-safe types.
    output = to_json_safe(output)

    output_path.parent.mkdir(parents=True, exist_ok=True)

    with output_path.open("w", encoding="utf-8") as file:
        json.dump(output, file, indent=2, ensure_ascii=False)
        file.write("\n")

    print(f"Wrote {len(oils)} oils to {output_path}", file=sys.stderr)

    if skipped:
        print(
            f"Skipped {len(skipped)} files; inspect the JSON 'skipped' field.",
            file=sys.stderr,
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "input_dir",
        type=Path,
        help="Root directory of noaa-oil-data",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("oil_catalog.json"),
    )

    args = parser.parse_args()

    if not args.input_dir.is_dir():
        raise SystemExit(f"Not a directory: {args.input_dir}")

    build_catalog(args.input_dir, args.output)


if __name__ == "__main__":
    main()