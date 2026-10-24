import { stereographicGrid } from "./stereographic";

type Rows = [number, number][][];

function readRows(value: unknown, height: number, width: number): Rows {
    if (!Array.isArray(value) || value.length !== height) throw new Error("invalid NorKyst coverage rows");
    for (const row of value) {
        if (!Array.isArray(row)) throw new Error("invalid NorKyst coverage row");
        let previous = 0;
        for (const interval of row) {
            if (!Array.isArray(interval) || interval.length !== 2 ||
                !interval.every(Number.isInteger) || interval[0] < previous ||
                interval[0] >= interval[1] || interval[1] > width) {
                throw new Error("invalid NorKyst coverage interval");
            }
            previous = interval[1];
        }
    }
    return value as Rows;
}

function geometryKey(grid: ReturnType<typeof stereographicGrid>["grid"]): string {
    const p = grid.projection;
    return JSON.stringify([grid.grid_shape, grid.grid_origin_m, grid.grid_spacing_m,
        p.grid_mapping_name, p.latitude_of_projection_origin, p.straight_vertical_longitude_from_pole,
        p.false_easting, p.false_northing, p.semi_major_axis, p.semi_minor_axis, p.standard_parallel]);
}

export function norkystSupport(manifest: unknown, value: unknown) {
    const native = stereographicGrid(manifest);
    const data = value as { schema_version?: number; buffer_cells?: number; valid_rows?: unknown; unavailable_water_rows?: unknown };
    if (!data || data.schema_version !== 1 || data.buffer_cells !== 1 ||
        geometryKey(stereographicGrid(data).grid) !== geometryKey(native.grid)) {
        throw new Error("NorKyst coverage does not match the forcing grid");
    }
    const height = native.grid.grid_shape[0] - 1;
    const width = native.grid.grid_shape[1] - 1;
    const valid = readRows(data.valid_rows, height, width);
    const water = readRows(data.unavailable_water_rows, height, width);
    for (let row = 0; row < height; row++) {
        if (water[row].some(([left, right]) => valid[row].some(([a, b]) => left < b && right > a))) {
            throw new Error("NorKyst coverage overlaps unsupported water");
        }
    }
    return {
        contains(lon: number, lat: number): boolean {
            const point = native.position(lon, lat);
            if (!point) return false;
            const col = Math.min(Math.floor(point[0]), width - 1);
            const row = Math.min(Math.floor(point[1]), height - 1);
            return valid[row].some(([left, right]) => col >= left && col < right);
        },
        unavailableWater() {
            return waterPolygons(water, native.geographic);
        },
    };
}

function waterPolygons(rows: Rows, geographic: (col: number, row: number) => [number, number]) {
    // merge equal intervals vertically before converting cell edges to geographic polygons
    const rectangles: number[][] = [];
    let active = new Map<string, number[]>();
    rows.forEach((intervals, row) => {
        const next = new Map<string, number[]>();
        for (const [left, right] of intervals) {
            const key = `${left},${right}`;
            const rect = active.get(key) ?? [left, row, right, row + 1];
            rect[3] = row + 1;
            next.set(key, rect);
            active.delete(key);
        }
        rectangles.push(...active.values());
        active = next;
    });
    rectangles.push(...active.values());
    return {
        type: "FeatureCollection" as const,
        features: rectangles.map(([left, bottom, right, top]) => {
            const corners = [[left, bottom], [right, bottom], [right, top], [left, top], [left, bottom]];
            const ring: [number, number][] = [];
            for (let edge = 0; edge < 4; edge++) {
                const a = corners[edge], b = corners[edge + 1];
                const steps = Math.max(1, Math.ceil(Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])) / 32));
                for (let i = 0; i < steps; i++) {
                    ring.push(geographic(a[0] + (b[0] - a[0]) * i / steps, a[1] + (b[1] - a[1]) * i / steps));
                }
            }
            ring.push([...ring[0]]);
            return { type: "Feature" as const, properties: {}, geometry: { type: "Polygon" as const, coordinates: [ring] } };
        }),
    };
}
