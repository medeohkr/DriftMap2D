"""forecast_updates.py checks completed source updates against published manifests."""

import json
import logging
from datetime import datetime, timedelta, timezone


def published_manifest(client, bucket, key):
    """treat only a missing manifest as an unpublished source."""
    from botocore.exceptions import ClientError

    try:
        return json.loads(client.get_object(Bucket=bucket, Key=key)['Body'].read())
    except ClientError as error:
        if error.response['Error']['Code'] in ('NoSuchKey', '404'):
            return {}
        raise


def cmems_update(catalogue, dataset_id, today):
    """identify a completed ARCO update without treating valid times as a cycle."""
    datasets = [dataset for product in catalogue.products for dataset in product.datasets
                if dataset.dataset_id == dataset_id]
    if len(datasets) != 1 or not datasets[0].versions:
        raise ValueError('CMEMS catalogue has no unique dataset version')
    version = max(datasets[0].versions, key=lambda item: item.label)
    parts = [part for part in version.parts if part.name == 'default']
    if len(parts) != 1:
        raise ValueError('CMEMS catalogue has no unique default part')
    part = parts[0]
    if part.arco_updating_start_date:
        logging.info('CMEMS update in progress, checking again next hour')
        return None
    if not part.arco_updated_date:
        raise ValueError('CMEMS completed update timestamp is missing')
    updated = datetime.fromisoformat(part.arco_updated_date.replace('Z', '+00:00'))
    if updated.tzinfo is None:
        raise ValueError('CMEMS update timestamp has no timezone')
    updated = updated.astimezone(timezone.utc)
    if updated.date() > today:
        raise ValueError('CMEMS update timestamp is in the future')
    return {'dataset_id': dataset_id, 'dataset_version': version.label,
            'arco_updated_at': updated.isoformat()}


def newer_source_update(source, published):
    """publish completed revisions newer than the manifest regardless of UTC midnight."""
    if source is None:
        return False
    if not published:
        return True
    current = datetime.fromisoformat(source['arco_updated_at'].replace('Z', '+00:00'))
    previous = datetime.fromisoformat(published['arco_updated_at'].replace('Z', '+00:00'))
    return current > previous or (current == previous and source != published)


def norkyst_cycle(reference):
    """restore the midnight cycle after source timestamp quantization."""
    if reference.tzinfo is None:
        raise ValueError('NorKyst forecast reference has no timezone')
    reference = reference.astimezone(timezone.utc)
    midnight = reference.replace(hour=0, minute=0, second=0, microsecond=0)
    if reference.hour >= 12:
        midnight += timedelta(days=1)
    if abs(reference - midnight) > timedelta(hours=2):
        raise ValueError(f'NorKyst forecast reference is not near midnight: {reference.isoformat()}')
    return midnight


def norkyst_update_available(source, reference, latest_valid_time, published):
    """detect a new cycle or additional hours from the published cycle."""
    grid = published.get('grid', {})
    previous = grid.get('source_forecast_reference_time')
    if grid.get('source') != source or not previous:
        return True
    current_cycle = norkyst_cycle(reference)
    previous_cycle = norkyst_cycle(datetime.fromisoformat(previous.replace('Z', '+00:00')))
    if current_cycle != previous_cycle:
        return current_cycle > previous_cycle
    previous_last = grid.get('source_last_time')
    if previous_last is None:
        days = published.get('days', [])
        previous_last = max((max(day['timestamps']) for day in days), default=None)
    return previous_last is None or latest_valid_time > int(previous_last)
