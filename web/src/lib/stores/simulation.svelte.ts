import type { Proteus } from "../../pkg/proteus_wasm";

export interface Simulation {
    proteus: Proteus | null;
    simulationActive: boolean;
    simulationRunning: boolean;
    simulationVersion: number;
    animationId: number | null;
    stepCount: number;
    currentTime: string;
}

export const simulation: Simulation = $state({
    proteus: null,
    simulationActive: false,
    simulationRunning: false,
    simulationVersion: 0,
    animationId: null,
    stepCount: 0,
    currentTime: "",
});