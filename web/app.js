import init, {
  Proteus,
  setup_panic_hook,
  HeatmapGenerator,
} from "./pkg/proteus.js";
import { preloader } from "./preloader.js";

// ========== MAP INITIALIZATION ==========
let map = new maplibregl.Map({
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
    [-Infinity, -80.0],
    [Infinity, 85.05],
  ],
});

map.addControl(
  new maplibregl.ScaleControl({ maxWidth: 100, unit: "metric" }),
  "bottom-right",
);

// ========== DOM ELEMENTS ==========
const latField = document.querySelector(".lat-field");
const lonField = document.querySelector(".lon-field");
const startBtn = document.getElementById("start-simulation");
const dayDisplay = document.getElementById("current-day");
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
const collapseLegendBtn = document.getElementById("legend-collapse");
const openLegendBtn = document.getElementById("legend-open");

// ========== GLOBAL STATE ==========
let today = new Date();
let proteus = null;
let simulationRunning = false;
let animationId = null;
let simulationVersion = 0;
let simulationHistory = [];
let timelineDay = 0;
let timelinePlaying = false;
let timelineAnimationId = null;
let playbackSpeed = 100;
let heatmap = null;
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
let stepSize = 1 / 96;
let totalDays = 7.0;
let playbackMode = false;
let stepCount = 0;
let boundingBox = [];
let releaseAmount = 1000.0;
let releaseDuration = 1.0;
let legendCollapsed = false;

const GRID_UPDATE_INTERVAL = 150;
let GRID_SIZE = 0.025;

const CONCENTRATIONS = [
  0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2,
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

// ========== HELPER FUNCTIONS ==========
function normalizeLongitude(lon) {
  lon = parseFloat(lon);
  return ((((lon + 180) % 360) + 360) % 360) - 180;
}

function getTileIndices(positions, minLat = -80) {
  const tiles = new Set();
  for (let i = 0; i < positions.length; i += 2) {
    const lon = positions[i];
    const lat = positions[i + 1];
    const lonIdx = Math.floor((lon + 180) / 10);
    const latIdx = Math.floor((lat - minLat) / 10);
    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
      tiles.add({ lonIdx, latIdx });
    }
  }
  return Array.from(tiles);
}

function getScaledConcentrations() {
  const scale = releaseAmount / 100.0;
  return CONCENTRATIONS.map((c) => c * scale);
}

function tonsPerKm2ToTonsPerCell(value) {
  const kmPerDegreeLon = 111.0 * Math.cos((rawLat * Math.PI) / 180);
  const kmPerDegreeLat = 111.0;
  const cellAreaKm2 = GRID_SIZE * kmPerDegreeLon * (GRID_SIZE * kmPerDegreeLat);
  return value * cellAreaKm2;
}

function updateConcentrationLayer() {
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

function updateSimulationDate() {
  if (simulationHistory.length === 0) {
    let inputDate = startDate.value.split("-");
    startYear = parseInt(inputDate[0]);
    startMonth = parseInt(inputDate[1]);
    startDay = parseInt(inputDate[2]);
  }
}

function updateTotalDays() {
  if (simulationHistory.length === 0) {
    totalDays = parseFloat(totalDaysField.value);
  }
}

function updateReleaseAmount() {
  if (simulationHistory.length === 0) {
    releaseAmount = parseFloat(releaseAmountField.value);
    updateLegend();
  }
}

function updateReleaseDuration() {
  if (simulationHistory.length === 0) {
    releaseDuration = parseFloat(releaseDurationField.value);
  }
}

function updateReleaseRadius() {
  if (simulationHistory.length === 0) {
    spreadKm = parseFloat(releaseRadiusField.value);
  }
}
// ========== VALIDATION ==========
function validateSimulation() {
  const errors = [];
  const lon = normalizeLongitude(parseFloat(rawLon));
  const lat = parseFloat(rawLat);

  if (!proteus) {
    errors.push("Simulation not initialized. Please wait.");
    return errors;
  }

  if (proteus.is_on_land(lon, lat)) {
    errors.push(
      `Release point (${lat.toFixed(2)}°, ${lon.toFixed(2)}°) is on land. Oil spills must start in water.`,
    );
  }

  const simStart = new Date(startYear, startMonth - 1, startDay);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const maxDate = new Date(today);
  const minDate = new Date(today);
  maxDate.setDate(today.getDate() + 10);
  minDate.setDate(today.getDate() - 30);
  if (simStart > maxDate) {
    errors.push(
      `Start date is beyond available forecast (max ${maxDate.toISOString().split("T")[0]})`,
    );
  }
  if (simStart < minDate) {
    errors.push(
      `Start date is before available range (min ${minDate.toISOString().split("T")[0]})`,
    );
  }

  const simEnd = new Date(simStart);
  simEnd.setDate(simEnd.getDate() + Math.ceil(totalDays));
  if (simEnd > maxDate) {
    errors.push(
      `Simulation would end beyond forecast range (${maxDate.toISOString().split("T")[0]})`,
    );
  }

  if (isNaN(totalDays) || totalDays <= 0)
    errors.push(`Total days must be positive.`);
  if (isNaN(releaseAmount) || releaseAmount <= 0)
    errors.push(`Release amount must be positive.`);
  if (isNaN(particleCount) || particleCount <= 0 || particleCount > 50000)
    errors.push(`Particle count must be between 1 and 50000.`);
  if (isNaN(spreadKm) || spreadKm < 0 || spreadKm > 50)
    errors.push(`Spread radius must be between 0 and 50 km.`);
  if (isNaN(releaseDuration) || releaseDuration < 0)
    errors.push(`Release duration must be positive`);
  return errors;
}

// ========== UI UPDATES ==========
function updateMarker() {
  if (simulationHistory.length === 0 && window.currentMarker)
    window.currentMarker.remove();
  if (simulationHistory.length === 0) {
    window.currentMarker = new maplibregl.Marker({
      color: "#244886",
      scale: 0.9,
    })
      .setLngLat([rawLon, rawLat])
      .addTo(map);
  }
}

function updateBoundingBox() {
  boundingBox = proteus.get_particle_bounding_box();
}

function updateStatsDisplay() {
  stranded.textContent = `${proteus.stranded_fraction().toFixed(1)}%`;
  emulsified.textContent = `${proteus.mass_weighted_emulsification().toFixed(1)}%`;
  evaporated.textContent = `${proteus.mass_weighted_evaporation().toFixed(1)}%`;
  totalMass.textContent = `${proteus.total_floating_mass_tons().toFixed(1)} t`;
}

function getStatsDisplay() {
  return {
    stranded: proteus.stranded_fraction().toFixed(1),
    emulsified: proteus.mass_weighted_emulsification().toFixed(1),
    evaporated: proteus.mass_weighted_evaporation().toFixed(1),
    total_mass: proteus.total_floating_mass_tons().toFixed(1),
  };
}

async function updateFields() {
  lonField.value = normalizeLongitude(rawLon).toFixed(2);
  latField.value = parseFloat(rawLat).toFixed(2);

  const currentDate = parseInt(
    `${startYear}${String(startMonth).padStart(2, "0")}${String(startDay).padStart(2, "0")}`,
  );
  const oceanTile = getTileIndices([normalizeLongitude(rawLon), rawLat], -80);
  preloader.preloadTiles(currentDate, oceanTile);
  await proteus.init_landmask(normalizeLongitude(rawLon), rawLat);
  updateMarker();
}

function updateOverlay() {
  map.setLayoutProperty(
    "overlay-layer",
    "visibility",
    overlay.checked ? "visible" : "none",
  );
}

// ========== VISUALIZATION ==========
function toggleVisualizationMode() {
  const isGrid = visualizationMode === "grid";
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

function toggleParticleMode() {
  if (visualizationMode === "particles") return;
  createHeatmapColorLegend(false);
  collapseLegendBtn.style.display = "none";
  openLegendBtn.style.display = "none";
  visualizationMode = "particles";
  toggleVisualizationMode();
  heatmapToggle.style.background = "none";
  heatmapToggle.style.color = "white";
  particleToggle.style.background = "white";
  particleToggle.style.color = "black";
  if (!playbackMode) updateParticleVisualization();
}

function toggleHeatmapMode() {
  if (visualizationMode === "grid") return;
  visualizationMode = "grid";
  toggleVisualizationMode();
  updateLegend();
  heatmapToggle.style.background = "white";
  heatmapToggle.style.color = "black";
  particleToggle.style.background = "none";
  particleToggle.style.color = "white";
  if (!playbackMode) updateGridVisualization();
}

function updateParticleVisualization() {
  const unstranded = proteus.get_unstranded_positions();
  const stranded = proteus.get_stranded_positions();

  const geojsonUnstranded = { type: "FeatureCollection", features: [] };
  const geojsonStranded = { type: "FeatureCollection", features: [] };

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

function updateGridVisualization() {
  const data = proteus.get_unstranded_positions_with_mass();
  if (!data?.length) {
    map
      .getSource("concentration")
      .setData({ type: "FeatureCollection", features: [] });
    return;
  }

  const lons = [],
    lats = [],
    masses = [];
  for (let i = 0; i < data.length; i += 3) {
    lons.push(data[i]);
    lats.push(data[i + 1]);
    masses.push(data[i + 2]);
  }

  heatmap.clear();
  heatmap.add_particles(lons, lats, masses);
  heatmap.smooth();
  const geojson = JSON.parse(
    heatmap.to_contour_geojson(
      getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
    ),
  );
  map.getSource("concentration").setData(geojson);
}

function createHeatmapColorLegend(show = true) {
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

// ========== TIMELINE ==========
function showTimeline() {
  if (!simulationHistory.length) return;
  timelineSlider.max = simulationHistory.length - 1;
  timelineSlider.value = simulationHistory.length - 1;
  document.getElementById("timeline-end").textContent =
    `Day ${simulationHistory[simulationHistory.length - 1].day}`;
  timelineContainer.style.display = "flex";
  updateTimelineDisplay(simulationHistory.length - 1);
  dayDisplay.textContent = "";
}

function updateTimelineDisplay(index) {
  if (index < 0 || index >= simulationHistory.length) return;
  const s = simulationHistory[index];
  timelineDay = index;
  document.getElementById("timeline-current").textContent = s.dateStr;
  timelineSlider.value = index;
  stranded.textContent = `${s.stats.stranded}%`;
  emulsified.textContent = `${s.stats.emulsified}%`;
  evaporated.textContent = `${s.stats.evaporated}%`;
  totalMass.textContent = `${s.stats.total_mass} t`;
  map.getSource("particles-unstranded").setData(s.unstrandedGeojson);
  map.getSource("particles-stranded").setData(s.strandedGeojson);
  map.getSource("concentration").setData(s.heatmapGeojson);
}

function timelinePlayback() {
  if (!timelinePlaying) return;
  if (timelineDay < simulationHistory.length - 1) {
    timelineDay++;
    updateTimelineDisplay(timelineDay);
    timelineAnimationId = setTimeout(
      () => requestAnimationFrame(timelinePlayback),
      playbackSpeed,
    );
  } else {
    timelinePlaying = false;
    timelinePlayBtn.style.display = "inline-block";
    timelinePauseBtn.style.display = "none";
  }
}

function updatePlaybackSpeed() {
  if (playbackSpeed === 100) {
    playbackSpeed = 50;
    timelineSpeed.textContent = "2x";
  } else if (playbackSpeed === 50) {
    playbackSpeed = 25;
    timelineSpeed.textContent = "4x";
  } else {
    playbackSpeed = 100;
    timelineSpeed.textContent = "1x";
  }
}

function updatePositionFromFields() {
  if (!isNaN(lonField.value) && !isNaN(latField.value)) {
    rawLon = parseFloat(lonField.value);
    rawLat = parseFloat(latField.value);
  }
  updateMarker();
}

function updateLegend() {
  if (visualizationMode === "grid") {
    createHeatmapColorLegend(true);
    if (legendCollapsed) {
      createHeatmapColorLegend(true);
      document.getElementById("concentration-legend").style.display = "none";
      openLegendBtn.style.display = "inline-block";
    } else {
      collapseLegendBtn.style.display = "inline-block";
    }
  }

}
// ========== SIMULATION CORE ==========
async function simulationStep(version) {
  if (!simulationRunning || version !== simulationVersion) return;

  try {
    const stepsPerDay = Math.round(1 / stepSize);
    const todayDateInt = proteus.current_date_int();

    await proteus.step(stepSize);

    if (stepCount % stepsPerDay === 0) {
      const oceanTiles = getTileIndices(proteus.get_positions(), -80);
      preloader.preloadTiles(todayDateInt, oceanTiles);
      preloader.preloadFutureSteps(todayDateInt, proteus.get_positions(), 2, 0);
      // Clean old tiles
      for (const url of window.__tileCache.keys()) {
        const match = url.match(/(\d{4})\/(\d{2})\/(\d{2})/);
        if (
          match &&
          parseInt(match[1] + match[2] + match[3]) < todayDateInt - 1
        ) {
          window.__tileCache.delete(url);
        }
      }
    }

    updateBoundingBox();

    if (stepCount % (stepsPerDay / 24) === 0) {
      captureSnapshot(Math.floor(proteus.current_day()));
      updateStatsDisplay();
    }

    if (
      visualizationMode === "grid" &&
      performance.now() - lastGridUpdate > GRID_UPDATE_INTERVAL
    ) {
      heatmap = new HeatmapGenerator(
        boundingBox[0] - GRID_SIZE * 2,
        boundingBox[1] + GRID_SIZE * 2,
        boundingBox[2] - GRID_SIZE * 2,
        boundingBox[3] + GRID_SIZE * 2,
        GRID_SIZE,
      );
      updateGridVisualization();
      lastGridUpdate = performance.now();
    } else if (visualizationMode !== "grid") {
      updateParticleVisualization();
    }

    dayDisplay.textContent = proteus.current_time_str();
    if (proteus.current_day() < totalDays + stepSize) {
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
  } finally {
    stepCount++;
  }
}

function captureSnapshot(day) {
  simulationHistory.push({
    day: day + 1,
    dateStr: proteus.current_time_str(),
    stats: getStatsDisplay(),
    unstrandedGeojson: getUnstrandedGeojson(),
    strandedGeojson: getStrandedGeojson(),
    heatmapGeojson: getHeatmapGeojson(),
  });
}

function getUnstrandedGeojson() {
  const positions = proteus.get_unstranded_positions();
  return {
    type: "FeatureCollection",
    features: Array.from({ length: positions.length / 2 }, (_, i) => ({
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: [positions[i * 2], positions[i * 2 + 1]],
      },
    })),
  };
}

function getStrandedGeojson() {
  const positions = proteus.get_stranded_positions();
  return {
    type: "FeatureCollection",
    features: Array.from({ length: positions.length / 2 }, (_, i) => ({
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: [positions[i * 2], positions[i * 2 + 1]],
      },
    })),
  };
}

function getHeatmapGeojson() {
  const data = proteus.get_unstranded_positions_with_mass();
  if (!data?.length) return { type: "FeatureCollection", features: [] };

  const lons = [],
    lats = [],
    masses = [];
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
  return JSON.parse(
    heatmap.to_contour_geojson(
      getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
    ),
  );
}

// ========== SIMULATION CONTROL ==========
async function startSimulation() {
  if (simulationRunning) return;

  const errors = validateSimulation();
  if (errors.length) {
    alert(`❌ Cannot start simulation:\n\n${errors.join("\n\n")}`);
    return;
  }

  simulationRunning = true;
  simulationVersion++;
  lastGridUpdate = 0;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  updatePositionFromFields();
  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();
  updateConcentrationLayer();

  if (window.currentMarker) window.currentMarker.remove();

  const lon = normalizeLongitude(rawLon);
  const lat = rawLat;
  oilType = oilMenu.value;

  if (autoZoom.checked && map.getZoom() < 5) {
    map.flyTo({
      center: [rawLon, rawLat],
      zoom: 6 - totalDays / 100,
      duration: 2000,
    });
  }

  proteus = new Proteus(
    lon,
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
    oilType,
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
  if (animationId) cancelAnimationFrame(animationId);
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
  simulationStep(simulationVersion);
}

async function resetSimulation() {
  simulationRunning = false;
  simulationVersion++;
  stepCount = 0;
  simulationHistory = [];
  playbackMode = false;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.4);
  if (animationId) cancelAnimationFrame(animationId);
  timelineContainer.style.display = "none";

  const lon = normalizeLongitude(rawLon);
  const lat = rawLat;
  oilType = oilMenu.value;

  proteus = new Proteus(
    lon,
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
    oilType,
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
  updateConcentrationLayer();
}

// ========== EXPORT/IMPORT ==========
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
    features: simulationHistory.map((snapshot) => ({
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
        geometries: [
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
          snapshot.heatmapGeojson?.features
            ? {
                type: "FeatureCollection",
                features: snapshot.heatmapGeojson.features,
              }
            : null,
        ].filter(Boolean),
      },
    })),
  };

  const blob = new Blob([JSON.stringify(exportData)], {
    type: "application/json",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `driftmap-full-${startYear}-${startMonth}-${startDay}.geojson`;
  a.click();
  URL.revokeObjectURL(a.href);
}

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
  updateLegend();

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  playbackMode = true;

  if (window.currentMarker) {
    window.currentMarker.remove();
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

// ========== INITIALIZATION ==========
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
      coordinates: [
        [-199.8, 85.05],
        [199.61, 85.05],
        [199.61, -80.0],
        [-199.8, -80.0],
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

async function initialize() {
  await init();
  setup_panic_hook();
  initGridLayer();

  const lon = normalizeLongitude(rawLon);
  const lat = rawLat;
  oilType = oilMenu.value;

  proteus = new Proteus(
    lon,
    lat,
    csValue,
    particleCount,
    spreadKm,
    startYear,
    startMonth,
    startDay,
    releaseAmount,
    releaseDuration,
    oilType,
  );
  updateMarker();
  await updateFields();

  const today = new Date();
  const minDate = new Date(today);
  minDate.setDate(today.getDate() - 30);
  const maxDate = new Date(today);
  maxDate.setDate(today.getDate() + 9);
  startDate.min = `${minDate.getFullYear()}-${String(minDate.getMonth() + 1).padStart(2, "0")}-${String(minDate.getDate()).padStart(2, "0")}`;
  startDate.max = `${maxDate.getFullYear()}-${String(maxDate.getMonth() + 1).padStart(2, "0")}-${String(maxDate.getDate()).padStart(2, "0")}`;
  startDate.value = `${startYear}-${String(startMonth).padStart(2, "0")}-${String(startDay).padStart(2, "0")}`;
}

// ========== EVENT LISTENERS ==========
map.on("click", (e) => {
  if (!simulationHistory.length) {
    rawLon = e.lngLat.lng.toFixed(2);
    rawLat = e.lngLat.lat.toFixed(2);
    updateFields();
    updateMarker();
  }
});

map.on("mousemove", (e) => {
  document.getElementById("coordinate-display").textContent =
    `${e.lngLat.lat.toFixed(2)}°, ${e.lngLat.lng.toFixed(2)}°`;
});

startBtn.addEventListener("click", startSimulation);
stopBtn.addEventListener("click", stopSimulation);
resumeBtn.addEventListener("click", resumeSimulation);
resetBtn.addEventListener("click", resetSimulation);
heatmapToggle.addEventListener("click", toggleHeatmapMode);
particleToggle.addEventListener("click", toggleParticleMode);
overlay.addEventListener("click", updateOverlay);
exportGeojsonBtn.addEventListener("click", exportScenario);
importGeojsonBtn.addEventListener("click", () => importGeojsonFile.click());
importGeojsonFile.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = (ev) => {
    try {
      loadGeoJsonResults(JSON.parse(ev.target.result));
    } catch (err) {
      alert("Invalid GeoJSON: " + err.message);
    }
  };
  reader.readAsText(file);
});

timelinePlayBtn.addEventListener("click", () => {
  timelinePlaying = true;
  timelinePlayback();
  timelinePlayBtn.style.display = "none";
  timelinePauseBtn.style.display = "inline-block";
});
timelinePauseBtn.addEventListener("click", () => {
  timelinePlaying = false;
  if (timelineAnimationId) clearTimeout(timelineAnimationId);
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
timelineSlider.addEventListener("input", (e) =>
  updateTimelineDisplay(parseInt(e.target.value)),
);

releaseAmountField.addEventListener("input", () => {
  releaseAmount = parseFloat(releaseAmountField.value);
  updateLegend();
});
releaseDurationField.addEventListener("input", () => {
  releaseDuration = parseFloat(releaseDurationField.value);
});
releaseRadiusField.addEventListener("input", () => {
  spreadKm = parseFloat(releaseRadiusField.value);
});
startDate.addEventListener("input", () => {
  const parts = startDate.value.split("-");
  startYear = parseInt(parts[0]);
  startMonth = parseInt(parts[1]);
  startDay = parseInt(parts[2]);
});
totalDaysField.addEventListener("input", () => {
  totalDays = parseFloat(totalDaysField.value);
});
latField.addEventListener("input", updatePositionFromFields);
lonField.addEventListener("input", updatePositionFromFields);

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
collapseLegendBtn.addEventListener("click", () => {
  document.getElementById("concentration-legend").style.display = "none";
  openLegendBtn.style.display = "inline-block";
  collapseLegendBtn.style.display = "none";
  legendCollapsed = true;
});
openLegendBtn.addEventListener("click", () => {
  document.getElementById("concentration-legend").style.display = "flex";
  openLegendBtn.style.display = "none";
  collapseLegendBtn.style.display = "inline-block";
  legendCollapsed = false;
});

// ========== COORDINATE DISPLAY ==========
const coordDisplay = document.createElement("div");
coordDisplay.id = "coordinate-display";
coordDisplay.style.cssText =
  "position: absolute; bottom: 18px; right: 45px; color: rgba(255,255,255,0.7); font-family: monospace; font-size: 12px; z-index: 10; pointer-events: none;";
document.body.appendChild(coordDisplay);

initialize().catch(console.error);