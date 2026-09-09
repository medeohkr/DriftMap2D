import type { Proteus } from "../pkg/proteus";
import { getOilById } from "./oils";
import { dateOffset, getTotalDays } from "./utils";

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
    csValue: number;
    particleCount: number;
    stepsPerDay: number;
    startDate: string;
    startTime: string;
    endDate: string;
    endTime: string;
    totalDays: number;
    tracerType: string;
    tracerJson: string;
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
    maxWaterFrac: string | null;
}

export interface SidebarState {
    collapseStage: number;
}

export interface Release {
    lat: number,
    lon: number,
    radius: number,
    schedule: Schedule[]
}

export interface Schedule {
    amount: number,
    duration: number
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
    csValue: 0.05,
    particleCount: 10000,
    stepsPerDay: 96,

    startDate: dateOffset(0),
    startTime: "00:00",
    endDate: dateOffset(7),
    endTime: "00:00",
    totalDays: 7,

    tracerType: "generic",
    tracerJson: "",

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
    playbackSpeed: 60,
    playbackMode: false,
});

export const stats: Stats = $state({
    stranded: "",
    emulsified: "",
    evaporated: "",
    totalMass: "",
});

export const history: History = {
    simulationHistory: [],
};

export const oilOverrides: OilOverrides = {
    api: null,
    bullwinkleFrac: null,
    maxWaterFrac: null,
};

export const sidebarState: SidebarState = $state({
    collapseStage: 0,
});

export const releaseConfig = $state({
    releases: [
        {
            lat: 26.58,
            lon: 56.25,
            radius: 2.5,
            schedule: [
                {
                    amount: 100,
                    duration: 12,
                },
            ],
        },
    ],
    activeReleaseIndex: 0,

    get activeRelease() {
        return this.releases[this.activeReleaseIndex];
    },

    addRelease(release: any) {
        releaseConfig.releases.push({
            lat: releaseConfig.activeRelease.lat + 0.02,
            lon: releaseConfig.activeRelease.lon + 0.02,
            radius: releaseConfig.activeRelease.radius,
            schedule: [
                {
                    amount: 100,
                    duration: 12,
                }
            ],
            ...release,
        });
        releaseConfig.activeReleaseIndex = releaseConfig.releases.length - 1;
    },

    removeRelease(index: number) {
        if (releaseConfig.releases.length <= 1) return;
        if (index === releaseConfig.activeReleaseIndex) {
            releaseConfig.activeReleaseIndex = Math.max(0, index - 1);
        } else if (index < releaseConfig.activeReleaseIndex) {
            releaseConfig.activeReleaseIndex--;
        }
        releaseConfig.releases.splice(index, 1);

    },

    addInterval(interval: any) {
        releaseConfig.activeRelease.schedule.push({
            amount: 100,
            duration: 12,
            ...interval,
        });
    },

    removeInterval(index: number) {
        releaseConfig.activeRelease.schedule.splice(index, 1);
    },
});
