"""test_build_oil_catalog.py compares catalog records with validation inputs."""
import importlib.util
import json
import lzma
import unittest
from importlib.resources import files
from pathlib import Path

import numpy as np
from opendrift.models.openoil.adios.oil import OpendriftOil

from build_oil_catalog import convert_record, load_water_fraction_limits, opendrift_name


class CatalogTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        """load independent OpenDrift references and the validation extractor."""
        resource = files("opendrift.models.openoil.adios").joinpath("oils.xz")
        with lzma.open(resource, "rt", encoding="utf-8") as stream:
            cls.records = json.load(stream)
        cls.limits = load_water_fraction_limits()
        path = Path(__file__).resolve().parents[2] / "validation/oil/run.py"
        spec = importlib.util.spec_from_file_location("oil_validation", path)
        cls.validation = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.validation)

    def test_properties_match_validation_for_multiple_oils(self):
        """compare every weathering input for generic and Norwegian oils."""
        selected = [r for r in self.records if r["oil_id"] in
                    {"GN00002", "GN00004", "GN00006", "AD04001"}]
        selected += [r for r in self.records if opendrift_name(r) in self.limits][:3]
        self.assertGreaterEqual(len(selected), 7)
        for raw in selected:
            with self.subTest(oil=raw["oil_id"]):
                raw = json.loads(json.dumps(raw))
                raw["metadata"]["name"] = opendrift_name(raw)
                reference = self.validation.oil_properties(OpendriftOil(raw))
                actual = convert_record(raw, "test", self.limits)
                for key in ("density_kgm3", "dynamic_viscosity_cp", "interfacial_tension_n_m",
                            "boiling_points_c", "molecular_weights_kg_mol", "component_mass_fractions",
                            "bullwinkle_fraction", "bulltime", "emulsion_water_fraction_max",
                            "emulsion_water_fraction_max_by_temperature"):
                    np.testing.assert_allclose(actual[key], reference[key], rtol=0, atol=0, err_msg=key)
                self.assertEqual(actual["api_gravity"], reference["api"])
                self.assertEqual(actual["sara_mass_fractions"]["asphaltenes"],
                                 reference["sara_mass_fractions"]["asphaltenes"])

    def test_norwegian_limits_preserve_temperature_and_zero_values(self):
        """retain measured limits without replacing zero with a default."""
        raw = next(r for r in self.records if opendrift_name(r) == "AASGARD A 2003")
        actual = convert_record(raw, "test", self.limits)
        self.assertEqual(actual["emulsion_water_fraction_max_by_temperature"], [[0.46, 5.0], [0.51, 15.0]])
        limits = {opendrift_name(raw): {"temperatures": [15], "max_water_fraction": [0]}}
        self.assertEqual(convert_record(raw, "test", limits)["emulsion_water_fraction_max_by_temperature"], [[0, 15]])


if __name__ == "__main__":
    unittest.main()
