"""gzip_oil.py compresses an oil catalog for publication to R2."""
import argparse
import gzip
from pathlib import Path


def main():
    """write a reproducible gzip file beside the input catalog."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("catalog", type=Path)
    parser.add_argument("-o", "--output", type=Path)
    args = parser.parse_args()
    output = args.output or args.catalog.with_suffix(args.catalog.suffix + ".gz")
    output.write_bytes(gzip.compress(args.catalog.read_bytes(), compresslevel=9, mtime=0))


if __name__ == "__main__":
    main()
