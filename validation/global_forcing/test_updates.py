"""test_updates.py checks hourly publication decisions without network access."""

import io
import sys
import unittest
import tempfile
from datetime import date, datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from botocore.exceptions import ClientError

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from scripts.forecast_updates import cmems_update, published_manifest, newer_source_update, newer_norkyst_cycle
from scripts import check_forecast_update as check


class SourceUpdates(unittest.TestCase):
    """completed updates remain eligible after midnight without rolling back revisions."""

    def catalogue(self, updated='2026-10-28T09:00:00Z', updating=None):
        part = SimpleNamespace(name='default', arco_updated_date=updated,
                               arco_updating_start_date=updating)
        version = SimpleNamespace(label='202211', parts=[part])
        dataset = SimpleNamespace(dataset_id='smoc', versions=[version])
        return SimpleNamespace(products=[SimpleNamespace(datasets=[dataset])])

    def test_completed_update_has_stable_identity(self):
        result = cmems_update(self.catalogue(), 'smoc', date(2026, 10, 28))
        self.assertEqual(result, {'dataset_id': 'smoc', 'dataset_version': '202211',
                                  'arco_updated_at': '2026-10-28T09:00:00+00:00'})

    def test_previous_day_updates_remain_eligible_but_in_progress_updates_do_not(self):
        self.assertIsNotNone(cmems_update(self.catalogue(), 'smoc', date(2026, 10, 29)))
        self.assertIsNone(cmems_update(self.catalogue(updating='2026-10-28'),
                                      'smoc', date(2026, 10, 28)))

    def test_delayed_updates_publish_once_and_never_roll_back(self):
        source = cmems_update(self.catalogue(), 'smoc', date(2026, 10, 29))
        old = dict(source, arco_updated_at='2026-10-27T09:00:00+00:00')
        self.assertTrue(newer_source_update(source, old))
        self.assertFalse(newer_source_update(source, source))
        self.assertFalse(newer_source_update(old, source))
        reference = datetime(2026, 10, 28, tzinfo=timezone.utc)
        manifest = {'grid': {'source': 'norkyst',
                            'source_forecast_reference_time': '2026-10-27T00:00:00+00:00'}}
        self.assertTrue(newer_norkyst_cycle('norkyst', reference, manifest))
        manifest['grid']['source_forecast_reference_time'] = reference.isoformat()
        self.assertFalse(newer_norkyst_cycle('norkyst', reference, manifest))
        self.assertFalse(newer_norkyst_cycle('norkyst', reference.replace(day=27), manifest))

    def test_missing_future_or_ambiguous_metadata_fails(self):
        for updated in (None, '2026-10-29T00:00:00Z', '2026-10-28T00:00:00'):
            with self.subTest(updated=updated), self.assertRaises(ValueError):
                cmems_update(self.catalogue(updated), 'smoc', date(2026, 10, 28))
        with self.assertRaises(ValueError):
            cmems_update(self.catalogue(), 'different', date(2026, 10, 28))

    def test_only_missing_published_manifest_is_ignored(self):
        client = Mock()
        client.get_object.return_value = {'Body': io.BytesIO(b'{"source_update": {}}')}
        self.assertEqual(published_manifest(client, 'bucket', 'manifest'), {'source_update': {}})
        client.get_object.side_effect = ClientError({'Error': {'Code': 'NoSuchKey'}}, 'GetObject')
        self.assertEqual(published_manifest(client, 'bucket', 'manifest'), {})
        client.get_object.side_effect = ClientError({'Error': {'Code': 'AccessDenied'}}, 'GetObject')
        with self.assertRaises(ClientError):
            published_manifest(client, 'bucket', 'manifest')

    def test_workflow_output_skips_unchanged_and_incomplete_updates(self):
        source = {'dataset_id': 'smoc', 'dataset_version': '202211',
                  'arco_updated_at': '2026-10-28T09:00:00+00:00'}
        for current, previous, expected in ((None, {}, 'false'),
                                            (source, {'source_update': source}, 'false'),
                                            (source, {}, 'true')):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / 'output'
                environment = {'ACCOUNT_ID': 'test', 'ACCESS_KEY': 'test',
                               'SECRET_KEY': 'test', 'GITHUB_OUTPUT': str(output)}
                with patch.dict('os.environ', environment), \
                        patch.object(sys, 'argv', ['check', 'global']), \
                        patch.dict(sys.modules, {'copernicusmarine': Mock()}), \
                        patch.object(check.boto3, 'client', return_value=Mock()), \
                        patch.object(check, 'cmems_update', return_value=current), \
                        patch.object(check, 'published_manifest', return_value=previous):
                    check.main()
                self.assertEqual(output.read_text(), f'ready={expected}\n')


if __name__ == '__main__':
    unittest.main()
