import { config, releaseConfig } from "./stores.svelte";

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

export function getPositions() {
    let positions = [];
    for (let index = 0; index < releaseConfig.releases.length; index++) {
        positions.push(releaseConfig.releases[index].lon);
        positions.push(releaseConfig.releases[index].lat);
    }
    return positions;
}

export function getAveragePosition() {
    let totalLon = 0;
    let totalLat = 0;
    for (let index = 0; index < releaseConfig.releases.length; index++) {
        totalLon += releaseConfig.releases[index].lon;
        totalLat += releaseConfig.releases[index].lat;
    }
    let averageLon = totalLon / releaseConfig.releases.length;
    let averageLat = totalLon / releaseConfig.releases.length;
    return [averageLon, averageLat];
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
            amount: interval.amount,
            duration: interval.duration
        }))
    }));

    return JSON.stringify(releasesData);
}