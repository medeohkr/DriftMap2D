import { Proteus } from "../../pkg/proteus";
import { preloader } from "./preloader";
import { state } from './state';
import { dom } from './ui'
import { normalizeLongitude, getTileIndices, updateBoundingBox, get} from './utils'
import { updateStatsDisplay, getStatsDisplay } from './ui'
import { initGridLayer, updateGridVisualization, updateParticleVisualization, updateConcentrationLayer, buildHeatmap, getScaledConcentrations, tonsPerKm2ToTonsPerCell } from './visualization'
import { updatePositionFromFields, zoom, map, updateFields, updateMarker } from './map'
import { showTimeline } from './timeline'
import { exportScenario, loadGeoJsonResults } from './importExport'
import { updateLegend, vizParams} from './visualization'
import { getOilData } from './oils'

export function updateSimulationDate() {
  if (state.simulationHistory.length === 0) {
    let inputDate = dom.startDate.value.split("-");
    state.startYear = parseInt(inputDate[0]);
    state.startMonth = parseInt(inputDate[1]);
    state.startDay = parseInt(inputDate[2]);
  }
}

export function updateTotalDays() {
  if (state.simulationHistory.length === 0) {
    state.totalDays = parseFloat(dom.totalDaysField.value);
  }
}

export function updateReleaseAmount() {
  if (state.simulationHistory.length === 0) {
    state.releaseAmount = parseFloat(dom.releaseAmountField.value);
    updateLegend();
  }
}

export function updateReleaseDuration() {
  if (state.simulationHistory.length === 0) {
    state.releaseDuration = parseFloat(dom.releaseDurationField.value);
  }
}

export function updateReleaseRadius() {
  if (state.simulationHistory.length === 0) {
    state.spreadKm = parseFloat(dom.releaseRadiusField.value);
  }
}
// ========== VALIDATION ==========
export function validateSimulation() {
  const errors = [];
  const lon = normalizeLongitude(state.rawLon);
  const lat = state.rawLat;

  if (!state.proteus) {
    errors.push("Simulation not initialized. Please wait.");
    return errors;
  }

  if (state.proteus.is_on_land(lon, lat)) {
    errors.push(
      `Release point (${lat.toFixed(2)}°, ${lon.toFixed(2)}°) is on land. Oil spills must start in water.`,
    );
  }

  const simStart = new Date(state.startYear, state.startMonth - 1, state.startDay);
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
  simEnd.setDate(simEnd.getDate() + Math.ceil(state.totalDays));
  if (simEnd > maxDate) {
    errors.push(
      `Simulation would end beyond forecast range (${maxDate.toISOString().split("T")[0]})`,
    );
  }

  if (isNaN(state.totalDays) || state.totalDays <= 0)
    errors.push(`Total days must be positive.`);
  if (isNaN(state.releaseAmount) || state.releaseAmount <= 0)
    errors.push(`Release amount must be positive.`);
  if (isNaN(state.particleCount) || state.particleCount <= 0 || state.particleCount > 50000)
    errors.push(`Particle count must be between 1 and 50000.`);
  if (isNaN(state.spreadKm) || state.spreadKm < 0 || state.spreadKm > 50)
    errors.push(`Spread radius must be between 0 and 50 km.`);
  if (isNaN(state.releaseDuration) || state.releaseDuration < 0)
    errors.push(`Release duration must be positive`);
  if (isNaN(state.rawLon))
    errors.push(`Release location must have a longitude value`)
  if (isNaN(state.rawLat) || state.rawLat < -80 || state.rawLat > 85)
    errors.push(`Latitude must be between -80° and 85°`)
  return errors;
}

// ========== SIMULATION CORE ==========
export async function simulationStep(version) {
  if (!state.simulationRunning || version !== state.simulationVersion) return;

  try {
    const todayDateInt = state.proteus.get_current_date_int();

    await state.proteus.step(state.stepCount);

    if (state.stepCount % state.stepsPerDay === 0) {
      const oceanTiles = getTileIndices(state.proteus.get_positions(), -80);
      preloader.preloadTiles(todayDateInt, oceanTiles);
      preloader.preloadFutureSteps(todayDateInt, state.proteus.get_positions(), 2, 0);
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

    if (state.stepCount % (state.stepsPerDay / 24) === 0) {
      captureSnapshot(Math.floor(state.proteus.current_day()));
      updateStatsDisplay();
    }

    if (
      state.visualizationMode === "grid" &&
      performance.now() - state.lastGridUpdate > vizParams.GRID_UPDATE_INTERVAL
    ) {
      updateGridVisualization();
      state.lastGridUpdate = performance.now();
    } else if (state.visualizationMode !== "grid") {
      updateParticleVisualization();
    }

    dom.dayDisplay.textContent = state.proteus.current_time_str();
    if (state.stepCount < state.totalDays * state.stepsPerDay) {
      state.animationId = requestAnimationFrame(() => simulationStep(version));
    } else {
      state.simulationRunning = false;
      showTimeline();
      state.playbackMode = true;
      dom.startBtn.style.display = "none";
      dom.stopBtn.style.display = "none";
      dom.resumeBtn.style.display = "none";
      dom.exportGeojsonBtn.style.display = "inline-block";
    }
  } finally {
    state.stepCount++;
  }
}

export function captureSnapshot(day: Number) {
  state.simulationHistory.push({
    day: day,
    dateStr: state.proteus.current_time_str(),
    stats: getStatsDisplay(),
    unstrandedGeojson: getUnstrandedGeojson(),
    strandedGeojson: getStrandedGeojson(),
    heatmapGeojson: getHeatmapGeojson(),
  });
}

export function getUnstrandedGeojson() {
  const positions = state.proteus.get_unstranded_positions();
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

export function getStrandedGeojson() {
  const positions = state.proteus.get_stranded_positions();
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

export function getHeatmapGeojson() {
  buildHeatmap();
  return JSON.parse(
    state.heatmap.to_contour_geojson(
      getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
    ),
  );
}

// ========== SIMULATION CONTROL ==========
export async function startSimulation() {
  if (state.simulationRunning) return;

  if (state.landmaskPromise) {
      await state.landmaskPromise;
  }

  const errors = validateSimulation();
  if (errors.length) {
    alert(`❌ Cannot start simulation:\n\n${errors.join("\n\n")}`);
    return;
  }

  state.simulationRunning = true;
  state.simulationVersion++;
  state.lastGridUpdate = 0;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  updatePositionFromFields();
  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();
  updateConcentrationLayer();
  zoom();

  if (state.currentMarker) state.currentMarker.remove();

  const lon = normalizeLongitude(state.rawLon);
  const lat = state.rawLat;
  // oilType = state.oilMenu.value;


  state.proteus = new Proteus(
    lon,
    lat,
    state.csValue,
    state.particleCount,
    state.spreadKm,
    state.startYear,
    state.startMonth,
    state.startDay,
    state.stepsPerDay,
    state.releaseAmount,
    state.releaseDuration,
    getOilData(),
  );
  
  simulationStep(state.simulationVersion);

  dom.startBtn.style.display = "none";
  dom.stopBtn.style.display = "inline-flex";
  dom.resumeBtn.style.display = "none";
  dom.exportGeojsonBtn.style.display = "none";
  dom.statsDisplay.style.display = "grid";
}

export function stopSimulation() {
  state.simulationRunning = false;
  if (state.animationId) cancelAnimationFrame(state.animationId);
  dom.startBtn.style.display = "none";
  dom.stopBtn.style.display = "none";
  dom.resumeBtn.style.display = "inline-flex";
  dom.exportGeojsonBtn.style.display = "none";
}

export function resumeSimulation() {
  if (state.simulationRunning) return;
  state.simulationRunning = true;
  state.simulationVersion++;
  dom.startBtn.style.display = "none";
  dom.stopBtn.style.display = "inline-flex";
  dom.resumeBtn.style.display = "none";
  simulationStep(state.simulationVersion);
}

export async function resetSimulation() {
  state.simulationRunning = false;
  state.simulationVersion++;
  state.stepCount = 0;
  state.simulationHistory = [];
  state.playbackMode = false;

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.4);
  if (state.animationId) cancelAnimationFrame(state.animationId);
  dom.timelineContainer.style.display = "none";

  const lon = normalizeLongitude(state.rawLon);
  const lat = state.rawLat;
  // oilType = state.oilMenu.value;

  state.proteus = new Proteus(
    lon,
    lat,
    state.csValue,
    state.particleCount,
    state.spreadKm,
    state.startYear,
    state.startMonth,
    state.startDay,
    state.stepsPerDay,
    state.releaseAmount,
    state.releaseDuration,
    getOilData(),
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

  dom.dayDisplay.textContent = "";
  dom.startBtn.style.display = "inline-flex";
  dom.stopBtn.style.display = "none";
  dom.resumeBtn.style.display = "none";
  dom.exportGeojsonBtn.style.display = "none";
  dom.statsDisplay.style.display = "none";

  updateFields();
  updateMarker();
  updateConcentrationLayer();
  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseRadius();
  updateReleaseDuration();
}