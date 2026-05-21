#!/usr/bin/env python3
"""
upload_to_r2.py - Upload SMOC forecast tiles to Cloudflare R2
pip install boto3
"""

import boto3
from pathlib import Path
import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed

# ===== YOUR CREDENTIALS =====
ACCOUNT_ID = "d733df1598b4cde0f885a7ef0db5ccb8"
ACCESS_KEY = "036e8c8eeb522c92c85bea2783ee28f1"
SECRET_KEY = "4d92d6cda85083ffb841da100b49aa8ccb9609ac7ccd169ca678730bcde5ba32"
BUCKET_NAME = "driftmap-tiles"

# ===== PATHS =====
LOCAL_DIR = Path(r"D:\projects\driftmap\data\roaring_landmask_tiled")
R2_PREFIX = "roaring_landmask"

# ===== CONNECT =====
s3 = boto3.client(
    's3',
    endpoint_url=f'https://{ACCOUNT_ID}.r2.cloudflarestorage.com',
    aws_access_key_id=ACCESS_KEY,
    aws_secret_access_key=SECRET_KEY,
    region_name='auto',
)


def test_connection():
    try:
        buckets = [b['Name'] for b in s3.list_buckets()['Buckets']]
        print(f"✅ Connected! Buckets: {buckets}")
        return True
    except Exception as e:
        print(f"❌ Connection failed: {e}")
        return False


def upload_file(local_path, r2_key):
    try:
        s3.upload_file(str(local_path), BUCKET_NAME, r2_key)
        return True
    except Exception as e:
        print(f"  ❌ {r2_key}: {e}")
        return False


def upload_all():
    files = []
    for root, dirs, filenames in os.walk(LOCAL_DIR):
        for fname in filenames:
            if fname.endswith('.bin'):
                local = Path(root) / fname
                rel = str(local.relative_to(LOCAL_DIR)).replace('\\', '/')
                r2_key = f"{R2_PREFIX}/{rel}"
                files.append((local, r2_key))
    
    print(f"Uploading {len(files)} files...")
    
    uploaded = 0
    with ThreadPoolExecutor(max_workers=24) as executor:
        futures = {executor.submit(upload_file, l, r): r for l, r in files}
        for future in as_completed(futures):
            if future.result():
                uploaded += 1
            if uploaded % 500 == 0:
                print(f"  {uploaded}/{len(files)}")
    
    print(f"✅ Done: {uploaded}/{len(files)}")


def upload_recent(days=1):
    import time
    cutoff = time.time() - (days * 86400)
    
    files = []
    for root, dirs, filenames in os.walk(LOCAL_DIR):
        for fname in filenames:
            if fname.endswith('.bin'):
                local = Path(root) / fname
                if local.stat().st_mtime > cutoff:
                    rel = str(local.relative_to(LOCAL_DIR)).replace('\\', '/')
                    files.append((local, f"{R2_PREFIX}/{rel}"))
    
    print(f"Uploading {len(files)} files from last {days} day(s)...")
    
    uploaded = 0
    with ThreadPoolExecutor(max_workers=24) as executor:
        futures = {executor.submit(upload_file, l, r): r for l, r in files}
        for future in as_completed(futures):
            if future.result():
                uploaded += 1
    
    print(f"✅ Done: {uploaded}/{len(files)}")


if __name__ == "__main__":
    if not test_connection():
        print("\nCheck your ACCESS_KEY and SECRET_KEY in the script.")
        sys.exit(1)
    
    if len(sys.argv) > 1 and sys.argv[1] == "--recent":
        days = int(sys.argv[2]) if len(sys.argv) > 2 else 1
        upload_recent(days)
    else:
        upload_all()