import { dateOffset } from "../utils";

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
}

export const config: Config = $state({
    particleCount: 5000,
    timeStepMin: 15,
    advectionScheme: "rk2",
    diffusionScheme: "constant",
    diffusionCoeffs: [10, 0.1],

    startDate: dateOffset(0),
    startTime: "00:00",
    endDate: dateOffset(7),
    endTime: "00:00",
    totalDays: 7,

    tracerType: "generic",
});