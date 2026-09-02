import type { Proteus } from "../pkg/proteus";

export interface Simulation {
    proteus: Proteus | null;
    simulationActive: boolean;
    simulationRunning: boolean;
    simulationVersion: number;
    animationId: number | null;
    landmaskPromise: Promise<void> | null;
    stepCount: number;
    currentTime: string;
}

export interface Config {
    lon: number;
    lat: number;
    csValue: number;
    particleCount: number;
    spreadKm: number;
    stepsPerDay: number;
    startDate: string;
    totalDays: number;
    releaseAmount: number;
    releaseDuration: number;
    tracerType: string;
    oilName: string;
    oilJson: string;
    autoZoom: boolean;
}

export interface Visualization {
    gridUpdateInterval: number;
    gridSize: number;
    concentrations: number[];
    heatmap: any | null;
    lastGridUpdate: number;
    visualizationMode: "particles" | "heatmap";
    boundingBox: Float32Array;
    currentMarker: any;
    mapLon: string | null;
    mapLat: string | null;
}

export interface Timeline {
    timelineDay: number;
    timelineDate: string;
    timelinePlaying: boolean;
    timelineAnimationId: ReturnType<typeof setTimeout> | null;
    playbackSpeed: number;
    playbackMode: boolean;
}

export interface Stats {
    stranded: string;
    emulsified: string;
    evaporated: string;
    totalMass: string;
}

export interface History {
    simulationHistory: any[];
}

export interface OilOverrides {
    api: string | null;
    bullwinkleFrac: string | null;
}

export const simulation: Simulation = $state({
    proteus: null,
    simulationActive: false,
    simulationRunning: false,
    simulationVersion: 0,
    animationId: null,
    landmaskPromise: null,
    stepCount: 0,
    currentTime: "",
});

export const config: Config = $state({
    lon: 56.5,
    lat: 26.6,
    csValue: 0.1,
    particleCount: 10000,
    stepsPerDay: 96,

    startDate: new Date().toISOString().split("T")[0],
    totalDays: 7.0,

    spreadKm: 1.0,
    releaseAmount: 1000.0,
    releaseDuration: 1.0,

    tracerType: "oil",
    oilName: "arabian-light",
    oilJson: "",

    autoZoom: true,
});

export const visualization: Visualization = $state({
    gridUpdateInterval: 100,
    gridSize: 0.025,
    concentrations: [
        0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2,
    ],

    heatmap: null,
    lastGridUpdate: 0,
    visualizationMode: "particles",
    boundingBox: new Float32Array(),
    currentMarker: null,

    mapLon: null,
    mapLat: null,
});

export const timeline: Timeline = $state({
    timelineDay: 0,
    timelineDate: "",
    timelinePlaying: false,
    timelineAnimationId: null,
    playbackSpeed: 100,
    playbackMode: false,
});

export const stats: Stats = $state({
    stranded: "",
    emulsified: "",
    evaporated: "",
    totalMass: "",
});

export const history: History = {
    simulationHistory: []
}

export const oilOverrides: OilOverrides = {
    api: null,
    bullwinkleFrac: null,
}