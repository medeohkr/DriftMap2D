"""test_cron.py checks native tile boundaries, payloads, and date selection."""

import importlib.util
import json
import struct
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

import numpy as np
import netCDF4
import zstandard as zstd


spec = importlib.util.spec_from_file_location('norkyst_cron', Path(__file__).resolve().parents[2] / 'norkyst_cron.py')
cron = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cron)


class ForecastFreshness(unittest.TestCase):
    """reject stale cycles before any tile generation or publishing."""

    @staticmethod
    def dataset(reference):
        class Variable:
            units = 'hours since 2026-10-27 00:00:00'

            def __getitem__(self, index):
                return np.ma.asarray(reference)
        return SimpleNamespace(variables={'forecast_reference_time': Variable()}, close=Mock())

    def test_reference_decoding(self):
        self.assertEqual(cron.forecast_reference_time(self.dataset(24)),
                         datetime(2026, 10, 28, tzinfo=timezone.utc))
        for invalid in (np.nan, [0, 24], np.ma.masked):
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                cron.forecast_reference_time(self.dataset(invalid))
        with self.assertRaisesRegex(ValueError, 'no forecast_reference_time'):
            cron.forecast_reference_time(SimpleNamespace(variables={}))

    def test_current_cycle_stays_open(self):
        dataset = self.dataset(24)
        with patch.object(cron.netCDF4, 'Dataset', return_value=dataset):
            self.assertIs(cron.open_fresh_source('source', date(2026, 10, 28)), dataset)
        dataset.close.assert_not_called()

    def test_stale_cycle_fails_without_waiting(self):
        dataset = self.dataset(0)
        with patch.object(cron.netCDF4, 'Dataset', return_value=dataset), \
                patch.object(cron.time, 'sleep') as sleep:
            with self.assertRaisesRegex(RuntimeError, 'no tiles published'):
                cron.open_fresh_source('source', date(2026, 10, 28), wait_seconds=0)
        dataset.close.assert_called_once()
        sleep.assert_not_called()

    def test_stale_source_is_closed_and_reopened(self):
        stale, fresh = self.dataset(0), self.dataset(24)
        with patch.object(cron.netCDF4, 'Dataset', side_effect=[stale, fresh]) as opened, \
                patch.object(cron.time, 'sleep') as sleep, \
                patch.object(cron.time, 'monotonic', return_value=0):
            result = cron.open_fresh_source('source', date(2026, 10, 28))
        self.assertIs(result, fresh)
        self.assertEqual(opened.call_count, 2)
        stale.close.assert_called_once()
        fresh.close.assert_not_called()
        sleep.assert_called_once_with(900)


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

    def test_reordered_forecasts_preserve_their_source_data_indices(self):
        today = date(2026, 10, 23)
        start = int(datetime(2026, 10, 16, tzinfo=timezone.utc).timestamp())
        original = start + np.arange(10 * 24 + 1) * 3600
        order = np.r_[np.arange(9 * 24, len(original)), np.arange(9 * 24)]
        source = original[order]
        windows = list(cron.day_windows(source, today))
        for _, window, times in windows:
            np.testing.assert_array_equal(source[window], times)
        self.assertEqual(len(windows), 10)
        self.assertEqual(windows[8][2][-1], windows[9][2][0])

    def test_duplicate_times_select_last_record_and_netcdf_reads_match(self):
        today = date(2026, 10, 23)
        start = int(datetime(2026, 10, 16, tzinfo=timezone.utc).timestamp())
        original = start + np.arange(9 * 24 + 1) * 3600
        # append a replacement for one hour in the first day
        source = np.r_[original, original[12]]
        windows = list(cron.day_windows(source, today))
        first = windows[0][1]
        self.assertIsInstance(first, np.ndarray)
        self.assertEqual(first[12], len(source) - 1)
        with tempfile.TemporaryDirectory() as temporary:
            with netCDF4.Dataset(Path(temporary) / 'source.nc', 'w') as dataset:
                dataset.createDimension('time', len(source))
                variable = dataset.createVariable('value', 'i4', ('time',))
                variable[:] = np.arange(len(source))
                np.testing.assert_array_equal(cron.read(variable, first), first)
                for name, count in [('depth', 1), ('Y', 2), ('X', 3)]:
                    dataset.createDimension(name, count)
                current = dataset.createVariable('current', 'f4', ('time', 'depth', 'Y', 'X'))
                current[:] = np.broadcast_to(np.arange(len(source))[:, None, None, None], current.shape)
                samples = cron.read(current, (first, 0, slice(None), slice(None)))
                self.assertEqual(samples.shape, (25, 2, 3))
                np.testing.assert_array_equal(samples[:, 0, 0], first)
        self.assertIsInstance(windows[2][1], slice)

    def test_reordering_does_not_hide_missing_hours(self):
        today = date(2026, 10, 23)
        start = int(datetime(2026, 10, 16, tzinfo=timezone.utc).timestamp())
        source = start + np.arange(10 * 24 + 1) * 3600
        with self.assertRaisesRegex(ValueError, 'missing hourly samples'):
            list(cron.day_windows(np.delete(source, 12)[::-1], today))


class NativeCoverage(unittest.TestCase):
    def metadata(self):
        path = Path(__file__).resolve().parents[2] / 'crates/proteus-core/src/forcing/norkyst/fixtures/manifest.json'
        grid = json.loads(path.read_text())['grid']
        return dict(grid, grid_shape=[9, 9])

    def test_rows_roundtrip_and_reject_invalid_intervals(self):
        mask = np.array([[True, False, True], [False, False, False], [True, True, True]])
        rows = cron.coverage_rows(mask)
        self.assertEqual(rows, [[[0, 1], [2, 3]], [], [[0, 3]]])
        np.testing.assert_array_equal(cron.coverage_mask(rows, mask.shape), mask)
        for invalid in [[[[0, 4]]], [[[2, 1]]], [[[0, 2], [1, 3]]], [[[-1, 1]]]]:
            with self.assertRaises(ValueError):
                cron.coverage_mask(invalid, (1, 3))

    def test_zero_currents_and_temporary_missing_samples_keep_coverage(self):
        u = np.zeros((2, 1, 9, 9), dtype=np.float32)
        v = u.copy()
        u[0, 0, 1, 1] = np.nan
        u[:, :, 4:, 4:] = np.nan
        dataset = SimpleNamespace(variables={'u_eastward': u, 'v_northward': v})
        observed = cron.read_current_coverage(dataset, self.metadata(), 0, slice(0, 2))
        self.assertTrue(observed[1, 1])
        self.assertTrue(observed[0, 0])
        self.assertFalse(observed[8, 8])

    def test_hatching_excludes_land_and_keeps_a_single_coastal_buffer(self):
        class Landmask:
            def __init__(self, land):
                self.land = land

            def contains_many(self, lon, lat):
                return np.full(len(lon), self.land, dtype=bool)

        observed = np.zeros((9, 9), dtype=bool)
        observed[:, :3] = True
        ocean = cron.make_coverage(self.metadata(), observed, Landmask(False))
        valid = cron.coverage_mask(ocean['valid_rows'], (8, 8))
        water = cron.coverage_mask(ocean['unavailable_water_rows'], (8, 8))
        self.assertTrue(valid[:, :3].all())
        self.assertFalse(valid[:, 3:].any())
        np.testing.assert_array_equal(water, ~valid)
        land = cron.make_coverage(self.metadata(), observed, Landmask(True))
        self.assertFalse(cron.coverage_mask(land['unavailable_water_rows'], (8, 8)).any())
        with self.assertRaises(ValueError):
            cron.make_coverage(self.metadata(), np.zeros_like(observed), Landmask(False))


class RemoteReads(unittest.TestCase):
    class Variable:
        def __init__(self, limit, unavailable=False):
            data = np.arange(8 * 2 * 4, dtype=np.float32).reshape(8, 2, 4)
            self.data = np.ma.array(data, mask=np.zeros_like(data, dtype=bool))
            self.data.mask[3, 1, 2] = True
            self.shape, self.ndim = data.shape, data.ndim
            self.name = 'u_eastward'
            self.limit, self.unavailable = limit, unavailable
            self.requests = []

        def __getitem__(self, index):
            if any(isinstance(s, (list, np.ndarray)) for s in index):
                raise AssertionError('remote requests must use contiguous slices')
            result = self.data[index]
            self.requests.append(result.size)
            if self.unavailable or result.size > self.limit:
                raise RuntimeError('NetCDF: Access failure')
            return result

    def test_large_reads_are_split_before_contacting_server(self):
        variable = self.Variable(8)
        with patch.object(cron, 'MAX_READ_VALUES', 8), patch.object(cron.time, 'sleep') as sleep:
            result = cron.read(variable, (slice(None), 1, slice(None)))
        np.testing.assert_array_equal(result, variable.data[:, 1, :].filled(np.nan))
        self.assertTrue(all(size <= 8 for size in variable.requests))
        sleep.assert_not_called()

    def test_persistent_large_failures_retry_smaller_requests(self):
        variable = self.Variable(8)
        with patch.object(cron.time, 'sleep'), self.assertLogs(level='WARNING'):
            result = cron.read(variable, (slice(None), 1, slice(None)))
        np.testing.assert_array_equal(result, variable.data[:, 1, :].filled(np.nan))
        self.assertTrue(np.isnan(result[3, 2]))
        self.assertIn(32, variable.requests)
        self.assertIn(8, variable.requests)

    def test_reordered_times_are_read_as_runs_and_keep_output_order(self):
        variable = self.Variable(100)
        indices = np.array([4, 5, 1, 2, 0])
        result = cron.read(variable, (indices, 1, slice(None)))
        np.testing.assert_array_equal(result, variable.data[indices, 1, :].filled(np.nan))
        self.assertEqual(variable.requests, [8, 8, 4])

    def test_unavailable_source_fails_with_field_context(self):
        variable = self.Variable(100, unavailable=True)
        with patch.object(cron.time, 'sleep'), patch.object(cron, 'MAX_READ_SPLITS', 1), self.assertLogs(level='WARNING'):
            with self.assertRaisesRegex(RuntimeError, 'u_eastward.*after retries and smaller requests'):
                cron.read(variable, (slice(None), 1, slice(None)))
        self.assertLessEqual(len(variable.requests), 8)


if __name__ == '__main__':
    unittest.main()
