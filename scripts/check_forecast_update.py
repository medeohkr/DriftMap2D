"""check_forecast_update.py gates expensive workflow steps on source freshness."""

import argparse
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import boto3

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.forecast_updates import cmems_update, published_manifest, newer_source_update, norkyst_update_available


def main():
    """emit a GitHub Actions output without generating or publishing tiles."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('model', choices=['global', 'norkyst'])
    args = parser.parse_args()
    client = boto3.client('s3', region_name='auto',
                          endpoint_url=f"https://{os.environ['ACCOUNT_ID']}.r2.cloudflarestorage.com",
                          aws_access_key_id=os.environ['ACCESS_KEY'],
                          aws_secret_access_key=os.environ['SECRET_KEY'])
    today = datetime.now(timezone.utc).date()
    if args.model == 'global':
        import copernicusmarine
        dataset_id = 'cmems_mod_glo_phy_anfc_merged-uv_PT1H-i'
        catalogue = copernicusmarine.describe(dataset_id=dataset_id, disable_progress_bar=True)
        source = cmems_update(catalogue, dataset_id, today)
        previous = published_manifest(client, 'driftmap-tiles', 'tiles/manifest.json') if source else {}
        ready = newer_source_update(source, previous.get('source_update'))
    else:
        import netCDF4
        from norkyst_cron import SOURCE, forecast_reference_time, source_timeline
        with netCDF4.Dataset(SOURCE) as dataset:
            reference = forecast_reference_time(dataset)
            latest_valid_time = int(source_timeline(dataset).max())
        if reference.date() > today:
            raise ValueError('NorKyst forecast reference date is in the future')
        previous = published_manifest(client, 'driftmap-tiles', 'norkyst/manifest.json')
        ready = norkyst_update_available(SOURCE, reference, latest_valid_time, previous)
    print(f'{args.model} new completed update available={ready}')
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            output.write(f"ready={'true' if ready else 'false'}\n")


if __name__ == '__main__':
    main()
