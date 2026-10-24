"""reference.py extracts OpenDrift properties for independent catalog regression checks."""
import json
from importlib.resources import files

import numpy as np

WIND_FACTOR = 0.0
WIND_DEFLECTION = 0.0


def oil_properties(oil):
    """Use OpenOil's actual pseudo-components, rather than a separate catalog."""
    gnome = oil.gnome_oil
    temps = np.arange(-5.0, 40.5, 0.5)
    density = [float(oil.density_at_temp(t + 273.15)) for t in temps]
    viscosity = [float(oil.kvis_at_temp(t + 273.15)) * rho * 1000
                 for t, rho in zip(temps, density)]
    sara = np.asarray(gnome["sara_type"])
    fractions = np.asarray(oil.mass_fraction)
    limits = json.loads(files("opendrift.models.openoil.adios").joinpath(
        "max_water_fraction.json").read_text(encoding="utf-8")).get(oil.name)
    water_fraction_table = [] if limits is None else [
        [float(value), float(temp)] for value, temp in
        zip(limits["max_water_fraction"], limits["temperatures"])]
    return {
        "wind_factor": WIND_FACTOR, "wind_deflection": WIND_DEFLECTION,
        "product_type": str(oil.oil.metadata.product_type), "api": float(gnome["api"]),
        "density_kgm3": list(map(list, zip(density, temps.tolist()))),
        "dynamic_viscosity_cp": list(map(list, zip(viscosity, temps.tolist()))),
        "interfacial_tension_n_m": [[float(oil.oil_water_surface_tension()), 15.0]],
        "sara_mass_fractions": {"asphaltenes": float(fractions[sara == "Asphaltenes"].sum())},
        "boiling_points_c": (np.asarray(gnome["boiling_point"]) - 273.15).tolist(),
        "molecular_weights_kg_mol": (np.asarray(oil.molecular_weight) / 1000).tolist(),
        "component_mass_fractions": fractions.tolist(),
        "bullwinkle_fraction": float(oil.bullwinkle), "bulltime": float(oil.bulltime),
        "emulsion_water_fraction_max": float(oil.emulsion_water_fraction_max),
        "emulsion_water_fraction_max_by_temperature": water_fraction_table,
    }

