"""test_cron.py checks native tile boundaries, payloads, and date selection."""

import importlib.util
import json
import struct
import tempfile
import unittest
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

import numpy as np
import netCDF4
import zstandard as zstd


spec = importlib.util.spec_from_file_location('norkyst_cron', Path(__file__).resolve().parents[2] / 'norkyst_cron.py')
cron = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cron)


class NativeTiles(unittest.TestCase):
    """verify actual neighbor cells are shared across adjacent tile payloads."""

    def test_shared_halos_and_partial_edges(self):
        grid = np.arange(123 * 245, dtype=np.float32).reshape(1, 123, 245)
        left = cron.tile_array(grid[:, :121], 0, 0, (123, 245))
        right = cron.tile_array(grid[:, :121], 0, 120, (123, 245))
        np.testing.assert_array_equal(left[:, 1:, 120:122], right[:, 1:, :2])
        self.assertTrue(np.isnan(left[:, 0]).all())
        self.assertTrue(np.isnan(left[:, :, 0]).all())
        edge = cron.tile_array(grid[:, 119:], 120, 240, (123, 245))
        self.assertEqual(edge.shape, (1, 122, 122))
        np.testing.assert_array_equal(edge[:, :4, :6], grid[:, 119:, 239:].astype('<f2'))
        self.assertTrue(np.isnan(edge[:, 4:]).all())
        self.assertTrue(np.isnan(edge[:, :, 6:]).all())

    def test_payload_preserves_masks_zeros_and_field_order(self):
        arrays = [np.full((2, 122, 122), i, dtype='<f2') for i in range(5)]
        arrays[0][0, 0, 0] = np.nan
        header = dict(shape=[2, 122, 122], fields=list(cron.FIELDS))
        raw = zstd.ZstdDecompressor().decompress(cron.encode_tile(header, arrays))
        self.assertEqual(raw[:4], b'NKT1')
        length = struct.unpack_from('<I', raw, 4)[0]
        self.assertEqual(json.loads(raw[8:8 + length]), header)
        decoded = np.frombuffer(raw, dtype='<f2', offset=8 + length).reshape(5, 2, 122, 122)
        np.testing.assert_array_equal(decoded, np.stack(arrays))

    def test_generation_decodes_source_packing_and_converts_temperature(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with netCDF4.Dataset(root / 'source.nc', 'w') as dataset:
                for name, count in [('time', 2), ('depth', 1), ('Y', 3), ('X', 3)]:
                    dataset.createDimension(name, count)
                for field, name in cron.FIELDS.items():
                    dims = ('time', 'depth', 'Y', 'X') if field in ('u', 'v', 'temperature_k') else ('time', 'Y', 'X')
                    variable = dataset.createVariable(name, 'i2', dims, fill_value=-32767)
                    variable.scale_factor = 0.001
                    data = np.full(variable.shape, 10 if field == 'temperature_k' else 0, dtype=np.float32)
                    mask = np.zeros(variable.shape, dtype=bool)
                    mask[..., 1, 1] = True
                    variable[:] = np.ma.array(data, mask=mask)
                published = []

                def publish(path, kind):
                    published.append(path)

                publish.remote = False
                metadata = dict(grid_shape=[3, 3], grid_origin_m=[0, 0])
                details = cron.generate_day(dataset, metadata, 0, slice(0, 2), [0, 3600], root, publish)
            self.assertEqual(details['tiles'], ['000_000.bin'])
            raw = zstd.ZstdDecompressor().decompress(published[0].read_bytes())
            offset = 8 + struct.unpack_from('<I', raw, 4)[0]
            values = np.frombuffer(raw, dtype='<f2', offset=offset).reshape(5, 2, 122, 122)
            self.assertEqual(values[0, 0, 1, 1], 0)
            self.assertEqual(values[4, 0, 1, 1], np.float16(283.15))
            self.assertTrue(np.isnan(values[:, :, 2, 2]).all())

    def test_days_overlap_and_keep_partial_forecast(self):
        today = date(2026, 10, 23)
        start = int(datetime(2026, 10, 16, tzinfo=timezone.utc).timestamp())
        seconds = start + np.arange(9 * 24 + 7) * 3600
        windows = list(cron.day_windows(seconds, today))
        self.assertEqual(windows[0][0], today - timedelta(days=7))
        self.assertEqual(len(windows[-1][2]), 7)
        for left, right in zip(windows, windows[1:]):
            self.assertEqual(left[2][-1], right[2][0])
        with self.assertRaises(ValueError):
            list(cron.day_windows(np.delete(seconds, 5), today))
        with self.assertRaises(ValueError):
            list(cron.day_windows(seconds[1:], today))

    def test_cleanup_preserves_recent_future_and_unrelated_objects(self):
        class Client:
            def get_paginator(self, name):
                return self

            def paginate(self, **kwargs):
                return [{'Contents': [{'Key': key} for key in (
                    'norkyst/2026/10/15/000_000.bin', 'norkyst/2026/10/16/000_000.bin',
                    'norkyst/2026/10/27/manifest.json', 'norkyst/manifest.json')]}]

            def delete_objects(self, **kwargs):
                self.deleted = kwargs['Delete']['Objects']
                return {}

        client = Client()
        cron.cleanup(client, 'test', 'norkyst', date(2026, 10, 16))
        self.assertEqual(client.deleted, [{'Key': 'norkyst/2026/10/15/000_000.bin'}])


if __name__ == '__main__':
    unittest.main()
