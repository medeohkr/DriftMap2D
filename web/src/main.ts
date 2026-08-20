// main.js
import { initialize } from './lib/initialize.js';
import { initMap, map, updatePositionFromFields, updateMarker, updateFields} from './lib/map.js';
import { dom, setActiveTab, updateTracerMenu, updateBasicTracerMenu } from './lib/ui.js';
import { startSimulation, stopSimulation, resumeSimulation, resetSimulation, updateReleaseAmount, updateReleaseDuration, updateReleaseRadius, updateSimulationDate, updateTotalDays } from './lib/simulation.js';
import { exportScenario, loadGeoJsonResults } from './lib/importExport.js';
import { toggleHeatmapMode, toggleParticleMode, initGridLayer, updateOverlay } from './lib/visualization.js';
import { showTimeline, updateTimelineDisplay, timelinePlayback, updatePlaybackSpeed } from './lib/timeline.js';
import { state } from './lib/state.js';

// === INITIALIZE ===
initialize().catch(console.error);

// === EVENT LISTENERS ===

// tabs
dom.basicTab.addEventListener("click", () => setActiveTab(dom.basicTab));
dom.advancedTab.addEventListener("click", () => setActiveTab(dom.advancedTab));

// model selector
dom.modelMenu.addEventListener("change", updateTracerMenu);
dom.basicModelMenu.addEventListener("change", updateBasicTracerMenu);

// basic tab fields
dom.releaseAmountField.addEventListener("input", updateReleaseAmount);
dom.releaseDurationField.addEventListener("input", updateReleaseDuration);
dom.releaseRadiusField.addEventListener("input", updateReleaseRadius);
dom.startDate.addEventListener("input", updateSimulationDate);
dom.totalDaysField.addEventListener("input", updateTotalDays);
dom.latField.addEventListener("input", updatePositionFromFields);
dom.lonField.addEventListener("input", updatePositionFromFields);

// simulation buttons
dom.startBtn.addEventListener("click", startSimulation);
dom.stopBtn.addEventListener("click", stopSimulation);
dom.resumeBtn.addEventListener("click", resumeSimulation);
dom.resetBtn.addEventListener("click", resetSimulation);

// visualization
dom.heatmapToggle.addEventListener("click", toggleHeatmapMode);
dom.particleToggle.addEventListener("click", toggleParticleMode);
dom.overlay.addEventListener("click", updateOverlay);

// timeline
dom.timelinePlayBtn.addEventListener("click", () => {
  state.timelinePlaying = true;
  timelinePlayback();
  dom.timelinePlayBtn.style.display = "none";
  dom.timelinePauseBtn.style.display = "inline-block";
});
dom.timelinePauseBtn.addEventListener("click", () => {
  state.timelinePlaying = false;
  if (state.timelineAnimationId) clearTimeout(state.timelineAnimationId);
  dom.timelinePlayBtn.style.display = "inline-block";
  dom.timelinePauseBtn.style.display = "none";
});
dom.timelineSpeed.addEventListener("click", updatePlaybackSpeed);
dom.timelineRewind.addEventListener("click", () => {
  state.timelinePlaying = false;
  updateTimelineDisplay(0);
  dom.timelinePlayBtn.style.display = "inline-block";
  dom.timelinePauseBtn.style.display = "none";
});
dom.timelineSlider.addEventListener("input", (e: Event) =>
  updateTimelineDisplay(parseInt((e.target as HTMLInputElement)?.value)),
);

// import/export
dom.exportGeojsonBtn.addEventListener("click", exportScenario);
dom.importGeojsonBtn.addEventListener("click", () => dom.importGeojsonFile.click());
dom.importGeojsonFile.addEventListener("change", (e) => {
  const file: File | null = (e.target as HTMLInputElement)?.files?.[0];
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


// legend collapse
// dom.collapseLegendBtn.addEventListener("click", () => {
//   document.getElementById("concentration-legend").style.display = "none";
//   dopenLegendBtn.style.display = "inline-block";
//   collapseLegendBtn.style.display = "none";
//   legendCollapsed = true;
// });
// dom.openLegendBtn.addEventListener("click", () => {
//   document.getElementById("concentration-legend").style.display = "flex";
//   openLegendBtn.style.display = "none";
//   collapseLegendBtn.style.display = "inline-block";
//   legendCollapsed = false;
// });

// Initialize
