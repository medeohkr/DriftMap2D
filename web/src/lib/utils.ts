import { simulation, config, releaseConfig } from "./stores/index.svelte";

export function dateOffset(days: number) {
    const date = new Date();
    date.setDate(date.getDate() + days);
    return date.toISOString().split("T")[0];
}

export function getTotalDays(): number {
    const startDateTime = `${config.startDate}T${config.startTime}`
    const endDateTime = `${config.endDate}T${config.endTime}`
    const start = new Date(startDateTime).getTime();
    const end = new Date(endDateTime).getTime();
    return (end - start) / (1000 * 60 * 60 * 24);
}

export function startDateTime() {
    return `${config.startDate} ${config.startTime}`
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

export function getAveragePosition(): [number, number] | undefined {
    if (!simulation.proteus) return;

    const positions = simulation.proteus.get_positions();
    const n = positions.length / 2;
    if (n === 0) return;

    let sumLon = 0;
    let sumLat = 0;
    for (let i = 0; i < positions.length; i += 2) {
        sumLon += positions[i];
        sumLat += positions[i + 1];
    }

    return [sumLon / n, sumLat / n];
}

export function normalizeLongitude(lon: number) {
    return ((((lon + 180) % 360) + 360) % 360) - 180;
}

export function releasesToJson() {
    const releasesData = releaseConfig.releases.map(release => ({
        lon: release.lon,
        lat: release.lat,
        radius: release.radius,
        schedule: release.schedule.map(interval => ({
            amount: config.tracerType == "sar" ? 1 : interval.amount,
            duration: config.tracerType == "sar" ? 0 : interval.duration,
        }))
    }));

    return JSON.stringify(releasesData);
}