"""test_tiles.py checks validation tile halos and the encoded surface fields."""
import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np
import xarray as xr
import zstandard as zstd

spec = importlib.util.spec_from_file_location("validation_tiles", Path(__file__).with_name("download_testing_data.py"))
tiles = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tiles)


class TileTests(unittest.TestCase):
    def test_neighboring_tiles_share_two_columns(self):
        for step in (1 / 12, 0.25):
            coords = np.arange(-180, 180, step)
            left, _ = tiles.halo_indices(coords, -170, -160, True)
            right, _ = tiles.halo_indices(coords, -160, -150, True)
            np.testing.assert_array_equal(left[-2:], right[:2])
            self.assertEqual(len(left), round(10 / step) + 2)

    def test_dateline_wraps_and_latitude_stops_at_source_edge(self):
        coords = np.arange(-180, 180, 0.25)
        indices, origin = tiles.halo_indices(coords, -180, -170, True)
        self.assertEqual(indices[0], len(coords) - 1)
        self.assertEqual(origin, -180.25)
        indices, origin = tiles.halo_indices(np.arange(-80, 90.25, 0.25), -80, -70)
        self.assertEqual(indices[0], 0)
        self.assertEqual(origin, -80)
        self.assertEqual(len(indices), 41)

    def test_daily_encoding_preserves_nan_zero_and_halos(self):
        from datetime import datetime
        lons = np.arange(-180, 180, 0.25)
        lats = np.arange(-80, -69.5, 0.25)
        values = np.zeros((2, len(lats), len(lons)), dtype=np.float32)
        values[0, 5, 10] = np.nan
        field = xr.DataArray(values, dims=("time", "latitude", "longitude"))
        forcing = {k: field for k in ("u_day", "v_day", "u_wind", "v_wind", "sst")}
        forcing.update(cur_lons=lons, cur_lats=lats, wind_lons=lons, wind_lats=lats)
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(tiles, "TILES_DIR", Path(directory)), patch.object(tiles, "N_LON_TILES", 2), patch.object(tiles, "N_LAT_TILES", 1):
                self.assertEqual(tiles.tile_day(datetime(2026, 9, 19), forcing, 0), 2)
            raw = zstd.ZstdDecompressor().decompress((Path(directory) / "2026/09/19/000_000.bin").read_bytes())
        self.assertEqual(raw[:4], b"PRT3")
        lon, lat, wlon, wlat, times, width, height, layers, depth = struct.unpack_from("<ffffIIIIf", raw, 4)
        self.assertEqual((lon, lat, wlon, wlat), (-180.25, -80, -180.25, -80))
        self.assertEqual((times, width, height, layers, depth), (2, 42, 41, 1, 0))
        currents = np.frombuffer(raw, dtype="<f2", count=times * width * height * 2, offset=40)
        self.assertEqual(np.isnan(currents).sum(), 2)
        self.assertEqual(currents[0], 0)
        offset = 40 + currents.nbytes
        self.assertEqual(struct.unpack_from("<III", raw, offset), (42, 41, 2))
        self.assertEqual(len(raw), offset + 12 + 42 * 41 * 2 * 6)


if __name__ == "__main__":
    unittest.main()
