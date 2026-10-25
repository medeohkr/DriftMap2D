import { dateOffset, DEFAULT_SIMULATION_DAYS } from "../utils";

export interface Config {
    particleCount: number;
    timeStepMin: number;
    advectionScheme: string;
    diffusionScheme: string;
    diffusionCoeffs: number[];
    startDate: string;
    startTime: string;
    endDate: string;
    endTime: string;
    totalDays: number;
    tracerType: string;
    forcingModel: "global" | "norkyst"
}

export const simulationTimeState = $state({ usingDefault: true, defaultVersion: 0, importVersion: 0 });

export const config: Config = $state({
    particleCount: 10000,
    timeStepMin: 15,
    advectionScheme: "euler",
    diffusionScheme: "constant",
    diffusionCoeffs: [10, 0.1],

    startDate: dateOffset(0),
    startTime: "00:00",
    endDate: dateOffset(DEFAULT_SIMULATION_DAYS),
    endTime: "00:00",
    totalDays: DEFAULT_SIMULATION_DAYS,

    tracerType: "generic",
    forcingModel: "global"
});
