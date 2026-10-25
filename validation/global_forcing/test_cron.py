"""test_cron.py checks global tile sample counts and the cron's binary output."""

import ast
import contextlib
import io
import math
import struct
import tempfile
import unittest
from datetime import datetime, timedelta
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import zstandard as zstd


source = Path(__file__).resolve().parents[2] / 'daily_cron.py'
names = {'halo_indices', 'current_sample_count', 'encode_tile_header', 'tile_and_upload_day', 'ecmwf_steps', 'forecast_windows'}
tree = ast.parse(source.read_text(encoding='utf-8'))
functions = ast.Module(body=[item for item in tree.body if isinstance(item, ast.FunctionDef) and item.name in names], type_ignores=[])
cron = {'np': np, 'struct': struct, 'io': io, 'zstd': zstd, 'SMOC_HOURS_PER_DAY': 25,
        'math': math, 'timedelta': timedelta, 'WIND_STEP_HOURS': 6, 'ECMWF_FORECAST_HOURS': 360}
# load only pure tiling functions so tests cannot initialize credentials or contact R2
exec(compile(functions, str(source), 'exec'), cron)


class GlobalTiles(unittest.TestCase):
    def test_forecast_uses_source_endpoint_and_bracketing_wind(self):
        reference = datetime(2026, 10, 24)
        for hours in [239, 264, 300, 359, 360, 400]:
            with self.subTest(hours=hours):
                steps = cron['ecmwf_steps'](reference, reference + timedelta(hours=hours))
                self.assertEqual(steps, list(range(0, min(360, math.ceil(hours / 6) * 6) + 1, 6)))
                times = np.datetime64(reference) + np.arange(hours + 1) * np.timedelta64(1, 'h')
                windows = list(cron['forecast_windows'](times, reference, reference + timedelta(hours=steps[-1])))
                self.assertEqual(windows[-1][1][-1], np.datetime64(reference + timedelta(hours=min(hours, 360))))
                self.assertTrue(all(len(samples) == 25 for _, samples in windows[:-1]))
                self.assertGreaterEqual(len(windows[-1][1]), 2)
                if min(hours, 360) % 24:
                    self.assertEqual(len(windows[-1][1]), min(hours, 360) % 24 + 1)

    def test_forecast_rejects_missing_hours_and_caps_at_wind_endpoint(self):
        reference = datetime(2026, 10, 24)
        times = np.datetime64(reference) + np.arange(73) * np.timedelta64(1, 'h')
        windows = list(cron['forecast_windows'](times, reference, reference + timedelta(hours=54)))
        self.assertEqual([len(samples) for _, samples in windows], [25, 25, 7])
        for invalid in [np.delete(times, 12), times[1:], times[::-1]]:
            with self.assertRaises(ValueError):
                list(cron['forecast_windows'](invalid, reference, reference + timedelta(hours=72)))

    def test_hourly_timeline_and_partial_final_day(self):
        date = datetime(2026, 10, 25)
        for count in [2, 24, 25]:
            times = np.datetime64('2026-10-25') + np.arange(count) * np.timedelta64(1, 'h')
            self.assertEqual(cron['current_sample_count'](times, date), count)
        for times in [
            np.array(['2026-10-25'], dtype='datetime64[h]'),
            np.datetime64('2026-10-25') + np.arange(26) * np.timedelta64(1, 'h'),
            np.array(['2026-10-25T00', '2026-10-25T02'], dtype='datetime64[h]'),
            np.array(['2026-10-25T01', '2026-10-25T02'], dtype='datetime64[h]'),
        ]:
            with self.assertRaises(ValueError):
                cron['current_sample_count'](times, date)

    def test_actual_writer_preserves_partial_payload_and_wind_alignment(self):
        for count in [2, 19, 24, 25]:
            with self.subTest(count=count), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                date = datetime(2026, 10, 25)
                nc_dir = root / 'nc'
                nc_dir.mkdir()
                (nc_dir / 'smoc_20261025.nc').touch()
                fields = {
                    'longitude': SimpleNamespace(values=np.array([-180., -180. + 1 / 12])),
                    'latitude': SimpleNamespace(values=np.array([-80., -80. + 1 / 12])),
                    'time': SimpleNamespace(values=np.datetime64('2026-10-25') + np.arange(count) * np.timedelta64(1, 'h')),
                }
                u = np.broadcast_to(np.arange(count)[:, None, None], (count, 2, 2)).astype(np.float32).copy()
                u[0, 0, :] = np.nan
                for name, values in [('utotal', u), ('vtotal', -np.nan_to_num(u))]:
                    field = SimpleNamespace(values=values)
                    field.isel = lambda depth, field=field: field
                    fields[name] = field
                class Dataset:
                    def __getitem__(self, name): return fields[name]
                    def close(self): pass
                uploads = []
                cron.update({
                    'NC_DIR': nc_dir, 'TILES_DIR': root / 'tiles', 'TILE_SIZE': 10,
                    'N_LON_TILES': 1, 'N_LAT_TILES': 1, 'WIND_STEPS_PER_SMOC_DAY': 4,
                    'WIND_STEPS_PER_DAY': 5, 'BUCKET': 'test', 'shutil': __import__('shutil'), 'os': __import__('os'),
                    'xr': SimpleNamespace(open_dataset=lambda _: Dataset()),
                    's3': SimpleNamespace(upload_file=lambda path, *_: uploads.append(Path(path).read_bytes())),
                })
                wind = {
                    'reference_date': date, 'lons': np.array([-180., -179.75]), 'lats': np.array([-79.75, -80.]),
                    'u_wind': np.full((5, 2, 2), 12, dtype=np.float32),
                    'v_wind': np.full((5, 2, 2), 13, dtype=np.float32),
                    'sst': np.full((5, 2, 2), 280, dtype=np.float32),
                }
                wind_steps = math.ceil((count - 1) / 6) + 1
                for name in ['u_wind', 'v_wind', 'sst']:
                    wind[name] = wind[name][:wind_steps]
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(cron['tile_and_upload_day'](date, None, wind), 1)
                payload = zstd.ZstdDecompressor().decompress(uploads[0])
                self.assertEqual(payload[:4], b'PRT3')
                self.assertEqual(struct.unpack_from('<I', payload, 20)[0], count)
                self.assertEqual(struct.unpack_from('<III', payload, 24), (4, 2, 1))
                self.assertTrue(np.isnan(np.frombuffer(payload, dtype='<f2', count=4, offset=40)[0]))
                self.assertEqual(np.frombuffer(payload, dtype='<f2', count=4, offset=40 + (count - 1) * 32)[0], count - 1)
                wind_offset = 40 + count * 32
                self.assertEqual(struct.unpack_from('<III', payload, wind_offset), (4, 2, wind_steps))
                self.assertEqual(np.frombuffer(payload, dtype='<f2', count=4, offset=wind_offset + 12)[0], 12)
                self.assertEqual(len(payload), wind_offset + 12 + wind_steps * 8 * 6)


if __name__ == '__main__':
    unittest.main()
