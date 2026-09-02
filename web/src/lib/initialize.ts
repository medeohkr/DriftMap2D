import { simulation, config } from "./stores.svelte";
import { initMap, updateMarker, normalizeLongitude } from "./map";
import { initGridLayer } from "./visualization";
import { getOilData, loadOilCatalog} from "./oils";
import init, { Proteus, setup_panic_hook } from "../pkg/proteus";

export async function initialize() {
    await init();
    setup_panic_hook();
    initMap();
    initGridLayer();
    loadOilCatalog();
    
    simulation.proteus = new Proteus(
        normalizeLongitude(config.lon),
        config.lat,
        config.csValue,
        config.particleCount,
        config.spreadKm,
        config.startDate,
        config.stepsPerDay,
        config.releaseAmount,
        config.releaseDuration,
        getOilData(),
    );

    updateMarker();
}
