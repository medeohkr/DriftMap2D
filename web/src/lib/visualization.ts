import { state } from "./state";
import { map } from "./map";
import { dom } from "./ui";
import { HeatmapGenerator } from "../../pkg/proteus";
import { getShiftedBounds } from "./utils";

const GRID_UPDATE_INTERVAL = 100;
const GRID_SIZE = 0.025;
const PADDING = GRID_SIZE * 2;
const CONCENTRATIONS = [
  0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2,
];

export const vizParams = {
  GRID_UPDATE_INTERVAL,
  GRID_SIZE,
  PADDING,
  CONCENTRATIONS,
}

const COLORS = [
  "rgb(65, 85, 185)",
  "rgb(60, 150, 130)",
  "rgb(70, 180, 120)",
  "rgb(150, 200, 90)",
  "rgb(195, 210, 100)",
  "rgb(240, 180, 60)",
  "rgb(240, 140, 40)",
  "rgb(220, 80, 40)",
  "rgb(190, 30, 50)", 
  "rgb(140, 15, 100)",
];

export function getScaledConcentrations() {
  const scale = state.releaseAmount / 100.0;
  return CONCENTRATIONS.map((c) => c * scale);
}

export function tonsPerKm2ToTonsPerCell(value) {
  const kmPerDegreeLon = 111.0 * Math.cos((state.rawLat * Math.PI) / 180);
  const kmPerDegreeLat = 111.0;
  const cellAreaKm2 = GRID_SIZE * kmPerDegreeLon * (GRID_SIZE * kmPerDegreeLat);
  return value * cellAreaKm2;
}

export function updateConcentrationLayer() {
  const thresholds = getScaledConcentrations().map(tonsPerKm2ToTonsPerCell);
  const stops = [];
  for (let i = 0; i < 10; i++) {
    stops.push(thresholds[i]);
    stops.push(COLORS[i]);
  }
  map.setPaintProperty("concentration-fill", "fill-color", [
    "interpolate",
    ["linear"],
    ["get", "concentration"],
    ...stops,
  ]);
}


export function updateOverlay() {
  map.setLayoutProperty(
    "overlay-layer",
    "visibility",
    dom.overlay.checked ? "visible" : "none",
  );
}

// ========== VISUALIZATION ==========
export function toggleVisualizationMode() {
  const isGrid = state.visualizationMode === "grid";
  map.setLayoutProperty(
    "concentration-fill",
    "visibility",
    isGrid ? "visible" : "none",
  );
  map.setLayoutProperty(
    "unstranded-particles-layer",
    "visibility",
    isGrid ? "none" : "visible",
  );
  map.setLayoutProperty(
    "stranded-particles-layer",
    "visibility",
    isGrid ? "none" : "visible",
  );
}

export function toggleParticleMode() {
  if (state.visualizationMode === "particles") return;
  createHeatmapColorLegend(false);
  // dom.collapseLegendBtn.style.display = "none";
  // dom.openLegendBtn.style.display = "none";
  state.visualizationMode = "particles";
  toggleVisualizationMode();
  dom.heatmapToggle.style.background = "none";
  dom.heatmapToggle.style.color = "white";
  dom.particleToggle.style.background = "white";
  dom.particleToggle.style.color = "black";
  if (!state.playbackMode) updateParticleVisualization();
}

export function toggleHeatmapMode() {
  if (state.visualizationMode === "grid") return;
  state. visualizationMode = "grid";
  toggleVisualizationMode();
  updateLegend();
  dom.heatmapToggle.style.background = "white";
  dom.heatmapToggle.style.color = "black";
  dom.particleToggle.style.background = "none";
  dom.particleToggle.style.color = "white";
  if (!state.playbackMode) updateGridVisualization();
}

export function updateParticleVisualization() {
  const unstranded = state.proteus?.get_unstranded_positions();
  const stranded = state.proteus?.get_stranded_positions();

  const geojsonUnstranded = { type: "FeatureCollection", features: [] as any[] };
  const geojsonStranded = { type: "FeatureCollection", features: [] as any[] };

  for (let i = 0; i < unstranded.length; i += 2) {
    geojsonUnstranded.features.push({
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: [unstranded[i], unstranded[i + 1]],
      },
    });
  }
  for (let i = 0; i < stranded.length; i += 2) {
    geojsonStranded.features.push({
      type: "Feature",
      geometry: { type: "Point", coordinates: [stranded[i], stranded[i + 1]] },
    });
  }

  map.getSource("particles-unstranded").setData(geojsonUnstranded);
  map.getSource("particles-stranded").setData(geojsonStranded);
}

export function updateGridVisualization() {
  const data = state.proteus?.get_unstranded_positions_with_mass();
  if (!data?.length) return;
  buildHeatmap();
  const geojson = JSON.parse(
    state.heatmap.to_contour_geojson(
      getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
    ),
  );
  map.getSource("concentration").setData(geojson);
}

export function buildHeatmap() {
  const data = state.proteus?.get_unstranded_positions_with_mass();
  if (!data?.length) return;
  const { lonMin, lonMax, needsShift } = getShiftedBounds(data);

  state.heatmap = new HeatmapGenerator(
    lonMin - PADDING, lonMax + PADDING,
    state.boundingBox[2] - PADDING, state.boundingBox[3] + PADDING,
    GRID_SIZE
  );

  const lons = [],
    lats = [],
    masses = [];
  for (let i = 0; i < data.length; i += 3) {
    let lon = data[i];
    if (needsShift && lon < 0) lon += 360;
    lons.push(lon);
    lats.push(data[i + 1]);
    masses.push(data[i + 2]);
  }

  state.heatmap.clear();
  state.heatmap.add_particles(lons, lats, masses);
  state.heatmap.smooth();
}
export function createHeatmapColorLegend(show = true) {
  const oldLegend = document.getElementById("concentration-legend");
  if (oldLegend) oldLegend.remove();
  if (!show) return;

  const scaled = getScaledConcentrations();

  const legendDiv = document.createElement("div");
  legendDiv.id = "concentration-legend";

  let barsHtml = '<div class="legend-bars">';
  let labelsHtml = '<div class="legend-labels">';

  for (let i = 0; i < 10; i++) {
    barsHtml += `<div style="background: ${COLORS[9 - i]};"></div>`;
    labelsHtml += `<div>${scaled[9 - i].toFixed(4)} tons/km²</div>`;
  }
  barsHtml += "</div>";
  labelsHtml += "</div>";

  legendDiv.innerHTML = barsHtml + labelsHtml;
  document.getElementById("map").appendChild(legendDiv);
}

export function updateLegend() {
  if (state.visualizationMode === "grid") {
    createHeatmapColorLegend(true);
    if (state.legendCollapsed) {
      createHeatmapColorLegend(true);
      document.getElementById("concentration-legend").style.display = "none";
      // state.openLegendBtn.style.display = "inline-block";
    } else {
      // state.collapseLegendBtn.style.display = "inline-block";
    }
  }

}

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

    toggleVisualizationMode();
    updateOverlay();
  });
}
