import { initMap, updateMarker } from "./map";
import { initGridLayer } from "./visualization";
import { loadOilCatalog} from "./oils";
import { createProteus } from "./simulation";
import init, { setup_panic_hook } from "../pkg/proteus";
import { releaseConfig } from "./stores.svelte";

export async function initialize() {
    await init();
    setup_panic_hook();
    initMap();
    initGridLayer();
    loadOilCatalog();
    updateMarker(releaseConfig.activeRelease.lon, releaseConfig.activeRelease.lat);
    createProteus();
}
