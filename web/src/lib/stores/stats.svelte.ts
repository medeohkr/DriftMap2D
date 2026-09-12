export interface Stats {
    stranded: string;
    emulsified: string;
    evaporated: string;
    totalMass: string;
}

export const stats: Stats = $state({
    stranded: "",
    emulsified: "",
    evaporated: "",
    totalMass: "",
});