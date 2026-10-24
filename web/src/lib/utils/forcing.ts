export type ForcingModel = "global" | "norkyst";
export interface ForcingWindow {
    start: number;
    end: number;
}

export function globalForcingWindow(now = new Date()): ForcingWindow {
    const midnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const day = 86400000;
    // ten generated days begin yesterday and include the following midnight
    return { start: midnight - 7 * day, end: midnight + 9 * day };
}

export function norkystForcingWindow(manifest: unknown): ForcingWindow {
    const days = (manifest as { days?: { timestamps?: unknown }[] } | null)?.days;
    if (!Array.isArray(days) || days.length === 0) {
        throw new Error("NorKyst availability is missing");
    }
    const intervals = days.map(day => {
        const timestamps = day?.timestamps;
        if (!Array.isArray(timestamps) || timestamps.length < 2 ||
            !timestamps.every(t => typeof t === "number" && Number.isSafeInteger(t)) ||
            timestamps.some((t, i) => i > 0 && t - timestamps[i - 1] !== 3600)) {
            throw new Error("NorKyst availability has invalid timestamps");
        }
        const start = timestamps[0] * 1000;
        const end = timestamps[timestamps.length - 1] * 1000;
        if (!Number.isFinite(new Date(start).getTime()) || !Number.isFinite(new Date(end).getTime())) {
            throw new Error("NorKyst availability has invalid dates");
        }
        return { start, end };
    }).sort((a, b) => a.start - b.start);
    for (let i = 1; i < intervals.length; i++) {
        if (intervals[i].start !== intervals[i - 1].end) {
            throw new Error("NorKyst availability has a gap between days");
        }
    }
    return { start: intervals[0].start, end: intervals[intervals.length - 1].end };
}

export async function loadForcingWindow(model: ForcingModel): Promise<ForcingWindow> {
    if (model === "global") return globalForcingWindow();
    return norkystForcingWindow(await loadNorkystManifest());
}

let manifestRequest: Promise<unknown> | null = null;

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
    return new Date(`${date}T${time}:00Z`).getTime();
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
