import { state } from './stores.js'
import { get } from './utils.js'

// === DOM EXPORT ===

export const dom = {
    // sidebar
    sidebar: get<HTMLElement>("sidebar"),

    // tabs and panels
    basicTab: get<HTMLElement>("tab-basic"),
    advancedTab: get<HTMLElement>("tab-advanced"),
    basicPanel: get<HTMLElement>("panel-basic"),
    advancedPanel: get<HTMLElement>("panel-advanced"),

    // model and tracer selectors
    modelMenu: get<HTMLSelectElement>("model-selector"),
    oilMenu: get<HTMLSelectElement>("oil-selector"),
    objectMenu: get<HTMLSelectElement>("object-selector"),
    plasticMenu: get<HTMLSelectElement>("plastic-selector"),

    modelMenuContainer: get<HTMLElement>("model-selector-container"),
    oilMenuContainer: get<HTMLElement>("oil-selector-container"),
    objectMenuContainer: get<HTMLElement>("object-selector-container"),
    plasticMenuContainer: get<HTMLElement>("plastic-selector-container"),

    basicModelMenu: get<HTMLSelectElement>("model-selector-basic"),
    basicOilMenu: get<HTMLSelectElement>("oil-selector-basic"),
    basicObjectMenu: get<HTMLSelectElement>("object-selector-basic"),
    basicPlasticMenu: get<HTMLSelectElement>("plastic-selector-basic"),

    basicModelMenuContainer: get<HTMLElement>("model-selector-container-basic"),
    basicOilMenuContainer: get<HTMLElement>("oil-selector-container-basic"),
    basicObjectMenuContainer: get<HTMLElement>("object-selector-container-basic"),
    basicPlasticMenuContainer: get<HTMLElement>("plastic-selector-container-basic"),

    // basic tab config
    startDate: get<HTMLInputElement>("start-day-selector"),
    totalDaysField: get<HTMLInputElement>("total-day-field"),
    latField: get<HTMLInputElement>("lat-field"),
    lonField: get<HTMLInputElement>("lon-field"),
    releaseAmountField: get<HTMLInputElement>("release-amount-field"),
    releaseDurationField: get<HTMLInputElement>("release-duration-field"),
    releaseRadiusField: get<HTMLInputElement>("release-radius-field"),

    // day display
    dayDisplay: get<HTMLElement>("current-day"),

    // simulation buttons
    startBtn: get<HTMLButtonElement>("start-simulation"),
    stopBtn: get<HTMLButtonElement>("stop-simulation"),
    resumeBtn: get<HTMLButtonElement>("resume-simulation"),
    resetBtn: get<HTMLButtonElement>("reset-simulation"),

    // visualization toggles
    heatmapToggle: get<HTMLButtonElement>("heatmap-toggle"),
    particleToggle: get<HTMLButtonElement>("particle-toggle"),
    overlay: get<HTMLInputElement>("overlay-checkbox"),

    // stats
    statsDisplay: get<HTMLElement>("stats-container"),
    emulsified: get<HTMLElement>("emulsified"),
    stranded: get<HTMLElement>("stranded"),
    evaporated: get<HTMLElement>("evaporated"),
    totalMass: get<HTMLElement>("total-mass"),

    // collapseLegendBtn: get<HTMLElement>("legend-collapse"),
    // openLegendBtn: get<HTMLElement>("legend-open"),
    // slideHandle: get<HTMLElement>('slide-handle'),

    // timeline
    timelineSlider: get<HTMLInputElement>("timeline-slider"),
    timelinePlayBtn: get<HTMLButtonElement>("timeline-play"),
    timelinePauseBtn: get<HTMLButtonElement>("timeline-pause"),
    timelineContainer: get<HTMLElement>("timeline-container"),
    timelineSpeed: get<HTMLButtonElement>("timeline-speed"),
    timelineRewind: get<HTMLButtonElement>("timeline-rewind"),

    // import/export
    exportGeojsonBtn: get<HTMLButtonElement>("export-geojson"),
    importGeojsonBtn: get<HTMLButtonElement>("import-geojson"),
    importGeojsonFile: get<HTMLInputElement>("import-geojson-file"),
    autoZoom: get<HTMLInputElement>("autozoom-checkbox"),
};

// === UI CHANGE FUNCTIONS ===

export function setActiveTab(tab: HTMLElement) {
    const isBasic = tab === dom.basicTab;

    dom.basicTab?.classList.toggle("active", isBasic);
    dom.advancedTab?.classList.toggle("active", !isBasic);
    
    dom.basicPanel.hidden = !isBasic;
    dom.advancedPanel.hidden = isBasic;
}

export function updateTracerMenu() {
    dom.oilMenuContainer.classList.add("hidden");
    dom.objectMenuContainer.classList.add("hidden");
    dom.plasticMenuContainer.classList.add("hidden");
    
    if (dom.modelMenu.value === "oil-weathering") {
        dom.oilMenuContainer.classList.remove("hidden");
    } else if (dom.modelMenu.value === "search-and-rescue") {
        dom.objectMenuContainer.classList.remove("hidden");
    } else if (dom.modelMenu.value === "plastic-drift") {
        dom.plasticMenuContainer.classList.remove("hidden");
    }
}

export function updateBasicTracerMenu() {
    dom.basicOilMenuContainer.classList.add("hidden");
    dom.basicObjectMenuContainer.classList.add("hidden");
    dom.basicPlasticMenuContainer.classList.add("hidden");
    
    if (dom.basicModelMenu.value === "oil-weathering") {
        dom.basicOilMenuContainer.classList.remove("hidden");
    } else if (dom.basicModelMenu.value === "search-and-rescue") {
        dom.basicObjectMenuContainer.classList.remove("hidden");
    } else if (dom.basicModelMenu.value === "plastic-drift") {
        dom.basicPlasticMenuContainer.classList.remove("hidden");
    }
}

// === STATS ===

export function updateStatsDisplay() {
  dom.stranded.textContent = `${state.proteus?.stranded_fraction()?.toFixed(1)}%`;
  dom.emulsified.textContent = `${state.proteus?.mass_weighted_emulsification()?.toFixed(1)}%`;
  dom.evaporated.textContent = `${state.proteus?.mass_weighted_evaporation()?.toFixed(1)}%`;
  dom.totalMass.textContent = `${state.proteus?.total_floating_mass_tons()?.toFixed(1)} t`;
}

export function getStatsDisplay() {
  return {
    stranded: state.proteus?.stranded_fraction()?.toFixed(1),
    emulsified: state.proteus?.mass_weighted_emulsification()?.toFixed(1),
    evaporated: state.proteus?.mass_weighted_evaporation()?.toFixed(1),
    total_mass: state.proteus?.total_floating_mass_tons()?.toFixed(1),
  };
}


// === INITIALIZATION ===
updateTracerMenu();
updateBasicTracerMenu();