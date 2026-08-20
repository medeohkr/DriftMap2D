import init, {
  Proteus,
  setup_panic_hook,
} from "../../pkg/proteus";
import { preloader } from "./preloader";
import { state } from './state';
import { dom } from './ui'
import { initMap, updateMarker, updateFields} from './map'
import { initGridLayer, updateLegend } from './visualization'
import { updateSimulationDate, updateTotalDays, updateReleaseAmount, updateReleaseDuration, updateReleaseRadius } from './simulation'
import { normalizeLongitude } from './utils'
import { getOilData } from './oils'

export async function initialize() {
  await init();
  setup_panic_hook();
  initMap();
  initGridLayer();

  const lon = normalizeLongitude(state.rawLon);
  const lat = state.rawLat;
  const oilData = getOilData();
  console.log("Oil data length:", oilData.length);
  console.log("First 100 chars:", oilData);
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
  updateMarker();
  await updateFields();

  const today = new Date();
  const minDate = new Date(today);
  minDate.setDate(today.getDate() - 30);
  const maxDate = new Date(today);
  maxDate.setDate(today.getDate() + 9);
  dom.startDate.min = `${minDate.getFullYear()}-${String(minDate.getMonth() + 1).padStart(2, "0")}-${String(minDate.getDate()).padStart(2, "0")}`;
  dom.startDate.max = `${maxDate.getFullYear()}-${String(maxDate.getMonth() + 1).padStart(2, "0")}-${String(maxDate.getDate()).padStart(2, "0")}`;
  dom.startDate.value = `${state.startYear}-${String(state.startMonth).padStart(2, "0")}-${String(state.startDay).padStart(2, "0")}`;
}