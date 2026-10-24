import { awaitLandmaskPreload } from "./map";
import { config, releaseConfig, simulation } from "./stores/index.svelte";
import { showToast } from "./stores/toast.svelte";
import { getReleasesOutsideDomain, loadForcingWindow, normalizeLongitude, utcInputTimestamp, validateForcingWindow } from "./utils";

function releaseMessage(releases: readonly { lon: number; lat: number }[], issue: string): string {
    if (releases.length !== 1) return `${releases.length} releases are ${issue}`;
    const release = releases[0];
    const lon = normalizeLongitude(release.lon);
    const latitude = `${Math.abs(release.lat).toFixed(1)}°${release.lat < 0 ? "S" : "N"}`;
    const longitude = `${Math.abs(lon).toFixed(1)}°${lon < 0 ? "W" : "E"}`;
    return `Release ${latitude} ${longitude} is ${issue}`;
}

export async function validateSimulationStart(): Promise<boolean> {
    const model = config.forcingModel;
    const version = simulation.simulationVersion;
    const [bounds, outside] = await Promise.all([
        loadForcingWindow(model),
        getReleasesOutsideDomain(model, releaseConfig.releases),
        awaitLandmaskPreload(),
    ]);
    if (simulation.simulationVersion !== version) return false;
    if (config.forcingModel !== model) {
        showToast("forcing selection changed, please start again");
        return false;
    }
    if (outside.length) {
        showToast(releaseMessage(outside, "outside grid domain"));
        return false;
    }
    if (!simulation.proteus) {
        showToast("Please wait... The simulation is loading");
        return false;
    }
    const onLand = releaseConfig.releases.filter(release =>
        simulation.proteus!.is_on_land(release.lon, release.lat));
    if (onLand.length) {
        showToast(releaseMessage(onLand, "on land"));
        return false;
    }
    validateForcingWindow(
        utcInputTimestamp(config.startDate, config.startTime),
        utcInputTimestamp(config.endDate, config.endTime),
        bounds,
    );
    return true;
}
