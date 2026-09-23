export interface Visualization {
    particleRadius: number;
    gridUpdateInterval: number;
    gridSize: number;
    smoothLevel: number;
    heatmap: any | null;
    lastGridUpdate: number;
    visualizationMode: "particles" | "heatmap";
    boundingBox: Float32Array;
    currentMarker: any;
    mapLon: string | null;
    mapLat: string | null;
}

export const visualization: Visualization = $state({
    particleRadius: 1.5,
    gridUpdateInterval: 150,
    gridSize: 0.02,
    smoothLevel: 2,

    heatmap: null,
    lastGridUpdate: 0,
    visualizationMode: "particles",
    boundingBox: new Float32Array(),
    currentMarker: null,

    mapLon: null,
    mapLat: null,
});