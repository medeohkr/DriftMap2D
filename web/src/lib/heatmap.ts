// heatmap.ts shares an immutable heatmap result between snapshot capture and live display

interface HeatmapSource {
    current_day(): number;
    generate_heatmap(cellSize: number, smoothing: number, levels: Float32Array): string;
}

export class HeatmapCache {
    private cached: {
        source: HeatmapSource;
        day: number;
        cellSize: number;
        smoothing: number;
        levels: number[];
        geojson: any;
    } | null = null;

    /** rebuild only when the engine advances or heatmap settings change */
    get(source: HeatmapSource, cellSize: number, smoothing: number, levels: readonly number[]) {
        const day = source.current_day();
        const cached = this.cached;
        if (cached && cached.source === source && cached.day === day &&
            cached.cellSize === cellSize && cached.smoothing === smoothing &&
            cached.levels.length === levels.length && cached.levels.every((v, i) => v === levels[i])) {
            return cached.geojson;
        }
        const geojson = JSON.parse(source.generate_heatmap(cellSize, smoothing, new Float32Array(levels)));
        this.cached = { source, day, cellSize, smoothing, levels: [...levels], geojson };
        return geojson;
    }

    /** release references when starting or resetting a simulation */
    clear() {
        this.cached = null;
    }

    peek(source: HeatmapSource) {
        return this.cached?.source === source ? this.cached.geojson : null;
    }
}
