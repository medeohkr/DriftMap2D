import * as maplibregl from "maplibre-gl";

export let map: maplibregl.Map;

export function initMap() {
  map = new maplibregl.Map({
    container: "map",
    style: {
      version: 8,
      sources: {
        "carto-dark": {
          type: "raster",
          tiles: ["https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png"],
          tileSize: 256,
          attribution:
            '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
        },
      },
      layers: [{ id: "carto-dark-layer", type: "raster", source: "carto-dark" }],
    },
    center: [0, 40],
    zoom: 1.5,
    maxBounds: [
      [-Infinity, -75.0],
      [Infinity, 85.0],
    ],
  });

  map.addControl(
    new maplibregl.ScaleControl({ maxWidth: 100, unit: "metric" }),
    "bottom-right",
  );
}