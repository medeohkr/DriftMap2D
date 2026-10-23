type ReleasePosition = { lon: number; lat: number; radius: number };
type Bounds = [[number, number], [number, number]];

/** Frame releases in the shortest longitude arc, near the current world copy. */
export function getReleaseBounds(releases: ReleasePosition[], centerLon: number): Bounds | null {
    const valid = releases.filter(r => Number.isFinite(r.lon) && Number.isFinite(r.lat)
        && r.lat >= -90 && r.lat <= 90 && Number.isFinite(r.radius) && r.radius >= 0);
    if (!valid.length || !Number.isFinite(centerLon)) return null;

    const sorted = valid.map(r => ({ ...r, lon: ((r.lon % 360) + 360) % 360 }))
        .sort((a, b) => a.lon - b.lon);
    // Cut at the largest empty gap between release centers, not at +/-180.
    let largestGap = -1;
    let start = sorted[0].lon;
    for (let i = 0; i < sorted.length; i++) {
        const next = sorted[(i + 1) % sorted.length].lon;
        const gap = next + (i === sorted.length - 1 ? 360 : 0) - sorted[i].lon;
        if (gap > largestGap) {
            largestGap = gap;
            start = next;
        }
    }

    let west = Infinity, east = -Infinity, south = Infinity, north = -Infinity;
    for (const release of sorted) {
        const lon = release.lon < start ? release.lon + 360 : release.lon;
        // Release radius is in km. Frame three standard deviations of the
        // Gaussian release distribution used by Proteus.
        const angle = Math.min(Math.PI, 3 * release.radius / 6371);
        const latitude = release.lat * Math.PI / 180;
        const latitudeSpan = angle * 180 / Math.PI;
        const longitudeSpan = Math.abs(latitude) + angle >= Math.PI / 2 ? 180
            : Math.asin(Math.min(1, Math.sin(angle) / Math.cos(latitude))) * 180 / Math.PI;
        west = Math.min(west, lon - longitudeSpan);
        east = Math.max(east, lon + longitudeSpan);
        south = Math.min(south, Math.max(-75, release.lat - latitudeSpan));
        north = Math.max(north, Math.min(85, release.lat + latitudeSpan));
    }
    // Camera bounds follow the map's supported latitude range.
    south = Math.min(85, south);
    north = Math.max(-75, north);
    const middle = (west + east) / 2;
    if (east - west > 360) {
        west = middle - 180;
        east = middle + 180;
    }
    const shift = 360 * Math.round((centerLon - middle) / 360);
    return [[west + shift, south], [east + shift, north]];
}
