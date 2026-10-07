import { config } from '$lib/stores/config.svelte';
import { releaseConfig } from '$lib/stores/releases.svelte';
import { visualization } from '$lib/stores/visualization.svelte';
import { oilOverrides } from '$lib/stores/oil-overrides.svelte';
import { objectOverrides } from '$lib/stores/object-overrides.svelte';
import { genericOverrides } from '$lib/stores/generic-overrides.svelte';
import { history } from '$lib/stores/history.svelte';

export interface DriftMapConfig {
    // Tracer
    tracerType: string;
    
    // Simulation
    particleCount: number;
    timeStepMin: number;
    advectionScheme: string;
    diffusionScheme: string;
    diffusionCoeffs: number[];
    
    // Time
    startDate: string;
    startTime: string;
    endDate: string;
    endTime: string;
    totalDays: number;
    
    // Releases
    releases: Array<{
        lat: number;
        lon: number;
        radius: number;
        schedule: Array<{ amount: number; duration: number }>;
    }>;
    
    // Tracer-specific overrides
    oil: {
        id: string;
        windFactor: number;
        deflectionScheme: string;
        windDeflection: number | null;
        emulOnset: string;
        bullwinkleFrac: number;
        bulltime: number;
    };
    
    sar: {
        id: string;
        jibeProbability: number;
        capsizing: boolean;
        capsizeThreshold: number;
        capsizeFraction: number;
        capsizeSigma: number;
    };
    
    generic: {
        windFactor: number;
        windDeflection: number;
    };
    
    // Visualization
    visualization: {
        particleRadius: number;
        gridUpdateInterval: number;
        gridSize: number;
        smoothLevel: number;
        snapshotInterval: number;
        visualizationMode: string;
        autoZoom: boolean;
    };
}

export interface DriftMapGeoJson {
    type: "FeatureCollection";
    properties: {
        model: "DriftMap";
        version: string;
        date: string;
        includesSnapshots: boolean;
        config: DriftMapConfig;
    };
    features: Array<{
        type: "Feature";
        properties: {
            day: number;
            date: string;
            stats: any;
        };
        geometry: {
            type: "GeometryCollection";
            geometries: any[];
        };
    }>;
}

export function getCurrentConfig(): DriftMapConfig {
    return {
        tracerType: config.tracerType,
        
        particleCount: config.particleCount,
        timeStepMin: config.timeStepMin,
        advectionScheme: config.advectionScheme,
        diffusionScheme: config.diffusionScheme,
        diffusionCoeffs: [...config.diffusionCoeffs],
        
        startDate: config.startDate,
        startTime: config.startTime,
        endDate: config.endDate,
        endTime: config.endTime,
        totalDays: config.totalDays,
        
        releases: releaseConfig.releases.map(r => ({
            lat: r.lat,
            lon: r.lon,
            radius: r.radius,
            schedule: r.schedule.map(s => ({
                amount: s.amount,
                duration: s.duration,
            })),
        })),
        
        oil: {
            id: oilOverrides.id,
            windFactor: oilOverrides.windFactor,
            deflectionScheme: oilOverrides.deflectionScheme,
            windDeflection: oilOverrides.windDeflection,
            emulOnset: oilOverrides.emulOnset,
            bullwinkleFrac: oilOverrides.bullwinkleFrac,
            bulltime: oilOverrides.bulltime,
        },
        
        sar: {
            id: objectOverrides.id,
            jibeProbability: objectOverrides.jibeProbability,
            capsizing: objectOverrides.capsizing,
            capsizeThreshold: objectOverrides.capsizeThreshold,
            capsizeFraction: objectOverrides.capsizeFraction,
            capsizeSigma: objectOverrides.capsizeSigma,
        },
        
        generic: {
            windFactor: genericOverrides.windFactor,
            windDeflection: genericOverrides.windDeflection,
        },
        
        visualization: {
            particleRadius: visualization.particleRadius,
            gridUpdateInterval: visualization.gridUpdateInterval,
            gridSize: visualization.gridSize,
            smoothLevel: visualization.smoothLevel,
            snapshotInterval: visualization.snapshotInterval,
            visualizationMode: visualization.visualizationMode,
            autoZoom: visualization.autoZoom,
        },
    };
}

export function exportScenario(filename?: string): void {
    const config = getCurrentConfig();
    const hasSnapshots = history.simulationHistory.length > 0;
    const geojson: DriftMapGeoJson = {
        type: "FeatureCollection",
        properties: {
            model: "DriftMap",
            version: "0.1",
            date: new Date().toISOString(),
            includesSnapshots: hasSnapshots,
            config,
        },
        features: history.simulationHistory.map(snapshot => ({
            type: "Feature",
            properties: {
                day: snapshot.day,
                date: snapshot.dateStr,
                stats: snapshot.stats,
            },
            geometry: {
                type: "GeometryCollection",
                geometries: [
                    {
                        type: "MultiPoint",
                        coordinates: snapshot.unstrandedGeojson.features.map(
                            (f: any) => f.geometry.coordinates
                        ),
                    },
                    {
                        type: "MultiPoint",
                        coordinates: snapshot.strandedGeojson.features.map(
                            (f: any) => f.geometry.coordinates
                        ),
                    },
                    snapshot.heatmapGeojson.features
                        ? {
                            type: "GeometryCollection",
                            geometries: snapshot.heatmapGeojson.features.map(
                                (f: any) => f.geometry
                            ),
                        }
                        : null,
                ].filter(Boolean),
            },
        })),
    };
    
    const blob = new Blob([JSON.stringify(geojson)], {
        type: "application/geo+json",
    });
    
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = filename ?? `driftmap-${Date.now()}.geojson`;
    a.click();
    URL.revokeObjectURL(a.href);
}