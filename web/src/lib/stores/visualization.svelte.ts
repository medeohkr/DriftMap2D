import { simulation } from "./simulation.svelte";

export interface Visualization {
    particleRadius: number;
    gridUpdateInterval: number;
    gridSize: number;
    smoothLevel: number;
    snapshotInterval: number;
    heatmap: any | null;
    lastGridUpdate: number;
    visualizationMode: "particles" | "heatmap";
    boundingBox: Float32Array;
    currentMarker: any;
    autoZoom: boolean;
}

export const visualization: Visualization = $state({
    particleRadius: 1.5,
    gridUpdateInterval: 10,
    gridSize: 0.01,
    smoothLevel: 2,
    snapshotInterval: 2,

    heatmap: null,
    lastGridUpdate: 0,
    visualizationMode: "particles",
    boundingBox: new Float32Array(),
    currentMarker: null,
    autoZoom: true,

});
