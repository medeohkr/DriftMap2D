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
  maxBounds: [
    [-Infinity, -80.00],
    [Infinity, 85.05] 
  ]
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
const collapseBtn = document.getElementById("collapse");
const openBtn = document.getElementById("open");
const sidebar = document.querySelector(".sidebar");
const overlay = document.getElementById("overlay-checkbox");
const statsDisplay = document.querySelector(".stats-container");
const emulsified = document.getElementById("emulsified");
const stranded = document.getElementById("stranded");
const evaporated = document.getElementById("evaporated");
const totalMass = document.getElementById("total-mass");

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
let particleCount = 10000;
let spreadKm = 1.0;
let oilType = oilMenu.value;
let startYear = today.getFullYear();
let startMonth = today.getMonth() + 1;
let startDay = today.getDate();
let stepSize = 1 / 48;
let totalDays = 6.0;
let isError = false;
let playbackMode = false;
let stepCount = 0;
let boundingBox = [];
let releaseAmount = 1000.0;
let releaseDuration = 1.0;

const GRID_UPDATE_INTERVAL = 100;
let GRID_SIZE = 0.02;
const CONCENTRATIONS = [
    0.0001,
    0.0002,
    0.0005,
    0.001,
    0.002,
    0.005,
    0.01,
    0.02,
    0.05,
    0.1,
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
map.on("click", function (e) {
  if (simulationHistory.length === 0) {
    rawLon = e.lngLat.lng.toFixed(2);
    rawLat = e.lngLat.lat.toFixed(2);
    updateFields();
    updateMarker();
  }
});

const coordDisplay = document.createElement("div");
coordDisplay.id = "coordinate-display";
coordDisplay.style.cssText = `
    position: absolute;
    bottom: 18px;
    right: 45px;
    color: rgba(255, 255, 255, 0.7);
    font-family: monospace;
    font-size: 12px;
    z-index: 10;
    pointer-events: none;
`;
document.body.appendChild(coordDisplay);

map.on("mousemove", function (e) {
  const lon = e.lngLat.lng.toFixed(2);
  const lat = e.lngLat.lat.toFixed(2);
  coordDisplay.textContent = `${lat}°, ${lon}°`;
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
exportGeojsonBtn.addEventListener("click", exportScenario);
collapseBtn.addEventListener("click", () => {
  sidebar.style.display = "none";
  openBtn.style.display = "inline-block";
  collapseBtn.style.display = "none";
});
openBtn.addEventListener("click", () => {
  sidebar.style.display = "flex";
  openBtn.style.display = "none";
  collapseBtn.style.display = "inline-block";
});
overlay.addEventListener("click", updateOverlay);

function exportScenario() {
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
          coordinates: snapshot.unstrandedGeojson.features.map(
            (f) => f.geometry.coordinates,
          ),
        },
        {
          type: "MultiPoint",
          coordinates: snapshot.strandedGeojson.features.map(
            (f) => f.geometry.coordinates,
          ),
        },
      ];

      if (snapshot.heatmapGeojson && snapshot.heatmapGeojson.features) {
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
          stats: snapshot.stats,
          unstranded_particles: snapshot.unstrandedGeojson.features.length,
          stranded_particles: snapshot.strandedGeojson.features.length,
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
  a.download = `driftmap-${type}-${startYear}-${startMonth}-${startDay}.geojson`;
  a.click();
  URL.revokeObjectURL(url);
}

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
      console.error("Import error:", err.message, err.stack);
      alert("Invalid GeoJSON file: " + err.message);
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
      stats: feature.properties.stats,
      unstrandedGeojson: {
        type: "FeatureCollection",
        features: geometries[0].coordinates.map((coord) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: coord },
        })),
      },
      strandedGeojson: {
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
  latField.value = `${data.properties.config.release_lat}`;
  lonField.value = `${data.properties.config.release_lon}`;
  releaseAmountField.value = `${data.properties.config.release_amount_tons}`;
  releaseDurationField.value = `${data.properties.config.release_duration_days}`;
  releaseRadiusField.value = `${data.properties.config.release_radius_km}`;
  startDate.value = `${data.properties.config.start_date}`;
  totalDaysField.value = `${data.properties.config.total_days}`;
  statsDisplay.style.display = "flex";

  updatePositionFromFields();
  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();
  showTimeline();
  updateConcentrationLayer();

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  playbackMode = true;

  if (window.currentMarker) {
    window.currentMarker.remove();
  }

  if (visualizationMode == "grid") {
    createHeatmapColorLegend(true);
  }
  if (autoZoom.checked) {
    map.flyTo({
      center: [rawLon, rawLat],
      zoom: 6 - totalDays / 100,
      duration: 2000,
      essential: true,
    });
  }
}

function normalizeLongitude(lon) {
  lon = parseFloat(lon);
  lon = ((((lon + 180) % 360) + 360) % 360) - 180;
  return lon;
}
function getTileIndices(positions, minLat = -80) {
  const tiles = new Set();

  for (let i = 0; i < positions.length; i += 2) {
    const lon = positions[i];
    const lat = positions[i + 1];

    const minLon = -180;

    const lonIdx = Math.floor((lon - minLon) / 10);
    const latIdx = Math.floor((lat - minLat) / 10);

    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
      tiles.add({ lonIdx, latIdx });
    }
  }

  return Array.from(tiles);
}

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
  
  // Check if bounding box is valid
  if (boundingBox[0] === Infinity || boundingBox[1] === -Infinity || 
      boundingBox[2] === Infinity || boundingBox[3] === -Infinity) {
    return;
  }
  
  const TARGET_CELLS = 5000;
  
  const width = boundingBox[1] - boundingBox[0];
  const height = boundingBox[3] - boundingBox[2];
  
  // Guard against zero area
  if (width <= 0 || height <= 0) {
    return;
  }
  
  const area = width * height;
  let gridSize = Math.sqrt(area / TARGET_CELLS);
  gridSize = Math.max(0.02, Math.min(0.2, gridSize));
  
  GRID_SIZE = gridSize;
}

async function initialize() {
  await init();
  setup_panic_hook();
  initGridLayer();
  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;
  oilType = oilMenu.value;
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
    oilType
  );

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
        "fill-outline-color": "rgba(0,0,0,0)",
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
        "circle-color": "rgb(255, 255, 255)",
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
    map.addSource('overlay-png', {
      'type': 'image',
      'url': 'https://tiles.driftmap2d.com/currents.png',
      // 'url': 'data/currents.png', 
      'coordinates': [
        [-199.8, 85.05], // Top Left
        [199.61, 85.05], // Top Right
        [199.61, -80.0], // Bottom Right
        [-199.8, -80.0]  // Bottom Left
      ]
    });

    // 2. Add the raster layer
    map.addLayer({
      'id': 'overlay-layer',
      'type': 'raster',
      'source': 'overlay-png',
      'paint': {
        'raster-opacity': 0.4 
      }
    });
    toggleVisualizationMode();
    updateOverlay();
  });
}

function toggleVisualizationMode() {
  if (visualizationMode === "grid") {
    map.setLayoutProperty("concentration-fill", "visibility", "visible");
    map.setLayoutProperty("unstranded-particles-layer", "visibility", "none");
    map.setLayoutProperty("stranded-particles-layer", "visibility", "none");
  } else {
    map.setLayoutProperty("concentration-fill", "visibility", "none");
    map.setLayoutProperty("unstranded-particles-layer", "visibility", "visible");
    map.setLayoutProperty("stranded-particles-layer", "visibility", "visible");
  }
}

function toggleParticleMode() {
  if (visualizationMode == "particles") return;
  createHeatmapColorLegend(false);
  visualizationMode = "particles";
  toggleVisualizationMode();

  heatmapToggle.style.background = "none";
  heatmapToggle.style.color = "rgb(255, 255, 255)";
  particleToggle.style.background = "rgb(255, 255, 255)";
  particleToggle.style.color = "rgb(0, 0, 0)";

  if (!playbackMode) {
    updateParticleVisualization();
  }

}

function toggleHeatmapMode() {
  if (visualizationMode == "grid") {
    return;
  }
  createHeatmapColorLegend(true);
  visualizationMode = "grid";
  toggleVisualizationMode();

  heatmapToggle.style.background = "rgb(255, 255, 255)";
  heatmapToggle.style.color = "rgb(0, 0, 0)";
  particleToggle.style.background = "none";
  particleToggle.style.color = "rgb(255, 255, 255)";

  if (simulationHistory.length != 0){
    updateGridVisualization();

  }
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
  let displayLat = parseFloat(rawLat).toFixed(2);
  lonField.value = displayLon;
  latField.value = displayLat;
  const oceanTile = getTileIndices([normalizeLongitude(rawLon), rawLat], -80);
  const landTile = getTileIndices([normalizeLongitude(rawLon), rawLat], -90);
  const currentDate = parseInt(
    `${startYear}${String(startMonth).padStart(2, "0")}${String(startDay).padStart(2, "0")}`,
  );

  preloader.preloadTiles(currentDate, oceanTile);
  preloader.preloadLandmaskTiles(landTile);
}

function updateMarker() {
  if (simulationHistory.length === 0) {
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
  if (simulationHistory.length === 0) {
    releaseAmount = releaseAmountField.value;
    if (visualizationMode == "grid") {
      createHeatmapColorLegend(true);
    }
  }
}

function updateReleaseDuration() {
  if (simulationHistory.length === 0) {
    releaseDuration = releaseDurationField.value;
  }
}

function updateReleaseRadius() {
  if (simulationHistory.length === 0) {
    spreadKm = releaseRadiusField.value;
  }
}

function updateGridVisualization() {
    const data = proteus.get_unstranded_positions_with_mass();
    if (!data || data.length === 0) {
      map.getSource("concentration").setData({ type: "FeatureCollection", features: [] });
      return;
    }
    const lons = [];
    const lats = [];
    const masses = [];

    for (let i = 0; i < data.length; i += 3) {
        lons.push(data[i]);
        lats.push(data[i + 1]);
        masses.push(data[i + 2]);
    }

    heatmap.clear();
    heatmap.add_particles(lons, lats, masses);
    heatmap.smooth();
    const scaledConcentrations = getScaledConcentrations();
    const thresholdsInTonsPerCell = scaledConcentrations.map(tonsPerKm2ToTonsPerCell);
    const geojsonStr = heatmap.to_contour_geojson(thresholdsInTonsPerCell);
    const geojson = JSON.parse(geojsonStr);
    map.getSource("concentration").setData(geojson);
}

function updateParticleVisualization() {
  let unstrandedPositions = proteus.get_unstranded_positions();
  let strandedPositions = proteus.get_stranded_positions();

  const geojsonUnstranded = {
    type: "FeatureCollection",
    features: [],
  };

  const geojsonStranded = {
    type: "FeatureCollection",
    features: [],
  };

  for (let i = 0; i < unstrandedPositions.length; i += 2) {
    const lon = unstrandedPositions[i];
    const lat = unstrandedPositions[i + 1];

    if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
      geojsonUnstranded.features.push({
        type: "Feature",
        geometry: {
          type: "Point",
          coordinates: [lon, lat],
        },
        properties: {},
      });
    }
  }

  for (let i = 0; i < strandedPositions.length; i += 2) {
    const lon = strandedPositions[i];
    const lat = strandedPositions[i + 1];

    if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
      geojsonStranded.features.push({
        type: "Feature",
        geometry: {
          type: "Point",
          coordinates: [lon, lat],
        },
        properties: {},
      });
    }
  }

  map.getSource("particles-unstranded").setData(geojsonUnstranded);
  map.getSource("particles-stranded").setData(geojsonStranded);
}

function createHeatmapColorLegend(show = true) {
    const oldLegend = document.getElementById("concentration-legend");
    if (oldLegend) oldLegend.remove();
    if (!show) return;

    const scaled = getScaledConcentrations();
    
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

    html += '<div style="display:flex; flex-direction:column; gap:2px; text-align:right;">';
    for (let i = 0; i < 10; i++) {
        const val = scaled[9 - i];
        const label = val.toFixed(4) + " tons/km²";
        html += `<div style="color: white; font-family: monospace; font-size: 10px; height: 20px; line-height: 20px;">${label}</div>`;
    }
    html += "</div>";

    legendDiv.innerHTML = html;
    document.getElementById("map").appendChild(legendDiv);
}
function captureSnapshot(day) {
  const snapshot = {
    day: day + 1,
    dateStr: proteus.current_time_str(),
    stats: getStatsDisplay(),
    unstrandedGeojson: getUnstrandedGeojson(),
    strandedGeojson: getStrandedGeojson(),
    heatmapGeojson: getHeatmapGeojson(),
  };
  simulationHistory.push(snapshot);
}

function getUnstrandedGeojson() {
  const positions = proteus.get_unstranded_positions();
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

function getStrandedGeojson() {
  const positions = proteus.get_stranded_positions();
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
  const data = proteus.get_unstranded_positions_with_mass();
  if (!data ||data.length === 0) {
    return { type: "FeatureCollection", features: [] };
  }
  const lons = [];
  const lats = [];
  const masses = [];

  for (let i = 0; i < data.length; i += 3) {
      lons.push(data[i]);
      lats.push(data[i + 1]);
      masses.push(data[i + 2]);
  }


  heatmap = new HeatmapGenerator(
    boundingBox[0] - GRID_SIZE * 2,
    boundingBox[1] + GRID_SIZE * 2,
    boundingBox[2] - GRID_SIZE * 2,
    boundingBox[3] + GRID_SIZE * 2,
    GRID_SIZE,
  );

  heatmap.clear();
  heatmap.add_particles(lons, lats, masses);
  heatmap.smooth();
  const scaledConcentrations = getScaledConcentrations();          // tons/km², scaled by release size
  const thresholdsInTonsPerCell = scaledConcentrations.map(tonsPerKm2ToTonsPerCell);  // tons per cell
  const geojsonStr = heatmap.to_contour_geojson(thresholdsInTonsPerCell);
  return JSON.parse(geojsonStr);
}

async function simulationStep(version) {
  if (!simulationRunning || version !== simulationVersion) {
    return;
  }

  try {
    stepInProgress = true;
    const currentDay = Math.floor(proteus.current_day());
    const stepsPerDay = Math.round(1 / stepSize);
    const todayDateInt = proteus.current_date_int();

    await proteus.step(stepSize);
    if (stepCount % stepsPerDay === 0) {
      const oceanTiles = getTileIndices(proteus.get_positions(), -80);
      const landTiles = getTileIndices(proteus.get_positions(), -90);
      preloader.preloadTiles(todayDateInt, oceanTiles);
      preloader.preloadLandmaskTiles(landTiles);
      preloader.preloadFutureSteps(todayDateInt, proteus.get_positions(), 2, 0);
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

    if (stepCount % (stepsPerDay / 24) === 1) {
      captureSnapshot(currentDay);
      updateStatsDisplay();
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

    if (day < totalDays) {
      animationId = requestAnimationFrame(() => simulationStep(version));
    } else {
      simulationRunning = false;
      showTimeline();
      playbackMode = true;
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

async function startSimulation() {
  if (simulationRunning || isError) {
    if (isError)
      alert("Simulation dates are outside the available data window.");
    return;
  }

  simulationRunning = true;
  simulationVersion++;
  lastGridUpdate = 0;
  concentrationGrid = null;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();
  updateConcentrationLayer();

  if (window.currentMarker) {
    window.currentMarker.remove();
  }

  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;
  oilType = oilMenu.value;

  const currentZoom = map.getZoom();

  if (currentZoom < 5 && autoZoom.checked == true) {
    map.flyTo({
      center: [rawLon, rawLat],
      zoom: 6 - totalDays / 100,
      duration: 2000,
      essential: true,
    });
  }
  if (visualizationMode == "grid") {
    createHeatmapColorLegend(true);
  }

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
    oilType
  );
  simulationStep(simulationVersion);

  startBtn.style.display = "none";
  stopBtn.style.display = "inline-flex";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "none";
  statsDisplay.style.display = "grid";
}

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
  simulationHistory = [];
  playbackMode = false;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.4);

  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }

  const container = document.getElementById("timeline-container");
  if (container) container.style.display = "none";

  concentrationGrid = null;
  lastGridUpdate = 0;

  let lon = normalizeLongitude(rawLon);
  let lat = rawLat;
  oilType = oilMenu.value;

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
    oilType
  );

  map
    .getSource("concentration")
    .setData({ type: "FeatureCollection", features: [] });
  map
    .getSource("particles-unstranded")
    .setData({ type: "FeatureCollection", features: [] });
  map
    .getSource("particles-stranded")
    .setData({ type: "FeatureCollection", features: [] });

  dayDisplay.textContent = "";

  startBtn.style.display = "inline-flex";
  stopBtn.style.display = "none";
  resumeBtn.style.display = "none";
  exportGeojsonBtn.style.display = "none";
  statsDisplay.style.display = "none";

  updateFields();
  updateMarker();
  updateSimulationDate();
  updateTotalDays();
}

function updateSimulationDate() {
  if (simulationHistory.length === 0) {
    let inputDate = startDate.value.split("-");
    startYear = parseInt(inputDate[0]);
    startMonth = parseInt(inputDate[1]);
    startDay = parseInt(inputDate[2]);

    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const simStart = new Date(startYear, startMonth - 1, startDay);

    const minDate = new Date(today);
    minDate.setDate(minDate.getDate() - 30);

    const maxDate = new Date(today);
    maxDate.setDate(maxDate.getDate() + 10);

    if (simStart < minDate || simStart > maxDate) {
      isError = true;
      startDate.style.border = "2px solid rgb(255, 59, 20)";
      return;
    }

    const simEnd = new Date(simStart);
    simEnd.setDate(simEnd.getDate() + Math.ceil(totalDays));
    if (simEnd > maxDate) {
      isError = true;
      startDate.style.border = "2px solid rgb(255, 59, 20)";
      totalDaysField.style.border = "2px solid rgb(255, 59, 20)";
      return;
    }

    isError = false;
    startDate.style.border = "1px solid rgba(0, 0, 0, 0.3)";
    totalDaysField.style.border = "1px solid rgba(0, 0, 0, 0.3)";
  }
}

function setSimulationDate() {
  if (simulationHistory.length === 0) {
    const today = new Date();

    const minDate = new Date(today);
    minDate.setDate(today.getDate() - 30);

    const maxDate = new Date(today);
    maxDate.setDate(today.getDate() + 9);

    const formatDate = (d) => {
      return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    };

    startDate.min = formatDate(minDate);
    startDate.max = formatDate(maxDate);
    startDate.value = `${startYear}-${String(startMonth).padStart(2, "0")}-${String(startDay).padStart(2, "0")}`;
  }
}

function updateTotalDays() {
  if (simulationHistory.length === 0) {
    const days = parseFloat(totalDaysField.value);

    if (isNaN(days) || days <= 0) {
      isError = true;
      totalDaysField.style.border = "2px solid rgb(255, 59, 20)";
      return;
    }

    const simStart = new Date(startYear, startMonth - 1, startDay);
    const simEnd = new Date(simStart);
    simEnd.setDate(simEnd.getDate() + Math.ceil(days));

    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const maxDate = new Date(today);
    maxDate.setDate(maxDate.getDate() + 10);

    if (simEnd > maxDate) {
      isError = true;
      totalDaysField.style.border = "2px solid rgb(255, 59, 20)";
      return;
    }

    if (simStart < new Date(today).setDate(today.getDate() - 30)) {
      isError = true;
      totalDaysField.style.border = "2px solid rgb(255, 59, 20)";
      return;
    }

    isError = false;
    totalDaysField.style.border = "1px solid rgba(0, 0, 0, 0.3)";
    totalDays = days;
  }
}

function showTimeline() {
  if (simulationHistory.length === 0) return;

  const container = document.getElementById("timeline-container");
  const slider = document.getElementById("timeline-slider");

  if (!container || !slider) return;

  slider.max = simulationHistory.length - 1;
  slider.value = simulationHistory.length - 1;

  document.getElementById("timeline-end").textContent =
    `Day ${simulationHistory[simulationHistory.length - 1].day}`;
  container.style.display = "flex";
  updateTimelineDisplay(simulationHistory.length - 1);
  dayDisplay.textContent = "";
}

function updateTimelineDisplay(index) {
  if (index < 0 || index >= simulationHistory.length) return;

  const snapshot = simulationHistory[index];
  timelineDay = index;

  document.getElementById("timeline-current").textContent = snapshot.dateStr;
  document.getElementById("timeline-slider").value = index;
  stranded.textContent = `${snapshot.stats.stranded}%`;
  emulsified.textContent = `${snapshot.stats.emulsified}%`;
  evaporated.textContent = `${snapshot.stats.evaporated}%`;
  totalMass.textContent = `${snapshot.stats.total_mass} tons`;
  map.getSource("particles-unstranded").setData(snapshot.unstrandedGeojson);
  map.getSource("particles-stranded").setData(snapshot.strandedGeojson);
  map.getSource("concentration").setData(snapshot.heatmapGeojson);
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

function updateOverlay() {
  if (overlay.checked) {
    map.setLayoutProperty("overlay-layer", "visibility", "visible");
  } else {
    map.setLayoutProperty("overlay-layer", "visibility", "none");
  }
}

function updateStatsDisplay() {
    stranded.textContent = `${proteus.stranded_fraction().toFixed(1)}%`;
    emulsified.textContent = `${proteus.mass_weighted_emulsification().toFixed(1)}%`;
    evaporated.textContent = `${proteus.mass_weighted_evaporation().toFixed(1)}%`;
    totalMass.textContent = `${proteus.total_floating_mass_tons().toFixed(1)} tons`;
}

function getStatsDisplay() {
    return {
        stranded: proteus.stranded_fraction().toFixed(1),
        emulsified: proteus.mass_weighted_emulsification().toFixed(1),
        evaporated: proteus.mass_weighted_evaporation().toFixed(1),
        total_mass: proteus.total_floating_mass_tons().toFixed(1),
    };
}
function getScaledConcentrations() {
    const scale = releaseAmount / 100.0;
    return CONCENTRATIONS.map(c => c * scale);
}

function tonsPerKm2ToTonsPerCell(value) {
    const kmPerDegreeLon = 111.0 * Math.cos((rawLat * Math.PI) / 180);
    const kmPerDegreeLat = 111.0;
    const cellWidthKm = GRID_SIZE * kmPerDegreeLon;
    const cellHeightKm = GRID_SIZE * kmPerDegreeLat;
    const cellAreaKm2 = cellWidthKm * cellHeightKm;
    return value * cellAreaKm2;
}
function updateConcentrationLayer() {
    const scaledConcentrations = getScaledConcentrations();
    const thresholdsInTonsPerCell = scaledConcentrations.map(tonsPerKm2ToTonsPerCell);
    
    // Build the interpolation stops: [threshold0, color0, threshold1, color1, ...]
    const stops = [];
    for (let i = 0; i < 10; i++) {
        stops.push(thresholdsInTonsPerCell[i]);
        stops.push(COLORS[i]);
    }
    
    map.setPaintProperty("concentration-fill", "fill-color", [
        "interpolate",
        ["linear"],
        ["get", "concentration"],
        ...stops
    ]);
}
initialize().catch(console.error);