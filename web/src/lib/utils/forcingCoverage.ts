import { loadNorkystManifest, type ForcingModel } from "./forcing";

import { stereographicGrid } from "./stereographic";
import { norkystSupport } from "./forcingMask";

export function norkystCoverage(manifest: unknown) {
    const { grid, geographic, position } = stereographicGrid(manifest);
    const width = grid.grid_shape[1] - 1;
    const height = grid.grid_shape[0] - 1;
    const corners = [[0, 0], [width, 0], [width, height], [0, height], [0, 0]];
    const ring: [number, number][] = [];
    // sample the native edges to retain their curvature on the geographic map
    for (let edge = 0; edge < 4; edge++) {
        for (let i = 0; i < 32; i++) {
            const fraction = i / 32;
            ring.push(geographic(
                corners[edge][0] + fraction * (corners[edge + 1][0] - corners[edge][0]),
                corners[edge][1] + fraction * (corners[edge + 1][1] - corners[edge][1]),
            ));
        }
    }
    ring.push([...ring[0]]);
    const bounds: [[number, number], [number, number]] = [
        [Math.min(...ring.map(p => p[0])), Math.min(...ring.map(p => p[1]))],
        [Math.max(...ring.map(p => p[0])), Math.max(...ring.map(p => p[1]))],
    ];
    return {
        bounds,
        contains(lon: number, lat: number): boolean {
            return position(lon, lat) !== null;
        },
        feature: {
            type: "Feature" as const,
            properties: {},
            geometry: { type: "Polygon" as const, coordinates: [ring] },
        },
    };
}

export async function loadNorkystCoverage() {
    return norkystCoverage(await loadNorkystManifest());
}

let maskRequest: { url: string; promise: Promise<unknown> } | null = null;

export async function loadNorkystCoverageMask() {
    const manifest = await loadNorkystManifest();
    const descriptor = (manifest as { coverage?: { path?: string; sha256?: string } }).coverage;
    if (!descriptor?.path || !/^coverage-[0-9a-f]{16}\.json$/.test(descriptor.path) ||
        !descriptor.sha256 || !/^[0-9a-f]{64}$/.test(descriptor.sha256) ||
        descriptor.path !== `coverage-${descriptor.sha256.slice(0, 16)}.json`) {
        throw new Error("NorKyst coverage is unavailable. Please wait for the forcing update");
    }
    const url = `https://tiles.driftmap2d.com/norkyst/${descriptor.path}`;
    if (maskRequest?.url !== url) {
        const promise = fetch(url).then(async response => {
            if (!response.ok) throw new Error(`NorKyst coverage could not be loaded (${response.status})`);
            const bytes = await response.arrayBuffer();
            const digest = await crypto.subtle.digest("SHA-256", bytes);
            const hash = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
            if (hash !== descriptor.sha256) throw new Error("NorKyst coverage checksum does not match");
            return JSON.parse(new TextDecoder().decode(bytes));
        }).catch(error => {
            if (maskRequest?.url === url) maskRequest = null;
            throw error;
        });
        maskRequest = { url, promise };
    }
    return { ...norkystSupport(manifest, await maskRequest.promise), revision: descriptor.sha256 };
}

export async function getReleasesOutsideDomain<T extends { lon: number; lat: number }>(
    model: ForcingModel,
    releases: readonly T[],
): Promise<T[]> {
    const contains = model === "norkyst"
        ? (await loadNorkystCoverageMask()).contains
        : (lon: number, lat: number) => Number.isFinite(lon) && Number.isFinite(lat) && lat >= -80 && lat <= 90;
    return releases.filter(release => !contains(release.lon, release.lat));
}
