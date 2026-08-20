import { state } from "./state";
import { dom } from "./ui";
import { map } from "./map";
export function showTimeline() {
  if (!state.simulationHistory.length) return;
  dom.timelineSlider.max = state.simulationHistory.length - 1;
  dom.timelineSlider.value = state.simulationHistory.length - 1;
  document.getElementById("timeline-end").textContent =
    `Day ${state.simulationHistory[state.simulationHistory.length - 1].day}`;
  dom.timelineContainer.style.display = "flex";
  updateTimelineDisplay(state.simulationHistory.length - 1);
  dom.dayDisplay.textContent = "";
}

export function updateTimelineDisplay(index) {
  if (index < 0 || index >= state.simulationHistory.length) return;
  const s = state.simulationHistory[index];
  state.timelineDay = index;
  document.getElementById("timeline-current").textContent = s.dateStr;
  dom.timelineSlider.value = index;
  dom.stranded.textContent = `${s.stats.stranded}%`;
  dom.emulsified.textContent = `${s.stats.emulsified}%`;
  dom.evaporated.textContent = `${s.stats.evaporated}%`;
  dom.totalMass.textContent = `${s.stats.total_mass} t`;
  map.getSource("particles-unstranded").setData(s.unstrandedGeojson);
  map.getSource("particles-stranded").setData(s.strandedGeojson);
  map.getSource("concentration").setData(s.heatmapGeojson);
}

export function timelinePlayback() {
  if (!state.timelinePlaying) return;
  if (state.timelineDay < state.simulationHistory.length - 1) {
    state.timelineDay++;
    updateTimelineDisplay(state.timelineDay);
    state.timelineAnimationId = setTimeout(
      () => requestAnimationFrame(timelinePlayback),
      state.playbackSpeed,
    );
  } else {
    state.timelinePlaying = false;
    dom.timelinePlayBtn.style.display = "inline-block";
    dom.timelinePauseBtn.style.display = "none";
  }
}

export function updatePlaybackSpeed() {
  if (state.playbackSpeed === 100) {
    state.playbackSpeed = 50;
    dom.timelineSpeed.textContent = "2x";
  } else if (state.playbackSpeed === 50) {
    state.playbackSpeed = 25;
    dom.timelineSpeed.textContent = "4x";
  } else {
    state.playbackSpeed = 100;
    dom.timelineSpeed.textContent = "1x";
  }
}
