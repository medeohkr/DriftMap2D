import { map } from "./map";
import {
    config,
    simulation,
    timeline,
    visualization,
    stats,
    history,
} from "./stores/index.svelte";
import { HeatmapGenerator } from "../pkg/proteus_wasm";
import { ParticleLayer } from "./particleLayer";
import { getAverageReleasePosition, getTotalReleaseMass } from "./utils";

export const particleLayer = new ParticleLayer();

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
        particleLayer.setPositions(proteus.get_unstranded_positions(), proteus.get_stranded_positions());
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

export function buildHeatmap() {
    const data =
        simulation.proteus?.get_unstranded_positions_with_mass() ??
        new Float32Array();

    const { lonMin, lonMax, needsShift } = getShiftedBounds(data);

    const padding = visualization.gridSize * (visualization.smoothLevel + 1);
    visualization.heatmap = new HeatmapGenerator(
        lonMin - padding,
        lonMax + padding,
        visualization.boundingBox[2] - padding,
        visualization.boundingBox[3] + padding,
        visualization.gridSize,
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

    visualization.heatmap.clear();
    visualization.heatmap.add_particles(lons, lats, masses);
    visualization.heatmap.smooth(visualization.smoothLevel);

    if (config.tracerType === "sar") {
        visualization.heatmap.normalize_probability();
    }
}

export function captureSnapshot(day: number) {
    history.simulationHistory.push({
        day: day,
        dateStr: simulation.proteus?.current_time_str() ?? "",
        activePositions: simulation.proteus?.get_unstranded_positions() ?? new Float32Array(),
        strandedPositions: simulation.proteus?.get_stranded_positions() ?? new Float32Array(),
        heatmapGeojson: getHeatmapGeojson(),
        stats: {
            activeParticles: stats.activeParticles,
            stranded: stats.stranded,
            emulsified: stats.emulsified,
            evaporated: stats.evaporated,
            totalMass: stats.totalMass,
        },
    });
}

export function getHeatmapGeojson() {
    buildHeatmap();
    if (config.tracerType === "sar") {
        return JSON.parse(
            visualization.heatmap.to_probability_contour_geojson(
                new Float32Array(PROBABILTIES),
            ),
        );
    } else {
        return JSON.parse(
            visualization.heatmap.to_contour_geojson(
                getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
            ),
        );
    }
}

export function getShiftedBounds(positions: Float32Array) {
    let lonMin = visualization.boundingBox[0];
    let lonMax = visualization.boundingBox[1];

    if (lonMax - lonMin > 180) {
        let shiftedMin = Infinity;
        let shiftedMax = -Infinity;

        for (let i = 0; i < positions.length; i += 3) {
            let lon = positions[i];
            if (lon < 0) lon += 360;
            if (lon < shiftedMin) shiftedMin = lon;
            if (lon > shiftedMax) shiftedMax = lon;
        }

        return {
            lonMin: shiftedMin,
            lonMax: shiftedMax,
            needsShift: true,
        };
    }

    return {
        lonMin: visualization.boundingBox[0],
        lonMax: visualization.boundingBox[1],
        needsShift: false,
    };
}

export function updateBoundingBox() {
    visualization.boundingBox =
        simulation.proteus?.get_particle_bounding_box() ?? new Float32Array();
}
