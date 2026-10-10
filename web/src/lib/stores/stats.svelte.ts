export interface Stats {
    activeParticles: string;
    stranded: string;
    emulsified: string;
    evaporated: string;
    totalMass: string;
}

export const stats: Stats = $state({
    activeParticles: "",
    stranded: "",
    emulsified: "",
    evaporated: "",
    totalMass: "",
});