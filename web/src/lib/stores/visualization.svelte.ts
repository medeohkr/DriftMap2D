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
    mapLon: string | null;
    mapLat: string | null;
}

export const visualization: Visualization = $state({
    particleRadius: 1.5,
    gridUpdateInterval: 100,
    gridSize: 0.02,
    smoothLevel: 2,
    snapshotInterval: 2,

    heatmap: null,
    lastGridUpdate: 0,
    visualizationMode: "particles",
    boundingBox: new Float32Array(),
    currentMarker: null,
    autoZoom: true,

    mapLon: null,
    mapLat: null,
});