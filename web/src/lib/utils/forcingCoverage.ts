import { loadNorkystManifest, type ForcingModel } from "./forcing";

import { stereographicGrid } from "./stereographic";

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

export async function getReleasesOutsideDomain<T extends { lon: number; lat: number }>(
    model: ForcingModel,
    releases: readonly T[],
): Promise<T[]> {
    const contains = model === "norkyst"
        ? (await loadNorkystCoverage()).contains
        : (lon: number, lat: number) => Number.isFinite(lon) && Number.isFinite(lat) && lat >= -80 && lat <= 90;
    return releases.filter(release => !contains(release.lon, release.lat));
}
