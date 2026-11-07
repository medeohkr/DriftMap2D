import type { Stats } from "./stats.svelte";

export interface Snapshot {
    day: number;
    dateStr: string;
    activePositions: Float32Array;
    strandedPositions: Float32Array;
    heatmapGeojson: any;
    stats: Stats;
}

export interface History {
    simulationHistory: Snapshot[];
}

export const history: History = {
    simulationHistory: [],
};
