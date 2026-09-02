import { simulation, config } from "./stores.svelte";
import { initMap, updateMarker, normalizeLongitude } from "./map";
import { initGridLayer } from "./visualization";
import { loadOilCatalog} from "./oils";
import { createProteus } from "./simulation";
import init, { Proteus, setup_panic_hook } from "../pkg/proteus";

export async function initialize() {
    await init();
    setup_panic_hook();
    initMap();
    initGridLayer();
    loadOilCatalog();
    createProteus();
    updateMarker();
}
