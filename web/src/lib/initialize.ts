import { initMap, updateMarker } from "./map";
import { initGridLayer } from "./visualization";
import { loadOilCatalog} from "./tracers/oils";
import { createProteus } from "./simulation";
import init, { setup_panic_hook } from "../pkg/proteus_wasm";

export async function initialize() {
    await init();
    setup_panic_hook();
    initMap();
    initGridLayer();
    loadOilCatalog();
    createProteus();
    updateMarker();
}
