import type { Proteus } from "../../pkg/proteus";

export interface Simulation {
    proteus: Proteus | null;
    simulationActive: boolean;
    simulationRunning: boolean;
    simulationVersion: number;
    animationId: number | null;
    landmaskPromise: Promise<void> | null;
    stepCount: number;
    currentTime: string;
}

export const simulation: Simulation = $state({
    proteus: null,
    simulationActive: false,
    simulationRunning: false,
    simulationVersion: 0,
    animationId: null,
    landmaskPromise: null,
    stepCount: 0,
    currentTime: "",
});