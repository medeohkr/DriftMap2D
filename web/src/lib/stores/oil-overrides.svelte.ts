export interface OilOverrides {
    query: string;
    id: string;
    windFactor: number;
    deflectionScheme: "samuels" | "constant"
    windDeflection: number | null;
    emulOnset: "time" | "fraction"
    bullwinkleFrac: number;
    bulltime: number;
}

export const oilOverrides: OilOverrides = $state({
    query: "",
    id: "",
    windFactor: 3.5,
    windDeflection: 20.0,
    deflectionScheme: "samuels",
    emulOnset: "time",
    bullwinkleFrac: 0,
    bulltime: 0
});