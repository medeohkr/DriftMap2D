import { map } from "./map";
import { config, simulation, timeline, visualization, stats, history} from "./stores/index.svelte";
import { HeatmapGenerator } from "../pkg/proteus";
import { getAverageReleasePosition } from "./utils";

export const COLORS = [
    "rgb(255, 255, 255)",
    "rgb(123, 218, 255)",
    "rgb(84, 152, 254)",
    "rgb(69, 97, 255)",
];

const CONCENTRATIONS = [
    0.0001, 0.001, 0.01, 0.1,
];

export const PROBABILTIES = [
    0.25, 0.5, 0.75, 0.95
]

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
                [-199.71, 85.05],
                [199.71, 85.05],
                [199.71, -80.0],
                [-199.71, -80.0],
            ],
        });
        map.addLayer({
            id: "overlay-layer",
            type: "raster",
            source: "overlay-png",
            paint: { "raster-opacity": 0.4 },
        });

        toggleVisualizationMode();
        updateOverlay(false);
    });
}

export function updateOverlay(checked: boolean) {
    map.setLayoutProperty(
        "overlay-layer",
        "visibility",
        checked ? "visible" : "none",
    );
}

export function getScaledConcentrations() {
    const scale = (simulation.proteus?.get_total_mass() ?? 0.0) / 100.0;
    return CONCENTRATIONS.map((c) => c * scale);
}

export function tonsPerKm2ToTonsPerCell(value: number) {
    const kmPerDegreeLon = 111.12 * Math.cos((getAverageReleasePosition()[1] * Math.PI) / 180);
    const kmPerDegreeLat = 111.12;
    const cellAreaKm2 =
        kmPerDegreeLon *
        kmPerDegreeLat *
        visualization.gridSize *
        visualization.gridSize;

    return value * cellAreaKm2;
}

export function updateConcentrationLayer() {
    const thresholds = config.tracerType === "sar" ? PROBABILTIES : getScaledConcentrations().map(tonsPerKm2ToTonsPerCell);
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
    map.setLayoutProperty(
        "unstranded-particles-layer",
        "visibility",
        isHeatmap ? "none" : "visible",
    );
    map.setLayoutProperty(
        "stranded-particles-layer",
        "visibility",
        isHeatmap ? "none" : "visible",
    );
}

export function toggleParticleMode() {
    if (visualization.visualizationMode === "particles") return;
    visualization.visualizationMode = "particles";
    toggleVisualizationMode();
    if (!timeline.playbackMode && simulation.simulationActive) updateParticleVisualization();
}

export function toggleHeatmapMode() {
    if (visualization.visualizationMode === "heatmap") return;
    visualization.visualizationMode = "heatmap";
    toggleVisualizationMode();
    if (!timeline.playbackMode && simulation.simulationActive) updateHeatmapVisualization();
}

export function updateParticleVisualization() {
    const unstranded = simulation.proteus?.get_unstranded_positions();
    const stranded = simulation.proteus?.get_stranded_positions();

    const geojsonUnstranded = {
        type: "FeatureCollection",
        features: [] as any[],
    };
    const geojsonStranded = {
        type: "FeatureCollection",
        features: [] as any[],
    };

    if (unstranded && stranded) {
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
                geometry: {
                    type: "Point",
                    coordinates: [stranded[i], stranded[i + 1]],
                },
            });
        }
    }

    map.getSource("particles-unstranded").setData(
        geojsonUnstranded,
    );
    map.getSource("particles-stranded").setData(
        geojsonStranded,
    );
}

export function updateHeatmapVisualization() {
    const geojson = getHeatmapGeojson();
    map.getSource("concentration").setData(geojson);
}

export function buildHeatmap() {
    const data = simulation.proteus?.get_unstranded_positions_with_mass() ?? new Float32Array;

    const { lonMin, lonMax, needsShift } = getShiftedBounds(data);

    const padding = visualization.gridSize * 3;
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
    visualization.heatmap.smooth(2);
    
    if (config.tracerType === "sar") {
        visualization.heatmap.normalize_probability();
    }
}

export function captureSnapshot(day: Number) {
    history.simulationHistory.push({
        day: day,
        dateStr: simulation.proteus?.current_time_str(),
        unstrandedGeojson: getUnstrandedGeojson(),
        strandedGeojson: getStrandedGeojson(),
        heatmapGeojson: getHeatmapGeojson(),
        stranded: stats.stranded,
        emulsified: stats.emulsified,
        evaporated: stats.evaporated,
        totalMass: stats.totalMass
    });
}

export function getUnstrandedGeojson() {
    const positions = simulation.proteus?.get_unstranded_positions();
    if (positions) {
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
}

export function getStrandedGeojson() {
    const positions = simulation.proteus?.get_stranded_positions();
    if (positions) {
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
}

export function getHeatmapGeojson() {
    buildHeatmap();
    if (config.tracerType === "sar") {
        console.log(visualization.heatmap.to_probability_contour_geojson(new Float32Array(PROBABILTIES)))
        return JSON.parse(
            visualization.heatmap.to_probability_contour_geojson(new Float32Array(PROBABILTIES))
        )
    } else {
        console.log(visualization.heatmap.to_contour_geojson(
                getScaledConcentrations().map(tonsPerKm2ToTonsPerCell),
            ))
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
