export type ForcingModel = "global" | "norkyst";
export const DEFAULT_SIMULATION_DAYS = 2;
export interface ForcingWindow {
    start: number;
    end: number;
}

export function globalForcingWindow(now = new Date()): ForcingWindow {
    const midnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const day = 86400000;
    // compatibility bounds until the global cron publishes its first manifest
    return { start: midnight - 7 * day, end: midnight + 9 * day };
}

export function norkystForcingWindow(manifest: unknown): ForcingWindow {
    return manifestForcingWindow(manifest, "NorKyst");
}

function manifestForcingWindow(manifest: unknown, label: string): ForcingWindow {
    const days = (manifest as { days?: { timestamps?: unknown }[] } | null)?.days;
    if (!Array.isArray(days) || days.length === 0) {
        throw new Error(`${label} availability is missing`);
    }
    const intervals = days.map(day => {
        const timestamps = day?.timestamps;
        if (!Array.isArray(timestamps) || timestamps.length < 2 ||
            !timestamps.every(t => typeof t === "number" && Number.isSafeInteger(t)) ||
            timestamps.some((t, i) => i > 0 && t - timestamps[i - 1] !== 3600)) {
            throw new Error(`${label} availability has invalid timestamps`);
        }
        const start = timestamps[0] * 1000;
        const end = timestamps[timestamps.length - 1] * 1000;
        if (!Number.isFinite(new Date(start).getTime()) || !Number.isFinite(new Date(end).getTime())) {
            throw new Error(`${label} availability has invalid dates`);
        }
        return { start, end };
    }).sort((a, b) => a.start - b.start);
    for (let i = 1; i < intervals.length; i++) {
        if (intervals[i].start !== intervals[i - 1].end) {
            throw new Error(`${label} availability has a gap between days`);
        }
    }
    return { start: intervals[0].start, end: intervals[intervals.length - 1].end };
}

export async function loadForcingWindow(model: ForcingModel): Promise<ForcingWindow> {
    if (model === "global") {
        const manifest = await loadGlobalManifest();
        // older tile deployments do not publish a manifest
        if (manifest === null) return globalForcingWindow();
        const published = manifestForcingWindow(manifest, "global");
        return { start: globalForcingWindow().start, end: published.end };
    }
    return norkystForcingWindow(await loadNorkystManifest());
}

export function defaultSimulationWindow(window: ForcingWindow, now = new Date()): ForcingWindow {
    const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const first = window.start;
    const last = window.end;
    if (last - first < 60000) throw new Error("forcing has no available simulation interval");
    const start = Math.min(Math.max(today, first), last - 60000);
    return { start, end: Math.min(start + DEFAULT_SIMULATION_DAYS * 86400000, last) };
}

let manifestRequest: Promise<unknown> | null = null;
let globalManifestRequest: Promise<unknown> | null = null;

function loadGlobalManifest(): Promise<unknown> {
    if (!globalManifestRequest) {
        globalManifestRequest = fetch("https://tiles.driftmap2d.com/tiles/manifest.json", { cache: "no-cache" })
            .then(response => {
                if (response.status === 404) return null;
                if (!response.ok) throw new Error(`global metadata could not be loaded (${response.status})`);
                return response.json();
            }).finally(() => { globalManifestRequest = null; });
    }
    return globalManifestRequest;
}

export function loadNorkystManifest(): Promise<unknown> {
    if (!manifestRequest) {
        manifestRequest = fetch("https://tiles.driftmap2d.com/norkyst/manifest.json", { cache: "no-cache" })
            .then(response => {
                if (!response.ok) throw new Error(`NorKyst metadata could not be loaded (${response.status})`);
                return response.json();
            }).finally(() => { manifestRequest = null; });
    }
    return manifestRequest;
}

export function utcInputTimestamp(date: string, time: string): number {
    return new Date(`${date}T${time}Z`).getTime();
}

export function validateForcingWindow(start: number, end: number, window: ForcingWindow): void {
    if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) {
        throw new Error("choose an end time after the start time");
    }
    if (start < window.start || end > window.end) {
        const describe = (time: number) => new Date(time).toISOString().slice(0, 16).replace("T", " ");
        throw new Error(`forcing is available from ${describe(window.start)} to ${describe(window.end)} UTC`);
    }
}
