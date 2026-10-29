"""test_suite.py checks release alignment and the local mean dilation reference."""

import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

from run import mean_dilation, read_cli
from matched_placement import segment_crossing


class StrandingSuiteTests(unittest.TestCase):
    def test_matched_placement_stays_on_the_segment_and_refines_the_first_crossing(self):
        point = segment_crossing((0, 0), (0.02, 0.01), lambda x, y: x >= 0.007)
        self.assertGreaterEqual(point[0], 0.007)
        self.assertLess(point[0], 0.0071)
        self.assertAlmostEqual(point[1], point[0] / 2)
        point = segment_crossing((0, 0), (0.02, 0),
                                 lambda x, y: 0.003 <= x <= 0.006 or x >= 0.015)
        self.assertGreaterEqual(point[0], 0.003)
        self.assertLess(point[0], 0.0031)

    def test_births_use_the_start_of_the_step_and_hourly_output_keeps_steps(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with (directory / "particles.jsonl").open("w") as output:
                for step in range(193):
                    ids = range(min(step + 1, 3))
                    output.write(json.dumps({"step": step, "particles": [
                        {"id": identity, "stranded": step >= 8 and identity == 0}
                        for identity in ids]}) + "\n")
            births, counts = read_cli(directory, 3)
            self.assertEqual(births.tolist(), [0, 0, 1])
            self.assertEqual(len(counts), 49)
            self.assertEqual(counts[2]["proteus_stranded"], 1)
            hourly = [json.loads(line) for line in
                      (directory / "particles_hourly.jsonl").read_text().splitlines()]
            self.assertEqual([row["step"] for row in hourly], list(range(0, 193, 4)))

    def test_mean_dilation_counts_zero_and_expands_only_one_cell(self):
        data = np.array([[-2, np.nan, np.nan], [2, np.nan, np.nan]], dtype=np.float32)
        mean_dilation(data)
        np.testing.assert_equal(data, [[-2, 0, np.nan], [2, 0, np.nan]])
        mean_dilation(data)
        np.testing.assert_equal(data, [[-2, 0, 0], [2, 0, 0]])


if __name__ == "__main__":
    unittest.main()
