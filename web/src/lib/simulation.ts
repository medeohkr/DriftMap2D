import {
    simulation,
    config,
    visualization,
    timeline,
    stats,
    history,
} from "./stores/index.svelte";
import { map, zoom } from "./map";
import {
    updateHeatmapVisualization,
    updateParticleVisualization,
    updateBoundingBox,
    updateConcentrationLayer,
    captureSnapshot,
    particleLayer,
    resetVisualizationCaches,
    getParticleFrame,
} from "./visualization";
import { SimulationLoop } from "./simulationLoop";
import { Proteus, clear_preloads } from "../pkg/proteus_wasm";
import { showToast } from "./stores/toast.svelte";
import { getTotalDays, startDateTime, releasesToJson, getTracerJson, getCentroidPosition } from "./utils";
import { validateSimulationStart } from "./validation";



export function createProteus() {
    resetVisualizationCaches();
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


let loop: SimulationLoop | null = null;
let loopVersion = -1;
let loopEngine: Proteus | null = null;

export function simulationStep(version: number): Promise<void> {
    const engine = simulation.proteus;
    if (!simulation.simulationRunning || version !== simulation.simulationVersion || !engine)
        return Promise.resolve();
    if (loop && loopVersion === version && loopEngine === engine) return loop.start();
    loop?.cancel();
    loopVersion = version;
    loopEngine = engine;
    const current = () => version === simulation.simulationVersion && simulation.proteus === engine;
    loop = new SimulationLoop({
        isCurrent: current,
        advance: async () => {
            const step = simulation.stepCount;
            simulation.stepInFlight = true;
            try {
                await engine.step(step);
            } finally {
                if (current()) simulation.stepInFlight = false;
            }
            if (!current()) return true;
            simulation.stepCount = step + 1;
            const finalStep = step >= getTotalDays() * Math.floor(1440 / config.timeStepMin);
            if (step % visualization.snapshotInterval === 0 || finalStep) {
                captureSnapshot(engine.current_day());
            }
            return finalStep;
        },
        render: force => {
            if (!current()) return;
            updateStats();
            updateBoundingBox();
            if (visualization.visualizationMode === "heatmap") {
                if (force || performance.now() - visualization.lastGridUpdate > visualization.gridUpdateInterval) {
                    updateHeatmapVisualization();
                    visualization.lastGridUpdate = performance.now();
                }
            } else {
                updateParticleVisualization();
            }
            simulation.currentTime = engine.current_time_str();
        },
        complete: () => {
            simulation.simulationRunning = false;
            timeline.playbackMode = true;
            console.log(`total simulation time: ${((performance.now() - simulationStartedAt) / 1000).toFixed(3)} s`);
            console.log("centroid:", getCentroidPosition());
        },
        error: error => {
            if (!current()) return;
            void resetSimulation();
            showToast(String(error), 7000);
        },
    });
    return loop.start();
}

let startInFlight = false;
let simulationStartedAt = 0;

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
        simulationStartedAt = performance.now();
        createProteus();
        simulation.simulationActive = true;
        simulation.simulationRunning = true;
        activated = true;
        visualization.lastGridUpdate = 0;
        particleLayer.setRadius(visualization.particleRadius);
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
    loop?.pause();
}

export function resumeSimulation() {
    if (simulation.simulationRunning) return;
    simulation.simulationRunning = true;
    if (!simulation.simulationActive || timeline.playbackMode) {
        simulation.simulationRunning = false;
        return;
    }
    void simulationStep(simulation.simulationVersion);
}

export async function resetSimulation() {
    loop?.cancel();
    loop = null;
    loopEngine = null;
    simulation.stepInFlight = false;
    clear_preloads();
    resetVisualizationCaches();

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
        particleLayer.clear();
    }
}

export function updateStats() {
    const frame = getParticleFrame();
    if (frame) Object.assign(stats, frame.stats);
}
