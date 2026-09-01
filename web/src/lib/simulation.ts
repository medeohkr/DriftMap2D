import {
    simulation,
    config,
    visualization,
    timeline,
    stats,
    history,
} from "./stores.svelte";
import { map, updateMarker, normalizeLongitude, zoom } from "./map";
import { preloader } from "./preloader";
import {
    updateHeatmapVisualization,
    updateParticleVisualization,
    updateBoundingBox,
    updateConcentrationLayer,
    captureSnapshot,
} from "./visualization";
import { getOilData } from "./oils";
import { Proteus } from "../pkg/proteus";

export function validateSimulation() {
    const errors = [];
    const lon = normalizeLongitude(config.lon);
    const lat = config.lat;

    if (!simulation.proteus) {
        errors.push("Simulation not initialized. Please wait.");
        return errors;
    }

    if (simulation.proteus.is_on_land(lon, lat)) {
        errors.push(
            `Release point (${lat.toFixed(2)}°, ${lon.toFixed(2)}°) is on land. Oil spills must start in water.`,
        );
    }

    const simStart = new Date(config.startDate);
    simStart.setHours(0, 0, 0, 0);

    const today = new Date();
    today.setHours(0, 0, 0, 0);

    const maxDate = new Date(today);
    const minDate = new Date(today);

    maxDate.setDate(today.getDate() + 10);
    minDate.setDate(today.getDate() - 30);
    if (simStart > maxDate) {
        errors.push(
            `Start date is beyond available forecast (max ${maxDate.toISOString().split("T")[0]})`,
        );
    }
    if (simStart < minDate) {
        errors.push(
            `Start date is before available range (min ${minDate.toISOString().split("T")[0]})`,
        );
    }

    const simEnd = new Date(simStart);
    simEnd.setDate(simEnd.getDate() + Math.ceil(config.totalDays));
    if (simEnd > maxDate) {
        errors.push(
            `Simulation would end beyond forecast range (${maxDate.toISOString().split("T")[0]})`,
        );
    }

    if (isNaN(config.totalDays) || config.totalDays <= 0)
        errors.push(`Total days must be positive.`);
    if (isNaN(config.releaseAmount) || config.releaseAmount <= 0)
        errors.push(`Release amount must be positive.`);
    if (
        isNaN(config.particleCount) ||
        config.particleCount <= 0 ||
        config.particleCount > 50000
    )
        errors.push(`Particle count must be between 1 and 50000.`);
    if (isNaN(config.spreadKm) || config.spreadKm < 0 || config.spreadKm > 50)
        errors.push(`Spread radius must be between 0 and 50 km.`);
    if (isNaN(config.releaseDuration) || config.releaseDuration < 0)
        errors.push(`Release duration must be positive`);
    if (isNaN(config.lon))
        errors.push(`Release location must have a longitude value`);
    if (isNaN(config.lat) || config.lat < -75 || config.lat > 85)
        errors.push(`Latitude must be between -75° and 85°`);
    return errors;
}

export async function simulationStep(version: number) {
    if (
        !simulation.simulationRunning ||
        version !== simulation.simulationVersion ||
        !simulation.proteus
    )
        return;

    try {
        const todayDateInt = simulation.proteus.get_current_date_int();
        if (simulation.stepCount != 0 && simulation.stepCount % config.stepsPerDay === 0) {
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

        await simulation.proteus?.step(simulation.stepCount);

        if (simulation.stepCount % (config.stepsPerDay / 24) === 0) {
            updateStats();
            captureSnapshot(Math.floor(simulation.proteus.current_day()));
        }

        updateBoundingBox();

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
        if (simulation.stepCount < config.totalDays * config.stepsPerDay) {
            simulation.animationId = requestAnimationFrame(() =>
                simulationStep(version),
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

    if (simulation.landmaskPromise) {
        await simulation.landmaskPromise;
    }

    const errors = validateSimulation();

    if (errors.length) {
        alert(`❌ Cannot start simulation:\n\n${errors.join("\n\n")}`);
        return;
    }

    simulation.simulationActive = true;
    simulation.simulationRunning = true;
    simulation.simulationVersion++;
    visualization.lastGridUpdate = 0;

    map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

    updateConcentrationLayer();
    zoom();

    if (visualization.currentMarker) visualization.currentMarker.remove();

    simulation.proteus = new Proteus(
        normalizeLongitude(config.lon),
        config.lat,
        config.csValue,
        config.particleCount,
        config.spreadKm,
        config.startDate,
        config.stepsPerDay,
        config.releaseAmount,
        config.releaseDuration,
        getOilData(),
    );

    simulationStep(simulation.simulationVersion);

    return errors;
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
    simulation.simulationActive = false;
    simulation.simulationRunning = false;
    simulation.simulationVersion++;
    simulation.stepCount = 0;
    history.simulationHistory = [];
    timeline.playbackMode = false;

    map.setPaintProperty("overlay-layer", "raster-opacity", 0.4);

    if (simulation.animationId) cancelAnimationFrame(simulation.animationId);

    simulation.proteus = new Proteus(
        normalizeLongitude(config.lon),
        config.lat,
        config.csValue,
        config.particleCount,
        config.spreadKm,
        config.startDate,
        config.stepsPerDay,
        config.releaseAmount,
        config.releaseDuration,
        getOilData(),
    );

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

    updateMarker();
    updateConcentrationLayer();
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
}

export function getStats() {
    return {
        stranded: stats.stranded,
        emulsified: stats.emulsified,
        evaporated: stats.evaporated,
        total_mass: stats.totalMass,
    };
}
