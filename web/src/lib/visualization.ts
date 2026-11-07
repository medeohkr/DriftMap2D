import { map } from "./map";
import {
    config,
    simulation,
    timeline,
    visualization,
    stats,
    history,
} from "./stores/index.svelte";
import { HeatmapCache } from "./heatmap";
import { ParticleFrameCache } from "./particleFrame";
import { ParticleLayer } from "./particleLayer";
import { getAverageReleasePosition, getTotalReleaseMass } from "./utils";

export const particleLayer = new ParticleLayer();
const heatmapCache = new HeatmapCache();
const frameCache = new ParticleFrameCache();

export function resetVisualizationCaches() {
    heatmapCache.clear();
    frameCache.clear();
}

export function getParticleFrame() {
    const source = simulation.proteus;
    if (!source) return null;
    return simulation.stepInFlight ? frameCache.peek(source) : frameCache.get(source);
}

export const COLORS = [
    "rgb(255, 255, 255)",
    "rgb(123, 218, 255)",
    "rgb(84, 152, 254)",
    "rgb(69, 97, 255)",
];

const CONCENTRATIONS = [0.008, 0.04, 0.2, 1];

export const PROBABILTIES = [0.25, 0.5, 0.75, 0.95];

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
                "fill-opacity": 1.0,
                "fill-antialias": false,
            },
        });
        map.addSource("sar-probability", {
            type: "geojson",
            data: {
                type: "FeatureCollection",
                features: [],
            },
            tolerance: 0,
            maxzoom: 24,
        });

        map.addLayer(particleLayer);
        particleLayer.setRadius(visualization.particleRadius);

        toggleVisualizationMode();
    });
}

export function getScaledConcentrations() {
    const scale = getTotalReleaseMass() * 0.005;
    return CONCENTRATIONS.map((c) => c * scale);
}

export function tonsPerKm2ToTonsPerCell(value: number) {
    const kmPerDegreeLon =
        111.12 * Math.cos((getAverageReleasePosition()[1] * Math.PI) / 180);
    const kmPerDegreeLat = 111.12;
    const cellAreaKm2 =
        kmPerDegreeLon *
        kmPerDegreeLat *
        visualization.gridSize *
        visualization.gridSize;

    return value * cellAreaKm2;
}

export function updateConcentrationLayer() {
    const thresholds =
        config.tracerType === "sar"
            ? PROBABILTIES
            : getScaledConcentrations().map(tonsPerKm2ToTonsPerCell);
    const stops = [];
    for (let i = 0; i < 4; i++) {
        stops.push(thresholds[i]);
        if (config.tracerType === "sar") {
            stops.push(COLORS[3 - i]);
        } else {
            stops.push(COLORS[i]);
        }
    }
    map.setPaintProperty("concentration-fill", "fill-color", [
        "interpolate",
        ["linear"],
        ["get", "concentration"],
        ...stops,
    ]);
}

export function toggleVisualizationMode() {
    const isHeatmap = visualization.visualizationMode === "heatmap";
    map.setLayoutProperty(
        "concentration-fill",
        "visibility",
        isHeatmap ? "visible" : "none",
    );
    particleLayer.setVisible(!isHeatmap);
}

export function toggleParticleMode() {
    if (visualization.visualizationMode === "particles") return;
    visualization.visualizationMode = "particles";
    toggleVisualizationMode();
    if (timeline.playbackMode || simulation.simulationActive)
        updateParticleVisualization();
}

export function toggleHeatmapMode() {
    if (visualization.visualizationMode === "heatmap") return;
    visualization.visualizationMode = "heatmap";
    toggleVisualizationMode();
    if (timeline.playbackMode || simulation.simulationActive)
        updateHeatmapVisualization();
}

export function updateParticleVisualization() {
    particleLayer.setRadius(visualization.particleRadius);
    if (timeline.playbackMode) {
        const snapshot = history.simulationHistory[timeline.timelineDay];
        if (snapshot) particleLayer.setPositions(snapshot.activePositions, snapshot.strandedPositions);
        return;
    }
    const proteus = simulation.proteus;
    if (proteus) {
        const frame = getParticleFrame();
        if (frame) particleLayer.setPositions(frame.activePositions, frame.strandedPositions);
    }
}

/** update only the visible view when changing the playback snapshot */
export function updateSnapshotVisualization() {
    if (visualization.visualizationMode === "particles") updateParticleVisualization();
    else updateHeatmapVisualization();
}

export function updateHeatmapVisualization() {
    const geojson = timeline.playbackMode
        ? history.simulationHistory[timeline.timelineDay]?.heatmapGeojson
        : getHeatmapGeojson();
    map.getSource("concentration").setData(geojson ?? { type: "FeatureCollection", features: [] });
}

export function captureSnapshot(day: number) {
    const frame = getParticleFrame();
    history.simulationHistory.push({
        day: day,
        dateStr: simulation.proteus?.current_time_str() ?? "",
        activePositions: frame?.activePositions ?? new Float32Array(),
        strandedPositions: frame?.strandedPositions ?? new Float32Array(),
        heatmapGeojson: getHeatmapGeojson(),
        stats: {
            ...(frame?.stats ?? stats),
        },
    });
}

export function getHeatmapGeojson() {
    const proteus = simulation.proteus;
    if (!proteus) return { type: "FeatureCollection", features: [] };
    if (simulation.stepInFlight) return heatmapCache.peek(proteus) ?? { type: "FeatureCollection", features: [] };
    const levels = config.tracerType === "sar"
        ? PROBABILTIES
        : getScaledConcentrations().map(tonsPerKm2ToTonsPerCell);
    return heatmapCache.get(proteus, visualization.gridSize, visualization.smoothLevel, levels);
}

export function updateBoundingBox() {
    visualization.boundingBox =
        getParticleFrame()?.boundingBox ?? new Float32Array();
}
