"""test_updates.py checks hourly publication decisions without network access."""

import io
import sys
import unittest
import tempfile
from datetime import date
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from botocore.exceptions import ClientError

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from scripts.forecast_updates import cmems_update, published_manifest
from scripts import check_forecast_update as check


class SourceUpdates(unittest.TestCase):
    """only completed updates from the current UTC date are eligible."""

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

    def test_stale_or_in_progress_updates_are_not_ready(self):
        self.assertIsNone(cmems_update(self.catalogue(), 'smoc', date(2026, 10, 29)))
        self.assertIsNone(cmems_update(self.catalogue(updating='2026-10-28'),
                                      'smoc', date(2026, 10, 28)))

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
