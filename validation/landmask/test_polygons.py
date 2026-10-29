"""test_polygons.py checks clipping semantics and regenerates the Rust fixture."""

import json
import sys
import unittest
from pathlib import Path

import zstandard
from shapely.geometry import Polygon, Point
from shapely import box

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from build_landmask_polygons import encode_tile


def fixture_polygon():
    return Polygon([(0, 0), (.4, 0), (.4, .2), (0, .2), (0, 0)],
                   holes=[[(.05, .05), (.15, .05), (.15, .15), (.05, .15), (.05, .05)]])


class PolygonEncoding(unittest.TestCase):
    def test_clipping_matches_original_polygons_at_bucket_and_tile_edges(self):
        original = Polygon([(9.8, 0), (10.2, 0), (10.2, .4), (9.8, .4), (9.8, 0)],
                           holes=[[(9.9, .1), (10.1, .1), (10.1, .2), (9.9, .2), (9.9, .1)]])
        for index in [(18, 9), (19, 9)]:
            payload = zstandard.ZstdDecompressor().decompress(encode_tile([original], index))
            data = json.loads(payload[4:])
            for x in [9.85, 9.95, 10, 10.05, 10.15]:
                if not index[0] * 10 - 180 <= x < index[0] * 10 - 170:
                    continue
                for y in [.05, .15, .25, .3, .5]:
                    col = int((x - (index[0] * 10 - 180)) * 4)
                    row = int(y * 4)
                    fragments = data['cells'][row * 40 + col]
                    actual = any(Polygon(rings[0], rings[1:]).covers(Point(x, y)) for rings in fragments)
                    self.assertEqual(actual, original.covers(Point(x, y)), (x, y))

    def test_rust_fixture_is_reproducible(self):
        path = ROOT / 'crates/proteus-core/src/basemodel/landmask_fixtures/polygons.bin.zst'
        expected = zstandard.ZstdDecompressor().decompress(encode_tile([fixture_polygon()], (18, 9)))
        actual = zstandard.ZstdDecompressor().decompress(path.read_bytes())
        self.assertEqual(actual, expected)


if __name__ == '__main__':
    unittest.main()
