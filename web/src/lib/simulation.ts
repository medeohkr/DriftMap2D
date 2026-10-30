import {
    simulation,
    config,
    visualization,
    timeline,
    stats,
    history,
} from "./stores/index.svelte";
import { map, zoom } from "./map";
import { preloader } from "./preloader";
import {
    updateHeatmapVisualization,
    updateParticleVisualization,
    updateBoundingBox,
    updateConcentrationLayer,
    captureSnapshot,
} from "./visualization";
import { Proteus } from "../pkg/proteus_wasm";
import { showToast } from "./stores/toast.svelte";
import { getTotalDays, startDateTime, releasesToJson, getTracerJson } from "./utils";
import { validateSimulationStart } from "./validation";



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
        new Float32Array(config.diffusionCoeffs),
        config.forcingModel
    );
}


export async function simulationStep(version: number) {
    if (
        !simulation.simulationRunning ||
        version !== simulation.simulationVersion ||
        !simulation.proteus
    )
        return;

    try {
        await simulation.proteus?.step(simulation.stepCount);
    } catch (error) {
        if (!simulation.simulationRunning || simulation.simulationVersion !== version) return;
        resetSimulation();
        showToast(String(error), 7000);
        return;
    }

    if (simulation.simulationVersion !== version) {
        return;
    }

    try {
        const todayDateInt = simulation.proteus.get_current_date_int();
        const model = config.forcingModel;
        // native tiles span only 96 km so refresh their spatial margin each step
        if (model === "norkyst" || simulation.stepCount % Math.floor(1440 / config.timeStepMin) === 0) {
            void preloader.preloadOcean(todayDateInt, simulation.proteus.get_positions(), model, 1);
            preloader.pruneBefore(preloader.addDays(todayDateInt, -1));
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
        if (simulation.stepCount < getTotalDays() * Math.floor(1440 / config.timeStepMin)) {
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

let startInFlight = false;

export async function startSimulation(): Promise<boolean> {
    if (startInFlight) {
        showToast("Please wait... The simulation is loading");
        return false;
    }
    if (simulation.simulationRunning) return false;

    startInFlight = true;
    const version = ++simulation.simulationVersion;
    let activated = false;
    let started = false;
    try {
        if (!await validateSimulationStart()) return false;
        createProteus();
        simulation.simulationActive = true;
        simulation.simulationRunning = true;
        activated = true;
        visualization.lastGridUpdate = 0;
        map.setPaintProperty("unstranded-particles-layer", "circle-radius", visualization.particleRadius);
        map.setPaintProperty("stranded-particles-layer", "circle-radius", visualization.particleRadius);
        updateConcentrationLayer();
        zoom();
        started = true;
        simulationStep(version);
        return true;
    } catch (error) {
        if (simulation.simulationVersion === version) showToast(String(error), 7000);
        return false;
    } finally {
        startInFlight = false;
        if (activated && !started && simulation.simulationVersion === version) {
            stopSimulation();
            simulation.simulationActive = false;
        }
    }
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
