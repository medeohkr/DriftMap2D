import objectCatalog from "./assets/object_catalog.json"
import { objectOverrides } from "./stores/index.svelte";

export interface ObjectCatalog {
    objects: Record<string, SarObject>;
};

interface SarObject {
    downwind: number[];
    right: number[];
    left: number[];
}

const catalog = objectCatalog as ObjectCatalog;

export function searchObjects(query: string) {
    const q = query.toLowerCase();
    return Object.entries(catalog.objects)
        .filter(([name]) => name.toLowerCase().includes(q))
        .map(([name, data]) => ({name, ...data}))
}

export function getGenericObjects() {
    return Object.entries(catalog.objects)
        .filter(([name]) => name.includes("mean values"))
        .map(([name, data]) => ({ name, ...data }))
}

export function getObjectJsonForRust(objectName: string) {
    const object = catalog.objects[objectName];
    return JSON.stringify({
        downwind: objectOverrides.downwind ?? object.downwind,
        right: objectOverrides.right ?? object.right,
        left: objectOverrides.left ?? object.left,
        jibe_probabiliy: objectOverrides.jibeProbability,
        capsizing: objectOverrides.capsizing,
        capsize_threshold: objectOverrides.capsizeThreshold,
        capsize_fraction: objectOverrides.capsizeFraction,     
        capsize_sigma: objectOverrides.capsizeSigma,
    });
}