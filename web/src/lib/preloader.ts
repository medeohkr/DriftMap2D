import { loadNorkystManifest, type ForcingModel } from "./utils/forcing";
import { stereographicGrid } from "./utils/stereographic";

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
    private nativeMetadata?: Promise<{ native: ReturnType<typeof stereographicGrid>; dates: Set<number> }>;
    private metadataLoadedAt = 0;
    private cache: TileCache;

    constructor() {
        this.pending = new Map();

        if (!window.__tileCache) {
            window.__tileCache = new Map();
        }
        this.cache = window.__tileCache;
    }

    getUrl(date: number, lonIdx: number, latIdx: number, forcingModel: ForcingModel): string {
        const year = Math.floor(date / 10000);
        const month = Math.floor((date % 10000) / 100);
        const day = date % 100;
        const monthStr = String(month).padStart(2, "0");
        const dayStr = String(day).padStart(2, "0");
        const lonStr = String(lonIdx).padStart(3, "0");
        const latStr = String(latIdx).padStart(3, "0");
        const baseUrl = `https://tiles.driftmap2d.com/${forcingModel === "norkyst" ? "norkyst" : "tiles"}`;
        return `${baseUrl}/${year}/${monthStr}/${dayStr}/${lonStr}_${latStr}.bin`;
    }

    preloadTile(date: number, lonIdx: number, latIdx: number, forcingModel: ForcingModel): void {
        const url = this.getUrl(date, lonIdx, latIdx, forcingModel);

        if (this.cache.has(url) || this.pending.has(url)) return;

        const promise = fetch(url)
            .then((response) => {
                if (!response.ok) throw new Error(`tile request failed (${response.status})`);
                return response.arrayBuffer();
            })
            .then((buffer) => {
                this.cache.set(url, new Uint8Array(buffer));
                this.pending.delete(url);
            })
            .catch((err) => {
                console.warn(`Preload failed: ${url}`, err);
                this.pending.delete(url);
            });

        this.pending.set(url, promise);
    }

    preloadTiles(date: number, tileIndices: TileIndex[], forcingModel: ForcingModel): void {
        for (const { lonIdx, latIdx } of tileIndices) {
            this.preloadTile(date, lonIdx, latIdx, forcingModel);
        }
    }

    private loadNativeMetadata() {
        // refresh availability periodically while sharing metadata between particle batches
        if (!this.nativeMetadata || Date.now() - this.metadataLoadedAt > 300000) {
            this.metadataLoadedAt = Date.now();
            this.nativeMetadata = loadNorkystManifest().then(manifest => {
                const native = stereographicGrid(manifest);
                if (!Number.isInteger(native.grid.core_cells) || native.grid.core_cells <= 0) {
                    throw new Error("NorKyst tile dimensions are invalid");
                }
                const days = (manifest as { days?: { date: string }[] }).days;
                if (!Array.isArray(days) || !days.length ||
                    days.some(day => !/^\d{4}-\d{2}-\d{2}$/.test(day.date))) {
                    throw new Error("NorKyst availability is missing or invalid");
                }
                return { native, dates: new Set(days.map(day => Number(day.date.replace(/-/g, "")))) };
            }).catch(error => {
                this.nativeMetadata = undefined;
                throw error;
            });
        }
        return this.nativeMetadata;
    }

    async getTileIndicesForOcean(
        positions: Float32Array,
        forcingModel: ForcingModel,
        edgeThreshold: number = 0.1,
    ): Promise<TileIndex[]> {
        if (forcingModel === "norkyst") {
            const { native } = await this.loadNativeMetadata();
            const { grid } = native;
            const tiles = new Map<string, TileIndex>();
            // 48 km margin for integration and diffusion probes
            const marginX = 48000 / grid.grid_spacing_m[0];
            const marginY = 48000 / grid.grid_spacing_m[1];
            for (let i = 0; i + 1 < positions.length; i += 2) {
                const position = native.position(positions[i], positions[i + 1]);
                if (!position) continue;
                const [x, y] = position;
                const left = Math.floor(Math.max(0, x - marginX) / grid.core_cells);
                const right = Math.floor(Math.min(grid.grid_shape[1] - 1, x + marginX) / grid.core_cells);
                const top = Math.floor(Math.max(0, y - marginY) / grid.core_cells);
                const bottom = Math.floor(Math.min(grid.grid_shape[0] - 1, y + marginY) / grid.core_cells);
                for (let lonIdx = left; lonIdx <= right; lonIdx++) {
                    for (let latIdx = top; latIdx <= bottom; latIdx++) {
                        tiles.set(`${lonIdx},${latIdx}`, { lonIdx, latIdx });
                    }
                }
            }
            return [...tiles.values()];
        }
        const tiles = new Set<string>();

        const addTile = (lonIdx: number, latIdx: number) => {
            const wrappedLonIdx = ((lonIdx % 36) + 36) % 36;
            if (wrappedLonIdx >= 0 && wrappedLonIdx < 36 && latIdx >= 0 && latIdx < 17) {
                tiles.add(`${wrappedLonIdx},${latIdx}`);
            }
        };

        for (let i = 0; i + 1 < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];

            if (!Number.isFinite(lon) || !Number.isFinite(lat) || lat < -80 || lat > 90) continue;

            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 80) / 10);

            addTile(centerLonIdx, centerLatIdx);

            const lonMod = ((lon % 10) + 10) % 10;
            const latMod = ((lat % 10) + 10) % 10;

            if (lonMod < edgeThreshold) {
                addTile(centerLonIdx - 1, centerLatIdx);
            } else if (lonMod > 10 - edgeThreshold) {
                addTile(centerLonIdx + 1, centerLatIdx);
            }

            if (latMod < edgeThreshold) {
                addTile(centerLonIdx, centerLatIdx - 1);
            } else if (latMod > 10 - edgeThreshold) {
                addTile(centerLonIdx, centerLatIdx + 1);
            }

            if ((lonMod < edgeThreshold || lonMod > 10 - edgeThreshold) &&
                (latMod < edgeThreshold || latMod > 10 - edgeThreshold)) {
                const lonDir = lonMod < edgeThreshold ? -1 : 1;
                const latDir = latMod < edgeThreshold ? -1 : 1;
                addTile(centerLonIdx + lonDir, centerLatIdx + latDir);
            }
        }

        return Array.from(tiles).map((key) => {
            const [lonIdx, latIdx] = key.split(',').map(Number);
            return { lonIdx, latIdx };
        });
    }
    
    async preloadOcean(
        currentDate: number,
        positions: Float32Array,
        forcingModel: ForcingModel,
        daysAhead = 0,
    ): Promise<void> {
        try {
            const tiles = await this.getTileIndicesForOcean(positions, forcingModel);
            const dates = forcingModel === "norkyst" ? (await this.loadNativeMetadata()).dates : null;
            for (let day = 0; day <= daysAhead; day++) {
                const date = this.addDays(currentDate, day);
                if (!dates || dates.has(date)) this.preloadTiles(date, tiles, forcingModel);
            }
        } catch (error) {
            console.warn("forcing preload failed", error);
        }
    }

    pruneBefore(date: number): void {
        for (const url of this.cache.keys()) {
            const match = url.match(/(\d{4})\/(\d{2})\/(\d{2})/);
            if (match && Number(match[1] + match[2] + match[3]) < date) this.cache.delete(url);
        }
    }

    addDays(dateInt: number, days: number): number {
        const year = Math.floor(dateInt / 10000);
        const month = Math.floor((dateInt % 10000) / 100);
        const day = dateInt % 100;
        const date = new Date(Date.UTC(year, month - 1, day + days));
        return (
            date.getUTCFullYear() * 10000 +
            (date.getUTCMonth() + 1) * 100 +
            date.getUTCDate()
        );
    }
}

window.getPreloadedTile = (url: string): Uint8Array | null => {
    if (window.__tileCache && window.__tileCache.has(url)) {
        return window.__tileCache.get(url)!;
    }
    return null;
};

export const preloader = new TilePreloader();
