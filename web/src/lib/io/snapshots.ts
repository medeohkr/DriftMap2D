// snapshots.ts converts compact playback arrays to and from scenario GeoJSON

import type { Snapshot } from "../stores/history.svelte";

/** serialize particle positions only when the user exports a scenario */
export function positionsToCoordinates(positions: Float32Array): number[][] {
    return Array.from({ length: positions.length / 2 }, (_, i) => [positions[i * 2], positions[i * 2 + 1]]);
}

/** preserve the existing MultiPoint file format without retaining point objects in memory */
export function coordinatesToPositions(coordinates: number[][]): Float32Array {
    const positions = new Float32Array(coordinates.length * 2);
    for (let i = 0; i < coordinates.length; i++) {
        const point = coordinates[i];
        if (!Number.isFinite(point[0]) || !Number.isFinite(point[1])) {
            throw new Error("Invalid particle coordinates in scenario");
        }
        positions[i * 2] = point[0];
        positions[i * 2 + 1] = point[1];
    }
    return positions;
}

export function exportSnapshot(snapshot: Snapshot) {
    return {
        type: "Feature" as const,
        properties: {
            day: snapshot.day,
            date: snapshot.dateStr,
            stats: snapshot.stats,
            heatmap: snapshot.heatmapGeojson,
        },
        geometry: {
            type: "GeometryCollection" as const,
            geometries: [
                { type: "MultiPoint", coordinates: positionsToCoordinates(snapshot.activePositions) },
                { type: "MultiPoint", coordinates: positionsToCoordinates(snapshot.strandedPositions) },
            ],
        },
    };
}

export function importSnapshot(feature: ReturnType<typeof exportSnapshot>): Snapshot {
    return {
        day: feature.properties.day,
        dateStr: feature.properties.date,
        stats: feature.properties.stats,
        activePositions: coordinatesToPositions(feature.geometry.geometries[0].coordinates),
        strandedPositions: coordinatesToPositions(feature.geometry.geometries[1].coordinates),
        heatmapGeojson: feature.properties.heatmap ?? null,
    };
}
