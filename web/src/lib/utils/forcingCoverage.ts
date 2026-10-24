import { loadNorkystManifest, type ForcingModel } from "./forcing";

interface NativeGrid {
    grid_shape: [number, number];
    grid_origin_m: [number, number];
    grid_spacing_m: [number, number];
    projection: {
        grid_mapping_name: string;
        latitude_of_projection_origin: number;
        straight_vertical_longitude_from_pole: number;
        false_easting: number;
        false_northing: number;
        semi_major_axis: number;
        semi_minor_axis: number;
        standard_parallel: number;
    };
}

export function norkystCoverage(manifest: unknown) {
    const grid = (manifest as { grid?: NativeGrid } | null)?.grid;
    if (!grid) throw new Error("NorKyst grid metadata is missing");
    const p = grid.projection;
    if (!p || p.grid_mapping_name !== "polar_stereographic" || p.latitude_of_projection_origin !== 90 ||
        ![p.straight_vertical_longitude_from_pole, p.false_easting, p.false_northing,
            p.semi_major_axis, p.semi_minor_axis, p.standard_parallel].every(Number.isFinite) ||
        p.semi_minor_axis <= 0 || p.semi_minor_axis > p.semi_major_axis ||
        p.standard_parallel <= 0 || p.standard_parallel >= 90 ||
        !Array.isArray(grid.grid_shape) || grid.grid_shape.length !== 2 ||
        !grid.grid_shape.every(v => Number.isInteger(v) && v >= 2) ||
        !Array.isArray(grid.grid_origin_m) || grid.grid_origin_m.length !== 2 ||
        !grid.grid_origin_m.every(Number.isFinite) ||
        !Array.isArray(grid.grid_spacing_m) || grid.grid_spacing_m.length !== 2 ||
        !grid.grid_spacing_m.every(v => Number.isFinite(v) && v > 0)) {
        throw new Error("NorKyst coverage metadata is invalid");
    }
    const e = Math.sqrt(1 - (p.semi_minor_axis / p.semi_major_axis) ** 2);
    const phiC = p.standard_parallel * Math.PI / 180;
    const mC = Math.cos(phiC) / Math.sqrt(1 - e * e * Math.sin(phiC) ** 2);
    const tC = Math.tan(Math.PI / 4 - phiC / 2) /
        ((1 - e * Math.sin(phiC)) / (1 + e * Math.sin(phiC))) ** (e / 2);

    const geographic = (col: number, row: number): [number, number] => {
        const x = grid.grid_origin_m[0] + col * grid.grid_spacing_m[0] - p.false_easting;
        const y = grid.grid_origin_m[1] + row * grid.grid_spacing_m[1] - p.false_northing;
        const t = Math.hypot(x, y) * tC / (p.semi_major_axis * mC);
        let phi = Math.PI / 2 - 2 * Math.atan(t);
        for (let i = 0; i < 12; i++) {
            const ratio = (1 - e * Math.sin(phi)) / (1 + e * Math.sin(phi));
            phi = Math.PI / 2 - 2 * Math.atan(t * ratio ** (e / 2));
        }
        return [p.straight_vertical_longitude_from_pole + Math.atan2(x, -y) * 180 / Math.PI,
            phi * 180 / Math.PI];
    };

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
            // match the float32 positions passed to the engine
            lon = Math.fround(lon);
            lat = Math.fround(lat);
            if (!Number.isFinite(lon) || !Number.isFinite(lat) || lat <= -90 || lat > 90) return false;
            const phi = lat * Math.PI / 180;
            const t = Math.tan(Math.PI / 4 - phi / 2) /
                ((1 - e * Math.sin(phi)) / (1 + e * Math.sin(phi))) ** (e / 2);
            const rho = p.semi_major_axis * mC * t / tC;
            const angle = (lon - p.straight_vertical_longitude_from_pole) * Math.PI / 180;
            const col = (p.false_easting + rho * Math.sin(angle) - grid.grid_origin_m[0]) / grid.grid_spacing_m[0];
            const row = (p.false_northing - rho * Math.cos(angle) - grid.grid_origin_m[1]) / grid.grid_spacing_m[1];
            return col >= 0 && col <= width && row >= 0 && row <= height;
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
