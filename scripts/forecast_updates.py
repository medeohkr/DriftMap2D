"""forecast_updates.py checks completed source updates against published manifests."""

import json
import logging
from datetime import datetime, timezone


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
    if updated.date() < today:
        logging.info('CMEMS latest completed update is %s, checking again next hour', updated)
        return None
    if updated.date() > today:
        raise ValueError('CMEMS update timestamp is in the future')
    return {'dataset_id': dataset_id, 'dataset_version': version.label,
            'arco_updated_at': updated.isoformat()}
