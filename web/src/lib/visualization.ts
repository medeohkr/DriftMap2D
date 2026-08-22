import { map } from "./map";

const CONCENTRATIONS = [
  0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2,
];

export function initGridLayer() {
  map.on("load", () => {
    map.addSource("concentration", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
      tolerance: 0,
      maxzoom: 24,
    });
    map.addLayer({
      id: "concentration-fill",
      type: "fill",
      source: "concentration",
      paint: {
        "fill-color": [
          "interpolate",
          ["linear"],
          ["get", "concentration"],
          CONCENTRATIONS[0],
          "rgb(60, 90, 190)",
          CONCENTRATIONS[1],
          "rgb(80, 140, 200)",
          CONCENTRATIONS[2],
          "rgb(90, 175, 195)",
          CONCENTRATIONS[3],
          "rgb(100, 190, 160)",
          CONCENTRATIONS[4],
          "rgb(140, 200, 120)",
          CONCENTRATIONS[5],
          "rgb(200, 210, 100)",
          CONCENTRATIONS[6],
          "rgb(225, 210, 100)",
          CONCENTRATIONS[7],
          "rgb(225, 170, 90)",
          CONCENTRATIONS[8],
          "rgb(215, 135, 80)",
          CONCENTRATIONS[9],
          "rgb(200, 100, 80)",
        ],
        "fill-opacity": 1.0,
        "fill-antialias": false,
      },
    });

    map.addSource("particles-unstranded", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
    });
    map.addLayer({
      id: "unstranded-particles-layer",
      type: "circle",
      source: "particles-unstranded",
      paint: {
        "circle-radius": 1.4,
        "circle-color": "white",
        "circle-opacity": 0.7,
      },
    });

    map.addSource("particles-stranded", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
    });
    map.addLayer({
      id: "stranded-particles-layer",
      type: "circle",
      source: "particles-stranded",
      paint: {
        "circle-radius": 2,
        "circle-color": "rgb(255, 59, 20)",
        "circle-opacity": 0.7,
      },
    });

    map.addSource("overlay-png", {
      type: "image",
      url: "https://tiles.driftmap2d.com/currents.png",
      // url: "images/currents.png",
      coordinates: [
        [-199.71, 85.05],
        [199.71, 85.05],
        [199.71, -80.00],
        [-199.71, -80.00],
      ],
    });
    map.addLayer({
      id: "overlay-layer",
      type: "raster",
      source: "overlay-png",
      paint: { "raster-opacity": 0.4 },
    });

    // toggleVisualizationMode();
    updateOverlay(false);
  });
}

export function updateOverlay(checked: boolean) {
  map.setLayoutProperty(
    "overlay-layer",
    "visibility",
    checked ? "visible" : "none",
  );
}