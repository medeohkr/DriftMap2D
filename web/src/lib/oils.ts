import { oilOverrides } from "./stores.svelte";

export interface OilRecord {
    oil_id: string;
    name: string;
    product_type: string | null;
    api_gravity: number;
    density_kgm3: [number, number][];
    dynamic_viscosity_cp: [number, number][];
    interfacial_tension_n_m: [number, number][];
    sara_mass_fractions: {
        saturates: number;
        aromatics: number;
        resins: number;
        asphaltenes: number;
    };
    distillation_cuts: {
        cumulative_fraction: number;
        vapor_temperature_c: number;
    }[];
    boiling_points_c: number[];
    molecular_weights_kg_mol: number[];
    component_mass_fractions: number[];
    bullwinkle_fraction: number;
    estimated: {
        density: boolean;
        dynamic_viscosity: boolean;
        interfacial_tension: boolean;
        sara: {
            saturates: boolean;
            aromatics: boolean;
            resins: boolean;
            asphaltenes: boolean;
        };
        distillation: boolean;
        bullwinkle_fraction: boolean;
    };
    source_file: string;
}

interface OilCatalog {
    schema_version: number;
    source: string;
    units: {
        density: string;
        temperature: string;
        dynamic_viscosity: string;
        interfacial_tension: string;
        molecular_weight: string;
        mass_fraction: string;
    };
    oils: OilRecord[];
    statistics: {
        oil_count: number;
        skipped_count: number;
    };
}

let catalog: OilCatalog | null = null;
let catalogPromise: Promise<OilCatalog> | null = null;

export async function loadOilCatalog(): Promise<OilCatalog> {
    if (catalog) return catalog;
    if (catalogPromise) return catalogPromise;

    catalogPromise = (async (): Promise<OilCatalog> => {
        try {
            const url = 'https://tiles.driftmap2d.com/oil_catalog.json.gz';
            const response = await fetch(url);
            
            if (!response.ok) {
                throw new Error(`Failed to load oil catalog: ${response.status}`);
            }

            const blob = await response.blob();
            const ds = new DecompressionStream('gzip');
            const decompressedStream = blob.stream().pipeThrough(ds);
            const resp = new Response(decompressedStream);
            const text = await resp.text();
            
            catalog = JSON.parse(text);
            return catalog!;
        } finally {
            catalogPromise = null;
        }
    })();

    return catalogPromise;
}

export function searchOils(query: string): OilRecord[] {
    if (!catalog) {
        console.warn('Oil catalog not loaded yet');
        return [];
    }

    const q = query.toLowerCase().trim();

    if (q.length === 0) {
        return getGenericOils();
    }

    return catalog.oils
        .filter((oil) => {
            const name = oil.name?.toLowerCase() || '';
            const id = oil.oil_id?.toLowerCase() || '';
            return name.includes(q) || id.includes(q);
        })
        .slice(0, 100);
}

export function getOilById(id: string): OilRecord | null {
    if (!catalog) {
        console.warn('Oil catalog not loaded yet');
        return null;
    }

    return catalog.oils.find((oil) => oil.oil_id === id) || null;
}

export function getGenericOils(): OilRecord[] {
    if (!catalog) {
        console.warn('Oil catalog not loaded yet');
        return [];
    }

    const genericIds = [
        'Generic Condensate',
        'Generic Diesel',
        'Generic Gasoline',
        'Generic Heavy Crude',
        'Generic Heavy Fuel Oil',
        'Generic IFO',
        'Generic Jet Fuel',
        'Generic Light Crude',
        'Generic Medium Crude',
    ];

    return catalog.oils
        .filter((oil) => {
            const name = oil.name.toLowerCase() || '';
            return genericIds.some((id) => name.includes(id.toLowerCase()));
        });
}

export function getOilJsonForRust(oilId: string): string {
    const oil = getOilById(oilId);
    if (!oil) {
        throw new Error(`Oil not found: ${oilId}`);
    }

    const oilProperties = {
        product_type: oil.name,
        api: oilOverrides.api ?? oil.api_gravity,
        density_kgm3: oil.density_kgm3,
        dynamic_viscosity_cp: oil.dynamic_viscosity_cp,
        interfacial_tension_n_m: oil.interfacial_tension_n_m,
        sara_mass_fractions: oil.sara_mass_fractions,
        distillation_cuts: oil.distillation_cuts,
        boiling_points_c: oil.boiling_points_c,
        molecular_weights_kg_mol: oil.molecular_weights_kg_mol,
        component_mass_fractions: oil.component_mass_fractions,
        bullwinkle_fraction: oilOverrides.bullwinkleFrac ?? oil.bullwinkle_fraction,
    };

    return JSON.stringify(oilProperties);
}