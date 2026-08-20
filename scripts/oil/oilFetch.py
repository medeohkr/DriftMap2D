
"""
Fetch ADIOS oil data and export as JSON files.
Run this script once to get all the oil JSON files for your project.
"""

import itertools
import json
import lzma
import os
from pathlib import Path

OILS_XZ = Path("web/assets/oils/oils.xz")
OILS_DIR = Path("web/assets/oils/")
OILS_DIR.mkdir(parents=True, exist_ok=True)

# Oils you want to fetch
OILS_TO_FETCH = [
    "Arabian Light",      # AD04001
    "Bonny Light",        # AD04002  
    "Marine Diesel",      # EC03001
    "Bachaquero",   # AD04005
    "IFO-380",            # EC01001
]

def get_archive():
    with lzma.open(OILS_XZ, "r") as archive:
        oils = json.load(archive)
    return oils

def fetch_oils(limit=50, name=''):
    oils = filter (
        lambda oil: name in oil['metadata']['name'],
        get_archive()
    )
    return list(itertools.islice(oils, limit))

def write_oil_json(oil):
    name = oil['metadata']['name']
    filename = f"{name}.json"
    filepath = OILS_DIR / filename
    with open(filepath, "w") as f:
        json.dump(oil, f, indent=2)
    print(f"Wrote {filepath}")

def main():
    found = 0
    for oil in OILS_TO_FETCH:
        print(f"printing assay for {oil}")
        if len(fetch_oils(name=oil)) != 0:
            print(f"{oil} found!\n")
            write_oil_json(fetch_oils(name=oil)[0])
            found += 1
        else:
            print(f"{oil} not found!\n")

    print(found)

if __name__ == "__main__":
    main()