import {
    simulation,
    config,
    visualization,
    timeline,
    stats,
    history,
} from "./stores/index.svelte";
import { awaitLandmaskPreload, map, zoom } from "./map";
import { preloader } from "./preloader";
import {
    updateHeatmapVisualization,
    updateParticleVisualization,
    updateBoundingBox,
    updateConcentrationLayer,
    captureSnapshot,
} from "./visualization";
import { Proteus } from "../pkg/proteus_wasm";
import { getTotalDays, startDateTime, releasesToJson, getTracerJson } from "./utils";



export function createProteus() {
    simulation.proteus = new Proteus(
        config.tracerType,
        getTracerJson(),
        startDateTime(),
        releasesToJson(),
        config.particleCount,
        config.timeStepMin,
        config.advectionScheme,
        config.diffusionScheme,
        new Float32Array(config.diffusionCoeffs)
    );
}


export async function simulationStep(version: number) {
    if (
        !simulation.simulationRunning ||
        version !== simulation.simulationVersion ||
        !simulation.proteus
    )
        return;

    await simulation.proteus?.step(simulation.stepCount);

    if (simulation.simulationVersion !== version) {
        return;
    }

    try {
        const todayDateInt = simulation.proteus.get_current_date_int();
        if (simulation.stepCount % Math.floor(1440 / config.timeStepMin) === 0) {
            const oceanTiles = preloader.getTileIndicesForOcean(
                simulation.proteus.get_positions(),
            );
            preloader.preloadTiles(todayDateInt, oceanTiles);
            preloader.preloadFutureSteps(
                todayDateInt,
                oceanTiles,
                1,
            );

            if (window.__tileCache) {
                for (const url of window.__tileCache?.keys()) {
                    const match = url.match(/(\d{4})\/(\d{2})\/(\d{2})/);
                    if (
                        match &&
                        parseInt(match[1] + match[2] + match[3]) <
                            todayDateInt - 1
                    ) {
                        window.__tileCache.delete(url);
                    }
                }
            }
        }
        updateBoundingBox();

        if (simulation.stepCount % visualization.snapshotInterval === 0) {
            updateStats();
            captureSnapshot(simulation.proteus.current_day());
        }

        if (
            visualization.visualizationMode === "heatmap" &&
            performance.now() - visualization.lastGridUpdate >
                visualization.gridUpdateInterval
        ) {
            updateHeatmapVisualization();
            visualization.lastGridUpdate = performance.now();
        } else if (visualization.visualizationMode !== "heatmap") {
            updateParticleVisualization();
        }

        simulation.currentTime = simulation.proteus.current_time_str();
        if (simulation.stepCount < getTotalDays() * Math.floor(1440 / config.timeStepMin)
            && simulation.proteus.total_floating_mass_tons() > 0.0
        ) {
            simulation.animationId = requestAnimationFrame(() =>
                simulationStep(version)
            );
        } else {
            simulation.simulationRunning = false;
            timeline.playbackMode = true;
        }
    } finally {
        simulation.stepCount++;
    }
}

export async function startSimulation() {
    if (simulation.simulationRunning) return;

    simulation.simulationActive = true;
    simulation.simulationRunning = true;
    simulation.simulationVersion++;
    visualization.lastGridUpdate = 0;

    map.setPaintProperty("unstranded-particles-layer", "circle-radius", visualization.particleRadius)
    map.setPaintProperty("stranded-particles-layer", "circle-radius", visualization.particleRadius)

    createProteus();
    updateConcentrationLayer();
    zoom();
    
    simulationStep(simulation.simulationVersion);
}

export function stopSimulation() {
    simulation.simulationRunning = false;
    if (simulation.animationId) cancelAnimationFrame(simulation.animationId);
}

export function resumeSimulation() {
    if (simulation.simulationRunning) return;
    simulation.simulationRunning = true;
    simulation.simulationVersion++;
    simulationStep(simulation.simulationVersion);
}

export async function resetSimulation() {
    if (simulation.animationId) cancelAnimationFrame(simulation.animationId);

    simulation.simulationActive = false;
    simulation.simulationRunning = false;
    simulation.simulationVersion++;
    simulation.stepCount = 0;
    history.simulationHistory = [];
    timeline.playbackMode = false;

    map.setPaintProperty("overlay-layer", "raster-opacity", 0.4);

    if (map) {
        map.getSource("concentration").setData({
            type: "FeatureCollection",
            features: [],
        });
        map.getSource("particles-unstranded").setData({
            type: "FeatureCollection",
            features: [],
        });
        map.getSource("particles-stranded").setData({
            type: "FeatureCollection",
            features: [],
        });
    }
}

export function updateStats() {
    stats.stranded =
        simulation.proteus?.stranded_fraction()?.toFixed(1) ?? "0.0";
    stats.emulsified =
        simulation.proteus?.mass_weighted_emulsification()?.toFixed(1) ?? "0.0";
    stats.evaporated =
        simulation.proteus?.mass_weighted_evaporation()?.toFixed(1) ?? "0.0";
    stats.totalMass =
        simulation.proteus?.total_floating_mass_tons()?.toFixed(1) ?? "0.0";
    stats.activeParticles =
        simulation.proteus?.active_particle_count().toString() ?? "0.0";
}