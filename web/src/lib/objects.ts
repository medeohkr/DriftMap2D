import objectCatalog from "./assets/object_catalog.json"


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
    return JSON.stringify(catalog.objects[objectName]);
}