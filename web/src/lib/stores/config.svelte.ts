import { dateOffset } from "../utils";

export interface Config {
    csValue: number;
    particleCount: number;
    stepsPerDay: number;
    startDate: string;
    startTime: string;
    endDate: string;
    endTime: string;
    totalDays: number;
    tracerType: string;
    tracerJson: string;
    autoZoom: boolean;
}

export const config: Config = $state({
    csValue: 0.05,
    particleCount: 10000,
    stepsPerDay: 96,

    startDate: dateOffset(0),
    startTime: "00:00",
    endDate: dateOffset(7),
    endTime: "00:00",
    totalDays: 7,

    tracerType: "generic",
    tracerJson: "",

    autoZoom: true,
});