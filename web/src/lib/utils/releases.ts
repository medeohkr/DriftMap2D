import { config, releaseConfig } from "../stores/index.svelte";

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
