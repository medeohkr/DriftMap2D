import { config, releaseConfig } from "../stores/index.svelte";
import type { ForcingModel } from "./forcing";

const defaultLocations = {
    global: { lat: 48.4, lon: -125.0 },
    norkyst: { lat: 60.0, lon: 4.5 },
};
const automaticLocations = new WeakMap<object, { lat: number; lon: number }>();

export function updateDefaultReleaseLocation(model: ForcingModel) {
    if (releaseConfig.releases.length !== 1) return;
    const release = releaseConfig.releases[0];
    const previous = automaticLocations.get(release) ?? defaultLocations.global;
    if (release.lat !== previous.lat || release.lon !== previous.lon) return;
    const next = defaultLocations[model];
    release.lat = next.lat;
    release.lon = next.lon;
    automaticLocations.set(release, next);
}

let releaseColors = new WeakMap<object, string>();
let nextReleaseColor = 0;

export function resetReleaseColors() {
    releaseColors = new WeakMap<object, string>();
    nextReleaseColor = 0;
    releaseConfig.releases.forEach(getReleaseColor);
}

export function getReleaseColor(release: object) {
    let color = releaseColors.get(release);
    if (!color) {
        const index = nextReleaseColor++;
        const hue = (220 + index * 30) % 360;
        color = `hsl(${hue} 50% 40%)`;
        releaseColors.set(release, color);
    }
    return color;
}

export function getTotalReleaseMass() {
    return releaseConfig.releases.reduce(
        (total, release) => total + release.schedule.reduce(
            (mass, interval) => mass + interval.amount,
            0,
        ),
        0,
    );
}

export function getReleasePositions() {
    let positions = [];
    for (let index = 0; index < releaseConfig.releases.length; index++) {
        positions.push(releaseConfig.releases[index].lon);
        positions.push(releaseConfig.releases[index].lat);
    }
    return positions;
}

export function getAverageReleasePosition() {
    const totals = releaseConfig.releases.reduce(
        (acc, release) => ({
            lon: acc.lon + release.lon,
            lat: acc.lat + release.lat,
        }),
        { lon: 0, lat: 0 }
    );
    
    return [
        totals.lon / releaseConfig.releases.length,
        totals.lat / releaseConfig.releases.length,
    ];
}

export function releasesToJson() {
    const releasesData = releaseConfig.releases.map(release => ({
        lon: release.lon,
        lat: release.lat,
        radius: release.radius,
        schedule: release.schedule.map(interval => ({
            amount: interval.amount,
            duration: config.tracerType == "sar" ? 0 : interval.duration,
        }))
    }));

    return JSON.stringify(releasesData);
}
