import { getObjectJson } from "../tracers/objects";
import { getOilJson } from "../tracers/oils";
import { config, genericOverrides } from "../stores/index.svelte";

export function getTracerJson() {
    if (config.tracerType === "oil") {
        return getOilJson();
    } else if (config.tracerType === "sar") {
        return getObjectJson();
    } else {
        return JSON.stringify({
            "wind_factor": genericOverrides.windFactor / 100,
            "wind_deflection": genericOverrides.windDeflection
        });
    }
}
