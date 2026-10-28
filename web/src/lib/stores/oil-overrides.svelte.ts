export interface OilOverrides {
    query: string;
    id: string;
    windFactor: number;
    deflectionScheme: "samuels" | "constant"
    windDeflection: number | null;
    emulOnset: "oil" | "time" | "fraction"
    bullwinkleFrac: number;
    bulltime: number;
}

export const oilOverrides: OilOverrides = $state({
    query: "Generic Light Crude",
    id: "GN00006",
    windFactor: 3,
    windDeflection: 0.0,
    deflectionScheme: "constant",
    emulOnset: "oil",
    bullwinkleFrac: 0,
    bulltime: 0
});