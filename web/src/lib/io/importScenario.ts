import { config } from '$lib/stores/config.svelte';
import { releaseConfig } from '$lib/stores/releases.svelte';
import { visualization } from '$lib/stores/visualization.svelte';
import { oilOverrides } from '$lib/stores/oil-overrides.svelte';
import { objectOverrides } from '$lib/stores/object-overrides.svelte';
import { genericOverrides } from '$lib/stores/generic-overrides.svelte';
import { history } from '$lib/stores/history.svelte';
import type { DriftMapGeoJson, DriftMapConfig } from './exportScenario';
import { showToast } from '$lib/stores/toast.svelte';

export function applyConfig(cfg: DriftMapConfig): void {
    // Tracer
    config.tracerType = cfg.tracerType;
    
    // Simulation
    config.particleCount = cfg.particleCount;
    config.timeStepMin = cfg.timeStepMin;
    config.advectionScheme = cfg.advectionScheme;
    config.diffusionScheme = cfg.diffusionScheme;
    config.diffusionCoeffs = [...cfg.diffusionCoeffs];
    
    // Time
    config.startDate = cfg.startDate;
    config.startTime = cfg.startTime;
    config.endDate = cfg.endDate;
    config.endTime = cfg.endTime;
    config.totalDays = cfg.totalDays;
    
    // Releases
    releaseConfig.releases = cfg.releases.map(r => ({
        lat: r.lat,
        lon: r.lon,
        radius: r.radius,
        schedule: r.schedule.map(s => ({
            amount: s.amount,
            duration: s.duration,
        })),
    }));
    releaseConfig.activeReleaseIndex = 0;
    
    // Oil
    oilOverrides.id = cfg.oil.id;
    oilOverrides.query = cfg.oil.id;
    oilOverrides.windFactor = cfg.oil.windFactor;
    oilOverrides.deflectionScheme = cfg.oil.deflectionScheme as "samuels" | "constant";
    oilOverrides.windDeflection = cfg.oil.windDeflection;
    oilOverrides.emulOnset = cfg.oil.emulOnset as "time" | "fraction";
    oilOverrides.bullwinkleFrac = cfg.oil.bullwinkleFrac;
    oilOverrides.bulltime = cfg.oil.bulltime;
    
    // SAR
    objectOverrides.id = cfg.sar.id;
    objectOverrides.query = cfg.sar.id;
    objectOverrides.jibeProbability = cfg.sar.jibeProbability;
    objectOverrides.capsizing = cfg.sar.capsizing;
    objectOverrides.capsizeThreshold = cfg.sar.capsizeThreshold;
    objectOverrides.capsizeFraction = cfg.sar.capsizeFraction;
    objectOverrides.capsizeSigma = cfg.sar.capsizeSigma;
    
    // Generic
    genericOverrides.windFactor = cfg.generic.windFactor;
    genericOverrides.windDeflection = cfg.generic.windDeflection;
    
    // Visualization
    visualization.particleRadius = cfg.visualization.particleRadius;
    visualization.gridUpdateInterval = cfg.visualization.gridUpdateInterval;
    visualization.gridSize = cfg.visualization.gridSize;
    visualization.smoothLevel = cfg.visualization.smoothLevel;
    visualization.snapshotInterval = cfg.visualization.snapshotInterval;
    visualization.visualizationMode = cfg.visualization.visualizationMode as "particles" | "heatmap";
    visualization.autoZoom = cfg.visualization.autoZoom;
}

export function loadSnapshots(geojson: DriftMapGeoJson): void {
    history.simulationHistory = geojson.features.map((feature: any) => {
        const geometries = feature.geometry.geometries;
        const hasHeatmaps = geojson.properties.includesSnapshots;
        
        const snapshot: any = {
            day: feature.properties.day,
            dateStr: feature.properties.date,
            stats: feature.properties.stats,
            unstrandedGeojson: {
                type: "FeatureCollection",
                features: geometries[0].coordinates.map((coord: number[]) => ({
                    type: "Feature",
                    geometry: { type: "Point", coordinates: coord },
                })),
            },
            strandedGeojson: {
                type: "FeatureCollection",
                features: geometries[1].coordinates.map((coord: number[]) => ({
                    type: "Feature",
                    geometry: { type: "Point", coordinates: coord },
                })),
            },
            heatmapGeojson: null,
        };
        
        if (hasHeatmaps && geometries[2]?.geometries) {
            snapshot.heatmapGeojson = {
                type: "FeatureCollection",
                features: geometries[2].geometries.map((geom: any) => ({
                    type: "Feature",
                    geometry: geom,
                    properties: { concentration: 1 },
                })),
            };
        }
        
        return snapshot;
    });
}

export async function importScenario(file: File): Promise<void> {
    try {
        const text = await file.text();
        const data: DriftMapGeoJson = JSON.parse(text);
        
        if (data.properties?.model !== "DriftMap") {
            showToast("Not a DriftMap scenario file");
            return;
        }
        
        // Apply config
        applyConfig(data.properties.config);
        
        // Load snapshots if present
        if (data.properties.includesSnapshots && data.features.length > 0) {
            loadSnapshots(data);
            showToast(`Loaded scenario with ${data.features.length} snapshots`);
        } else {
            showToast("Loaded scenario config");
        }
    } catch (e) {
        console.error("Import failed:", e);
        showToast("Failed to import scenario");
    }
}