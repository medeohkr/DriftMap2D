#!/usr/bin/env python3
"""Download 10-day ECMWF forecast — 6-hourly with SST."""

from ecmwf.opendata import Client
from datetime import datetime, timedelta, timezone

client = Client(source="aws")

now = datetime.now(timezone.utc)
yesterday = now - timedelta(days=1)
date_str = yesterday.strftime("%Y-%m-%d")

print(f"Downloading {date_str} 12Z — 6-hourly, 10 days...")
client.retrieve(
    date=date_str,
    time="12",
    step=list(range(0, 240, 6)),
    param=["10u", "10v", "skt"],  # skt = skin temperature (SST over ocean)
    target=f"data/ecmwf_{date_str.replace('-', '')}_12z.grib2",
    type="fc",
    levtype="sfc",
)

print(f"✅ Downloaded: {date_str} 12Z, 40 time steps")