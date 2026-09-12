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