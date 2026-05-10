import init, {
  Proteus,
  setup_panic_hook,
  HeatmapGenerator,
} from "./pkg/proteus.js";
import { preloader } from "./preloader.js";

let map = new maplibregl.Map({
  container: "map",
  style: {
    version: 8,
    sources: {
      "carto-dark": {
        type: "raster",
        tiles: [
          "https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
          "https://b.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
          "https://c.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
          "https://d.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
        ],
        tileSize: 256,
        attribution:
          '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> &copy; <a href="https://carto.com/attributions">CARTO</a>',
      },
    },
    layers: [
      {
        id: "carto-dark-layer",
        type: "raster",
        source: "carto-dark",
        minzoom: 0,
        maxzoom: 22,
      },
    ],
  },
  center: [0, 40],
  zoom: 1.5,
});
const scale = new maplibregl.ScaleControl({
  maxWidth: 100,
  unit: "metric",
});

map.addControl(scale, "bottom-right");

const latField = document.querySelector(".lat-field");
const lonField = document.querySelector(".lon-field");
const startBtn = document.getElementById("start-simulation");
const dayDisplay = document.getElementById("current-day");
const stepBtn = document.getElementById("step-simulation");
const stopBtn = document.getElementById("stop-simulation");
const resumeBtn = document.getElementById("resume-simulation");
const resetBtn = document.getElementById("reset-simulation");
const oilMenu = document.getElementById("oil-menu");
const startDate = document.getElementById("start-day-selector");
const totalDaysField = document.getElementById("total-day-field");
const heatmapToggle = document.getElementById("heatmap-toggle");
const particleToggle = document.getElementById("particle-toggle");
const releaseAmountField = document.getElementById("release-amount-field");
const releaseDurationField = document.getElementById("release-duration-field");
const releaseRadiusField = document.getElementById("release-radius-field");
const timelineSlider = document.getElementById("timeline-slider");
const timelinePlayBtn = document.getElementById("timeline-play");
const timelinePauseBtn = document.getElementById("timeline-pause");
const timelineContainer = document.getElementById("timeline-container");
const timelineSpeed = document.getElementById("timeline-speed");
const timelineRewind = document.getElementById("timeline-rewind");
const exportGeojsonBtn = document.getElementById("export-geojson");
const importGeojsonBtn = document.getElementById("import-geojson");
const importGeojsonFile = document.getElementById("import-geojson-file");
const autoZoom = document.getElementById("autozoom-checkbox");

map.on("click", function (e) {
  if (!simulationRunning) {
    rawLon = e.lngLat.lng.toFixed(2);
    rawLat = e.lngLat.lat.toFixed(2);

    updateFields();
    updateMarker();

    const currentPosition = getTileIndices([
      normalizeLongitude(rawLon),
      rawLat,
    ]);
    const currentDate = `${startYear}${String(startMonth).padStart(2, "0")}${String(startDay).padStart(2, "0")}`;

    preloader.preloadTiles(currentDate, currentPosition);
  }
});
startBtn.addEventListener("click", startSimulation);
stopBtn.addEventListener("click", stopSimulation);
resumeBtn.addEventListener("click", resumeSimulation);
resetBtn.addEventListener("click", resetSimulation);
latField.addEventListener("input", updatePositionFromFields);
lonField.addEventListener("input", updatePositionFromFields);
latField.addEventListener("blur", updateFields);
lonField.addEventListener("blur", updateFields);
startDate.addEventListener("input", updateSimulationDate);
totalDaysField.addEventListener("input", updateTotalDays);
heatmapToggle.addEventListener("click", toggleHeatmapMode);
particleToggle.addEventListener("click", toggleParticleMode);
releaseAmountField.addEventListener("input", updateReleaseAmount);
releaseDurationField.addEventListener("input", updateReleaseDuration);
releaseRadiusField.addEventListener("input", updateReleaseRadius);
timelineSlider.addEventListener("input", (e) => {
  updateTimelineDisplay(parseInt(e.target.value));
});

timelinePlayBtn.addEventListener("click", () => {
  timelinePlaying = true;
  timelinePlayback();
  timelinePlayBtn.style.display = "none";
  timelinePauseBtn.style.display = "inline-block";
});

timelinePauseBtn.addEventListener("click", () => {
  timelinePlaying = false;
  if (timelineAnimationId) {
    clearTimeout(timelineAnimationId);
  }
  timelinePlayBtn.style.display = "inline-block";
  timelinePauseBtn.style.display = "none";
});
timelineSpeed.addEventListener("click", updatePlaybackSpeed);
timelineRewind.addEventListener("click", () => {
  timelinePlaying = false;
  updateTimelineDisplay(0);
  timelinePlayBtn.style.display = "inline-block";
  timelinePauseBtn.style.display = "none";
});
exportGeojsonBtn.addEventListener("click", () => {
  if (simulationHistory.length === 0) {
    alert("No simulation data to export. Run a simulation first.");
    return;
  }

  // const hasHeatmaps = simulationHistory.some((s) => s.heatmapGeojson !== null);
  exportWithHeatmaps();
});

function exportParticlesOnly() {
  const exportData = {
    type: "FeatureCollection",
    properties: {
      model: "DriftMap",
      version: "1.0",
      date: new Date().toISOString(),
      includes_heatmaps: false,
      config: {
        release_lon: rawLon,
        release_lat: rawLat,
        release_amount_tons: releaseAmount,
        release_duration_days: releaseDuration,
        release_radius_km: spreadKm,
        start_date: `${startYear}-${String(startMonth).padStart(2, "0")}-${String(startDay).padStart(2, "0")}`,
        total_days: totalDays,
        particle_count: particleCount,
        cs_value: csValue,
      },
    },
    features: simulationHistory.map((snapshot) => ({
      type: "Feature",
      properties: {
        day: snapshot.day,
        date: snapshot.dateStr,
        active_particles: snapshot.activeGeojson.features.length,
        inactive_particles: snapshot.inactiveGeojson.features.length,
      },
      geometry: {
        type: "GeometryCollection",
        geometries: [
          {
            type: "MultiPoint",
            coordinates: snapshot.activeGeojson.features.map(
              (f) => f.geometry.coordinates,
            ),
          },
          {
            type: "MultiPoint",
            coordinates: snapshot.inactiveGeojson.features.map(
              (f) => f.geometry.coordinates,
            ),
          },
        ],
      },
    })),
  };

  downloadGeoJSON(exportData, "particles");
}

function exportWithHeatmaps() {
  const exportData = {
    type: "FeatureCollection",
    properties: {
      model: "DriftMap",
      version: "1.0",
      date: new Date().toISOString(),
      includes_heatmaps: true,
      config: {
        release_lon: rawLon,
        release_lat: rawLat,
        release_amount_tons: releaseAmount,
        release_duration_days: releaseDuration,
        release_radius_km: spreadKm,
        start_date: `${startYear}-${String(startMonth).padStart(2, "0")}-${String(startDay).padStart(2, "0")}`,
        total_days: totalDays,
        particle_count: particleCount,
        cs_value: csValue,
        oil_type: oilType,
      },
    },
    features: simulationHistory.map((snapshot) => {
      const geometries = [
        {
          type: "MultiPoint",
          coordinates: snapshot.activeGeojson.features.map(
            (f) => f.geometry.coordinates,
          ),
        },
        {
          type: "MultiPoint",
          coordinates: snapshot.inactiveGeojson.features.map(
            (f) => f.geometry.coordinates,
          ),
        },
      ];

      if (snapshot.heatmapGeojson && snapshot.heatmapGeojson.features) {
        // Store each polygon with its concentration property
        geometries.push({
          type: "FeatureCollection",
          features: snapshot.heatmapGeojson.features.map((f) => ({
            type: "Feature",
            geometry: f.geometry,
            properties: {
              concentration: f.properties.concentration,
            },
          })),
        });
      }

      return {
        type: "Feature",
        properties: {
          day: snapshot.day,
          date: snapshot.dateStr,
          active_particles: snapshot.activeGeojson.features.length,
          inactive_particles: snapshot.inactiveGeojson.features.length,
          has_heatmap: snapshot.heatmapGeojson !== null,
        },
        geometry: {
          type: "GeometryCollection",
          geometries: geometries,
        },
      };
    }),
  };

  downloadGeoJSON(exportData, "full");
}

function downloadGeoJSON(data, type) {
  const blob = new Blob([JSON.stringify(data)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `driftmap-results-${type}-${startYear}-${startMonth}-${startDay}.geojson`;
  a.click();
  URL.revokeObjectURL(url);
}

// Import GeoJSON results
importGeojsonBtn.addEventListener("click", () => {
  importGeojsonFile.click();
});

importGeojsonFile.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = (event) => {
    try {
      const data = JSON.parse(event.target.result);
      loadGeoJsonResults(data);
    } catch (err) {
      alert("Invalid GeoJSON file");
      console.error(err);
    }
  };
  reader.readAsText(file);
  importGeojsonFile.value = "";
});

function loadGeoJsonResults(data) {
  if (!data.features || data.features.length === 0) {
    alert("No simulation data found in file");
    return;
  }

  simulationRunning = false;
  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }

  const hasHeatmaps = data.properties.includes_heatmaps;

  simulationHistory = data.features.map((feature) => {
    const geometries = feature.geometry.geometries;

    const snapshot = {
      day: feature.properties.day,
      dateStr: feature.properties.date,
      activeGeojson: {
        type: "FeatureCollection",
        features: geometries[0].coordinates.map((coord) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: coord },
        })),
      },
      inactiveGeojson: {
        type: "FeatureCollection",
        features: geometries[1].coordinates.map((coord) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: coord },
        })),
      },
      heatmapGeojson: null,
    };

    if (hasHeatmaps && geometries.length > 2 && geometries[2]) {
      const heatmapFeatures =
        geometries[2].features || geometries[2].geometries || [];
      snapshot.heatmapGeojson = {
        type: "FeatureCollection",
        features: heatmapFeatures.map((f) => ({
          type: "Feature",
          geometry: f.geometry,
          properties: {
            concentration: f.properties?.concentration || 1,
          },
        })),
      };
    }

    return snapshot;
  });

  startBtn.style.display = "none";
  stopBtn.style.display = "none";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "inline-block";

  showTimeline();

  if (visualizationMode == "grid") {
    createHeatmapColorLegend(true);
  }
}
let today = new Date();
let proteus = null;
let simulationRunning = false;
let stepInProgress = false;
let animationId = null;
let simulationVersion = 0;
let simulationHistory = [];
let timelineDay = 0;
let timelinePlaying = false;
let timelineAnimationId = null;
let playbackSpeed = 100;
let heatmap = null;
let concentrationGrid = null;
let lastGridUpdate = 0;
let visualizationMode = "particles";
let rawLon = 56.5;
let rawLat = 26.6;
let csValue = 0.1;
let particleCount = 20000;
let spreadKm = 1.0;
let oilType = oilMenu ? oilMenu.value : "arabian_light";
let startYear = today.getFullYear();
let startMonth = today.getMonth() + 1;
let startDay = today.getDate();
let stepSize = 1 / 144;
let totalDays = 10.0;
let isError = false;
let stepCount = 0;
let boundingBox = [];
let releaseAmount = 100.0;
let releaseDuration = 1.0;

const GRID_UPDATE_INTERVAL = 200;
const GRID_SIZE = 0.05;
const CONCENTRATIONS = [
  particleCount / 20000,
  particleCount / 10000,
  particleCount / 5000,
  particleCount / 2500,
  particleCount / 1250,
  particleCount / 625,
  particleCount / 312.5,
  particleCount / 156.25,
  particleCount / 78.125,
  particleCount / 39.0625,
];
const COLORS = [
  "rgb(60, 90, 190)",
  "rgb(80, 140, 200)",
  "rgb(90, 175, 195)",
  "rgb(100, 190, 160)",
  "rgb(140, 200, 120)",
  "rgb(200, 210, 100)",
  "rgb(225, 210, 100)",
  "rgb(225, 170, 90)",
  "rgb(215, 135, 80)",
  "rgb(200, 100, 80)",
];
// Normalize longitude
function normalizeLongitude(lon) {
  lon = parseFloat(lon);
  lon = ((((lon + 180) % 360) + 360) % 360) - 180;
  return lon;
}
function getTileIndices(positions) {
  const tiles = new Set();

  for (let i = 0; i < positions.length; i += 2) {
    const lon = positions[i];
    const lat = positions[i + 1];

    // Calculate tile index (same as Rust: (lon - min_lon) / tile_size).floor()
    const minLon = -180;
    const minLat = -80;

    const lonIdx = Math.floor((lon - minLon) / 10);
    const latIdx = Math.floor((lat - minLat) / 10);

    // Clamp to valid range
    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
      tiles.add({ lonIdx, latIdx });
    }
  }

  return Array.from(tiles);
}

// Get next date (YYYYMMDD)
function addDays(dateInt, days) {
  const year = Math.floor(dateInt / 10000);
  const month = Math.floor((dateInt % 10000) / 100);
  const day = dateInt % 100;

  const date = new Date(year, month - 1, day);
  date.setDate(date.getDate() + days);

  return (
    date.getFullYear() * 10000 + (date.getMonth() + 1) * 100 + date.getDate()
  );
}

function updateBoundingBox() {
  boundingBox = proteus.get_particle_bounding_box();
}
// Initialize WASM and UI
async function initialize() {
  await init();
  setup_panic_hook();
  initGridLayer();
  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;

  proteus = new Proteus(
    normalizeLongitude(lon),
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
  );
  // await proteus.init_landmask();

  updateMarker();
  updateFields();
  setSimulationDate();
  updateTotalDays();
}

function initGridLayer() {
  map.on("load", () => {
    map.addSource("concentration", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
      tolerance: 0, // Disable simplification
      maxzoom: 24, // Keep detailed geometry at all zooms
    });
    map.addLayer({
      id: "concentration-fill",
      type: "fill",
      source: "concentration",
      paint: {
        // 'fill-color': [
        //     'interpolate', ['linear'], ['get', 'concentration'],
        //     0, 'rgb(231, 236, 251)',
        //     1, 'rgb(195, 209, 247)',
        //     2, 'rgb(162, 186, 244)',
        //     4, 'rgb(120, 153, 227)',
        //     8, 'rgb(68, 115, 227)',
        //     16, 'rgb(141, 142, 213)',
        //     32, 'rgb(252, 184, 197)',
        //     64, 'rgb(255, 115, 107)',
        //     128, 'rgb(254, 79, 44)',
        //     256, 'rgb(255, 106, 0)',
        //     512, 'rgb(255, 154, 0)',
        //     1024, 'rgb(255, 216, 1)',
        // ],
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
        "fill-outline-color": "rgba(0,0,0,0)",
      },
    });

    map.addSource("particles-active", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
    });
    map.addLayer({
      id: "active-particles-layer",
      type: "circle",
      source: "particles-active",
      paint: {
        "circle-radius": 1.4,
        "circle-color": "rgb(255, 255, 255)",
        "circle-opacity": 0.7,
      },
    });

    map.addSource("particles-inactive", {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
    });
    map.addLayer({
      id: "inactive-particles-layer",
      type: "circle",
      source: "particles-inactive",
      paint: {
        "circle-radius": 2,
        "circle-color": "rgb(255, 59, 20)",
        "circle-opacity": 0.7,
      },
    });

    toggleVisualizationMode();
  });
}

// Toggle between grid and particle visualization
function toggleVisualizationMode() {
  if (visualizationMode === "grid") {
    map.setLayoutProperty("concentration-fill", "visibility", "visible");
    map.setLayoutProperty("active-particles-layer", "visibility", "none");
    map.setLayoutProperty("inactive-particles-layer", "visibility", "none");
  } else {
    map.setLayoutProperty("concentration-fill", "visibility", "none");
    map.setLayoutProperty("active-particles-layer", "visibility", "visible");
    map.setLayoutProperty("inactive-particles-layer", "visibility", "visible");
  }
}

function toggleParticleMode() {
  if (visualizationMode == "particles") {
    return;
  }
  visualizationMode = "particles";
  toggleVisualizationMode();

  heatmapToggle.style.background = "none";
  heatmapToggle.style.color = "rgb(255, 255, 255)";
  particleToggle.style.background = "rgb(255, 255, 255)";
  particleToggle.style.color = "rgb(0, 0, 0)";

  // If we have timeline data, use the current snapshot's particles
  if (simulationHistory.length > 0 && timelineDay >= 0) {
    const snapshot = simulationHistory[timelineDay];
    if (snapshot && snapshot.activeGeojson) {
      map.getSource("particles-active").setData(snapshot.activeGeojson);
    }
    if (snapshot && snapshot.inactiveGeojson) {
      map.getSource("particles-inactive").setData(snapshot.inactiveGeojson);
    }
    createHeatmapColorLegend(false);
    return; // Skip live update
  }

  // Otherwise, try live update (only works during active simulation)
  if (proteus && proteus.get_positions().length > 0) {
    updateParticleVisualization();
  }

  createHeatmapColorLegend(false);
}

function toggleHeatmapMode() {
  if (visualizationMode == "grid") {
    return;
  }
  visualizationMode = "grid";
  toggleVisualizationMode();

  heatmapToggle.style.background = "rgb(255, 255, 255)";
  heatmapToggle.style.color = "rgb(0, 0, 0)";
  particleToggle.style.background = "none";
  particleToggle.style.color = "rgb(255, 255, 255)";

  // If we have timeline data, use the current snapshot's heatmap
  if (simulationHistory.length > 0 && timelineDay >= 0) {
    const snapshot = simulationHistory[timelineDay];
    if (snapshot && snapshot.heatmapGeojson) {
      createHeatmapColorLegend(true);
      return; // Skip the live grid generation
    }
  }

  // Otherwise, try live update (only works during active simulation)
  if (proteus && proteus.get_positions().length > 0) {
    // Check if bounding box is valid
    if (
      boundingBox.length === 0 ||
      boundingBox[0] === Infinity ||
      boundingBox[1] === -Infinity ||
      boundingBox[0] === boundingBox[1]
    ) {
      heatmap = new HeatmapGenerator(-180, 180, -80, 90, GRID_SIZE);
    } else {
      heatmap = new HeatmapGenerator(
        boundingBox[0] - GRID_SIZE * 2,
        boundingBox[1] + GRID_SIZE * 2,
        boundingBox[2] - GRID_SIZE * 2,
        boundingBox[3] + GRID_SIZE * 2,
        GRID_SIZE,
      );
    }
    updateGridVisualization();
  }

  createHeatmapColorLegend(true);
}

function updatePositionFromFields() {
  if (!Number.isNaN(lonField.value) && !Number.isNaN(latField.value)) {
    rawLon = lonField.value;
    rawLat = latField.value;
  }
  updateMarker();
}

function updateFields() {
  let displayLon = normalizeLongitude(rawLon).toFixed(2);
  lonField.value = displayLon;
  latField.value = rawLat;
}

function updateMarker() {
  if (!simulationRunning) {
    if (window.currentMarker) {
      window.currentMarker.remove();
    }

    window.currentMarker = new maplibregl.Marker({
      color: "#244886",
      scale: 0.9,
    })
      .setLngLat([rawLon, rawLat])
      .addTo(map);
  }
}

function updateReleaseAmount() {
  releaseAmount = releaseAmountField.value;
  if (visualizationMode == "grid") {
    createHeatmapColorLegend(true);
  }
}

function updateReleaseDuration() {
  releaseDuration = releaseDurationField.value;
}

function updateReleaseRadius() {
  spreadKm = releaseRadiusField.value;
}
// Update grid visualization from particle positions
function updateGridVisualization() {
  const positions = proteus.get_positions();

  const lons = [];
  const lats = [];

  for (let i = 0; i < positions.length; i += 2) {
    lons.push(positions[i]);
    lats.push(positions[i + 1]);
  }

  // Update grid
  heatmap.clear();
  heatmap.add_particles(lons, lats, null);
  heatmap.smooth();

  const geojsonStr = heatmap.to_contour_geojson(CONCENTRATIONS);
  const geojson = JSON.parse(geojsonStr);
  map.getSource("concentration").setData(geojson);
}

// Update particle visualization (points)
function updateParticleVisualization() {
  let activePositions = proteus.get_active_positions();
  let inactivePositions = proteus.get_inactive_positions();

  const geojsonActive = {
    type: "FeatureCollection",
    features: [],
  };

  const geojsonInactive = {
    type: "FeatureCollection",
    features: [],
  };

  for (let i = 0; i < activePositions.length; i += 2) {
    const lon = activePositions[i];
    const lat = activePositions[i + 1];

    if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
      geojsonActive.features.push({
        type: "Feature",
        geometry: {
          type: "Point",
          coordinates: [lon, lat],
        },
        properties: {},
      });
    }
  }

  for (let i = 0; i < inactivePositions.length; i += 2) {
    const lon = inactivePositions[i];
    const lat = inactivePositions[i + 1];

    if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
      geojsonInactive.features.push({
        type: "Feature",
        geometry: {
          type: "Point",
          coordinates: [lon, lat],
        },
        properties: {},
      });
    }
  }

  map.getSource("particles-active").setData(geojsonActive);
  map.getSource("particles-inactive").setData(geojsonInactive);
}

function createHeatmapColorLegend(show = true) {
  const oldLegend = document.getElementById("concentration-legend");
  if (oldLegend) oldLegend.remove();

  if (!show) return;

  const legendDiv = document.createElement("div");
  legendDiv.id = "concentration-legend";
  legendDiv.style.cssText = `
        position: absolute;
        bottom: 100px;
        right: 25px;
        display: flex;
        gap: 10px;
        z-index: 1;
    `;

  let html = "";
  html += '<div style="display:flex; flex-direction:column; gap:2px;">';
  for (let i = 0; i < 10; i++) {
    html += `<div style="background: ${COLORS[9 - i]}; height: 20px; width: 30px;"></div>`;
  }
  html += "</div>";

  html +=
    '<div style="display:flex; flex-direction:column; gap:2px; text-align:right;">';
  for (let i = 0; i < 10; i++) {
    html += `<div id="legend-val-${i}" style="color: white; font-family: monospace; font-size: 10px; height: 20px; line-height: 20px;">-</div>`;
  }
  html += "</div>";

  legendDiv.innerHTML = html;
  document.getElementById("map").appendChild(legendDiv);

  // Calculate values for legend
  const tonsPerParticle = releaseAmount / particleCount;

  const kmPerDegreeLon = 111.0 * Math.cos((rawLat * Math.PI) / 180);
  const kmPerDegreeLat = 111.0;

  // Cell area in km²
  const cellWidthKm = GRID_SIZE * kmPerDegreeLon;
  const cellHeightKm = GRID_SIZE * kmPerDegreeLat;
  const cellAreaKm2 = cellWidthKm * cellHeightKm;

  for (let i = 0; i < 10; i++) {
    const concentration = CONCENTRATIONS[9 - i];
    const tonsPerKm2 = (concentration / cellAreaKm2) * tonsPerParticle;

    const label = document.getElementById(`legend-val-${i}`);
    label.textContent = tonsPerKm2.toExponential(1) + " tons/km²";
  }
}
function captureSnapshot(day) {
  const snapshot = {
    day: day + 1,
    dateStr: proteus.current_time_str(),
    activeGeojson: getActiveGeojson(),
    inactiveGeojson: getInactiveGeojson(),
    heatmapGeojson: getHeatmapGeojson(),
  };
  simulationHistory.push(snapshot);
}

function getActiveGeojson() {
  const positions = proteus.get_active_positions();
  const features = [];
  for (let i = 0; i < positions.length; i += 2) {
    features.push({
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: [positions[i], positions[i + 1]],
      },
    });
  }
  return { type: "FeatureCollection", features };
}

function getInactiveGeojson() {
  const positions = proteus.get_inactive_positions();
  const features = [];
  for (let i = 0; i < positions.length; i += 2) {
    features.push({
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: [positions[i], positions[i + 1]],
      },
    });
  }
  return { type: "FeatureCollection", features };
}

function getHeatmapGeojson() {
  const positions = proteus.get_positions();
  if (positions.length === 0) return null;

  // Use world bounds if boundingBox is invalid
  let bbox = boundingBox;
  if (bbox.length === 0 || bbox[0] === Infinity || bbox[0] === bbox[1]) {
    bbox = [-180, 180, -80, 90];
  }

  const lons = [],
    lats = [];
  for (let i = 0; i < positions.length; i += 2) {
    lons.push(positions[i]);
    lats.push(positions[i + 1]);
  }

  heatmap = new HeatmapGenerator(
    bbox[0] - GRID_SIZE * 2,
    bbox[1] + GRID_SIZE * 2,
    bbox[2] - GRID_SIZE * 2,
    bbox[3] + GRID_SIZE * 2,
    GRID_SIZE,
  );

  heatmap.add_particles(lons, lats, null);
  heatmap.smooth();
  return JSON.parse(heatmap.to_contour_geojson(CONCENTRATIONS));
}

async function simulationStep(version) {
  if (!simulationRunning || version !== simulationVersion) {
    return;
  }

  try {
    stepInProgress = true;
    const currentDay = Math.floor(proteus.current_day());
    const stepsPerDay = Math.round(1 / stepSize);
    const todayDateInt = proteus.current_date_int(); // Renamed for clarity

    await proteus.step(stepSize);
    if (stepCount % stepsPerDay === 0) {
      const currentTiles = getTileIndices(proteus.get_positions());
      preloader.preloadTiles(todayDateInt, currentTiles);
      preloader.preloadFutureSteps(todayDateInt, proteus.get_positions(), 1, 0);
      for (const url of window.__tileCache.keys()) {
        const match = url.match(/(\d{4})\/(\d{2})\/(\d{2})/);
        if (match) {
          const tileDate = parseInt(match[1] + match[2] + match[3]);
          if (tileDate < todayDateInt - 1) {
            window.__tileCache.delete(url);
          }
        }
      }
    }

    if (stepCount % (stepsPerDay / 24) === 0) {
      captureSnapshot(currentDay);
    }

    updateBoundingBox();
    const now = performance.now();
    if (visualizationMode === "grid") {
      if (now - lastGridUpdate > GRID_UPDATE_INTERVAL) {
        heatmap = new HeatmapGenerator(
          boundingBox[0] - GRID_SIZE * 2,
          boundingBox[1] + GRID_SIZE * 2,
          boundingBox[2] - GRID_SIZE * 2,
          boundingBox[3] + GRID_SIZE * 2,
          GRID_SIZE,
        );
        updateGridVisualization();
        lastGridUpdate = now;
      }
    } else {
      updateParticleVisualization();
    }

    let day = proteus.current_day();
    dayDisplay.textContent = proteus.current_time_str();

    if (version === simulationVersion && day < totalDays) {
      animationId = requestAnimationFrame(() => simulationStep(version));
    } else {
      simulationRunning = false;
      showTimeline();
      startBtn.style.display = "none";
      stopBtn.style.display = "none";
      resumeBtn.style.display = "none";
      exportGeojsonBtn.style.display = "inline-block";
    }
  } catch (error) {
    console.error("Simulation step failed:", error);
    simulationRunning = false;
  } finally {
    stepCount++;
    stepInProgress = false;
  }
}

// Start simulation
async function startSimulation() {
  if (simulationRunning) {
    return;
  }

  // Reset visualization and state
  simulationRunning = true;
  simulationVersion++;
  lastGridUpdate = 0;
  concentrationGrid = null;

  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;

  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();

  proteus = new Proteus(
    normalizeLongitude(lon),
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
  );
  // await proteus.init_landmask();

  // Update UI
  startBtn.style.display = "none";
  stopBtn.style.display = "inline-flex";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "none";

  const currentZoom = map.getZoom();
  if (currentZoom < 5 && autoZoom.checked == true) {
    map.flyTo({
      center: [lon, lat],
      zoom: 6 - totalDays / 100,
      duration: 1500,
      essential: true,
    });
  }
  if (visualizationMode == "grid") {
    createHeatmapColorLegend(true);
  }
  simulationStep(simulationVersion);
}

// Stop simulation
function stopSimulation() {
  simulationRunning = false;

  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }

  startBtn.style.display = "none";
  stopBtn.style.display = "none";
  resumeBtn.style.display = "inline-flex";
  exportGeojsonBtn.style.display = "none";
}

// Resume simulation
function resumeSimulation() {
  if (simulationRunning) return;
  simulationRunning = true;
  simulationVersion++;
  startBtn.style.display = "none";
  stopBtn.style.display = "inline-flex";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "none";
  simulationStep(simulationVersion);
}

async function resetSimulation() {
  simulationRunning = false;
  simulationVersion++;
  stepCount = 0;
  simulationHistory = []; // Clear history!

  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }

  // Hide timeline
  const container = document.getElementById("timeline-container");
  if (container) container.style.display = "none";

  concentrationGrid = null;
  lastGridUpdate = 0;

  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;

  proteus = new Proteus(
    normalizeLongitude(lon),
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
  );
  // await proteus.init_landmask();

  map
    .getSource("concentration")
    .setData({ type: "FeatureCollection", features: [] });
  map
    .getSource("particles-active")
    .setData({ type: "FeatureCollection", features: [] });
  map
    .getSource("particles-inactive")
    .setData({ type: "FeatureCollection", features: [] });

  dayDisplay.textContent = "";

  startBtn.style.display = "inline-flex";
  stopBtn.style.display = "none";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "none";

  updateMarker();
  updateSimulationDate();
  updateTotalDays();
}

function updateSimulationDate() {
  if (!simulationRunning) {
    let inputDate = startDate.value.split("-");
    startYear = inputDate[0];
    startMonth = inputDate[1];
    startDay = inputDate[2];
  }
}

function setSimulationDate() {
  if (!simulationRunning) {
    const today = new Date();

    // Data window: 30 days analysis + 10 days forecast
    const minDate = new Date(today);
    minDate.setDate(today.getDate() - 30);

    const maxDate = new Date(today);
    maxDate.setDate(today.getDate() + 9);

    // Format as YYYY-MM-DD
    const formatDate = (d) => {
      return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    };

    startDate.min = formatDate(minDate);
    startDate.max = formatDate(maxDate);
    startDate.value = `${startYear}-${String(startMonth).padStart(2, "0")}-${String(startDay).padStart(2, "0")}`;
  }
}

function updateTotalDays() {
  if (!simulationRunning) {
    totalDays = totalDaysField.value;
  }
}

function showTimeline() {
  if (simulationHistory.length === 0) return;

  const container = document.getElementById("timeline-container");
  const slider = document.getElementById("timeline-slider");

  if (!container || !slider) return;

  slider.max = simulationHistory.length - 1;
  slider.value = simulationHistory.length - 1; // Show last frame by default

  document.getElementById("timeline-end").textContent =
    `Day ${simulationHistory[simulationHistory.length - 1].day}`;

  container.style.display = "flex";
  updateTimelineDisplay(simulationHistory.length - 1); // Show final state
  dayDisplay.textContent = "";
}

function updateTimelineDisplay(index) {
  if (index < 0 || index >= simulationHistory.length) return;

  const snapshot = simulationHistory[index];
  timelineDay = index;

  document.getElementById("timeline-current").textContent = snapshot.dateStr;
  document.getElementById("timeline-slider").value = index;

  // Always show particles if available
  if (snapshot.activeGeojson) {
    map.getSource("particles-active").setData(snapshot.activeGeojson);
  }
  if (snapshot.inactiveGeojson) {
    map.getSource("particles-inactive").setData(snapshot.inactiveGeojson);
  }

  // Always update heatmap regardless of current mode
  if (snapshot.heatmapGeojson) {
    map.getSource("concentration").setData(snapshot.heatmapGeojson);
  } else {
    // Clear heatmap if snapshot doesn't have one
    map
      .getSource("concentration")
      .setData({ type: "FeatureCollection", features: [] });
  }
}

function timelinePlayback() {
  if (!timelinePlaying) return;

  if (timelineDay < simulationHistory.length - 1) {
    timelineDay++;
    updateTimelineDisplay(timelineDay);
  } else {
    timelinePlaying = false;
    timelinePlayBtn.style.display = "inline-block";
    timelinePauseBtn.style.display = "none";
    return;
  }

  timelineAnimationId = setTimeout(() => {
    requestAnimationFrame(timelinePlayback);
  }, playbackSpeed);
}

function updatePlaybackSpeed() {
  if (playbackSpeed == 100) {
    playbackSpeed = 50;
    timelineSpeed.textContent = "2x";
    return;
  }
  if (playbackSpeed == 50) {
    playbackSpeed = 25;
    timelineSpeed.textContent = "4x";
    return;
  }
  if (playbackSpeed == 25) {
    playbackSpeed = 100;
    timelineSpeed.textContent = "1x";
    return;
  }
}

// Initialize when page loads
initialize().catch(console.error);
