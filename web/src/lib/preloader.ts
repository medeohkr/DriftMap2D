// src/lib/preloader.ts

interface TileIndex {
    lonIdx: number;
    latIdx: number;
}

type TileCache = Map<string, Uint8Array>;

declare global {
    interface Window {
        __tileCache?: TileCache;
        getPreloadedTile?: (url: string) => Uint8Array | null;
    }
}

export class TilePreloader {
    private pending: Map<string, Promise<void>>;
    private completed: Set<string>;
    private readonly baseUrl: string;
    private readonly landmaskUrl: string;
    private cache: TileCache;

    constructor() {
        this.pending = new Map();
        this.completed = new Set();
        this.baseUrl = "https://tiles.driftmap2d.com/tiles";
        this.landmaskUrl = "https://tiles.driftmap2d.com/roaring_landmask";

        // Initialize cache from window or create new
        if (!window.__tileCache) {
            window.__tileCache = new Map();
        }
        this.cache = window.__tileCache;
    }

    // ========== LANDMASK METHODS ==========

    preloadLandmask(lonIdx: number, latIdx: number): void {
        const url = `${this.landmaskUrl}/landmask_${String(lonIdx).padStart(3, "0")}_${String(latIdx).padStart(3, "0")}.bin`;

        if (this.completed.has(url) || this.pending.has(url)) return;

        const promise = fetch(url)
            .then((response) => response.arrayBuffer())
            .then((buffer) => {
                this.cache.set(url, new Uint8Array(buffer));
                this.completed.add(url);
                this.pending.delete(url);
            })
            .catch((err) => {
                console.warn(`Landmask preload failed: ${url}`, err);
                this.pending.delete(url);
            });

        this.pending.set(url, promise);
    }

    preloadLandmaskTiles(tileIndices: TileIndex[]): void {
        for (const { lonIdx, latIdx } of tileIndices) {
            this.preloadLandmask(lonIdx, latIdx);
        }
    }

    getTileIndicesForLandmask(
        positions: Float32Array,
        bufferTiles: number = 0,
    ): TileIndex[] {
        const tiles = new Set<string>();

        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];

            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 90) / 10);

            for (let dx = -bufferTiles; dx <= bufferTiles; dx++) {
                for (let dy = -bufferTiles; dy <= bufferTiles; dy++) {
                    const lonIdx = centerLonIdx + dx;
                    const latIdx = centerLatIdx + dy;
                    if (
                        lonIdx >= 0 &&
                        lonIdx < 36 &&
                        latIdx >= 0 &&
                        latIdx < 18
                    ) {
                        tiles.add(`${lonIdx},${latIdx}`);
                    }
                }
            }
        }

        return Array.from(tiles).map((key) => {
            const [lonIdx, latIdx] = key.split(",").map(Number);
            return { lonIdx, latIdx };
        });
    }

    // ========== OCEAN METHODS ==========

    getUrl(date: number, lonIdx: number, latIdx: number): string {
        const year = Math.floor(date / 10000);
        const month = Math.floor((date % 10000) / 100);
        const day = date % 100;
        const monthStr = String(month).padStart(2, "0");
        const dayStr = String(day).padStart(2, "0");
        const lonStr = String(lonIdx).padStart(3, "0");
        const latStr = String(latIdx).padStart(3, "0");
        return `${this.baseUrl}/${year}/${monthStr}/${dayStr}/${lonStr}_${latStr}.bin`;
    }

    preloadTile(date: number, lonIdx: number, latIdx: number): void {
        const url = this.getUrl(date, lonIdx, latIdx);

        if (this.completed.has(url) || this.pending.has(url)) return;

        const promise = fetch(url)
            .then((response) => response.arrayBuffer())
            .then((buffer) => {
                this.cache.set(url, new Uint8Array(buffer));
                this.completed.add(url);
                this.pending.delete(url);
            })
            .catch((err) => {
                console.warn(`Preload failed: ${url}`, err);
                this.pending.delete(url);
            });

        this.pending.set(url, promise);
    }

    preloadTiles(date: number, tileIndices: TileIndex[]): void {
        for (const { lonIdx, latIdx } of tileIndices) {
            this.preloadTile(date, lonIdx, latIdx);
        }
    }

    getTileIndicesForOcean(
        positions: Float32Array,
        bufferTiles: number = 0,
    ): TileIndex[] {
        const tiles = new Set<string>();

        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];

            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 80) / 10);

            for (let dx = -bufferTiles; dx <= bufferTiles; dx++) {
                for (let dy = -bufferTiles; dy <= bufferTiles; dy++) {
                    const lonIdx = centerLonIdx + dx;
                    const latIdx = centerLatIdx + dy;
                    if (
                        lonIdx >= 0 &&
                        lonIdx < 36 &&
                        latIdx >= 0 &&
                        latIdx < 17
                    ) {
                        tiles.add(`${lonIdx},${latIdx}`);
                    }
                }
            }
        }

        return Array.from(tiles).map((key) => {
            const [lonIdx, latIdx] = key.split(",").map(Number);
            return { lonIdx, latIdx };
        });
    }

    // ========== FUTURE PRELOADING ==========

    preloadFutureSteps(
        currentDate: number,
        currentPositions: Float32Array,
        stepsAhead: number = 3,
        bufferTiles: number = 1,
    ): void {
        for (let step = 1; step <= stepsAhead; step++) {
            const futureDate = this.addDays(currentDate, step);
            const futurePositions = this.predictPositions(
                currentPositions,
                step,
            );

            const futureOceanTiles = this.getTileIndicesForOcean(
                futurePositions,
                bufferTiles,
            );
            this.preloadTiles(futureDate, futureOceanTiles);

            const futureLandmaskTiles = this.getTileIndicesForLandmask(
                futurePositions,
                bufferTiles,
            );
            this.preloadLandmaskTiles(futureLandmaskTiles);
        }
    }

    predictPositions(
        positions: Float32Array,
        _stepsAhead: number,
    ): Float32Array {
        // Simple linear extrapolation based on current velocity
        // For now, just return current positions (conservative — preloads nearby tiles)
        return positions;
    }

    addDays(dateInt: number, days: number): number {
        const year = Math.floor(dateInt / 10000);
        const month = Math.floor((dateInt % 10000) / 100);
        const day = dateInt % 100;
        const date = new Date(year, month - 1, day);
        date.setDate(date.getDate() + days);
        return (
            date.getFullYear() * 10000 +
            (date.getMonth() + 1) * 100 +
            date.getDate()
        );
    }
}

// ── Expose to window ──
window.getPreloadedTile = (url: string): Uint8Array | null => {
    if (window.__tileCache && window.__tileCache.has(url)) {
        return window.__tileCache.get(url)!;
    }
    return null;
};

export const preloader = new TilePreloader();
