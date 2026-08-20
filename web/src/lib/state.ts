import { dom } from './ui'
import type { Proteus } from '../../pkg/proteus'
import * as maplibregl from 'maplibre-gl';

export interface State {
    proteus: Proteus | null;
    simulationRunning: boolean;
    animationId: number | null;
    simulationVersion: number;
    simulationHistory: any[];
    timelineDay: number;
    timelinePlaying: boolean;
    timelineAnimationId: number | null;
    playbackSpeed: number;
    heatmap: any | null;
    lastGridUpdate: number;
    visualizationMode: "particles" | "grid";
    rawLon: number;
    rawLat: number;
    csValue: number;
    particleCount: number;
    spreadKm: number;
    startYear: number;
    startMonth: number;
    startDay: number;
    stepsPerDay: number;
    totalDays: number;
    playbackMode: boolean;
    stepCount: number;
    boundingBox: number[];
    releaseAmount: number;
    releaseDuration: number;
    legendCollapsed: boolean;
    landmaskPromise: Promise<void> | null;
    currentMarker: maplibregl.Marker | null;
}

const today = new Date();

export const state: State = {
    proteus: null,
    simulationRunning: false,
    animationId: null,
    simulationVersion: 0,
    simulationHistory: [],
    timelineDay: 0,
    timelinePlaying: false,
    timelineAnimationId: null,
    playbackSpeed: 100,
    heatmap: null,
    lastGridUpdate: 0,
    visualizationMode: "particles",
    rawLon: 56.5,
    rawLat: 26.6,
    csValue: 0.1,
    particleCount: 10000,
    spreadKm: parseFloat(dom.releaseRadiusField.value),
    startYear: today.getFullYear(),
    startMonth: today.getMonth() + 1,
    startDay: today.getDate(),
    stepsPerDay: 96,
    totalDays: parseFloat(dom.totalDaysField.value),
    playbackMode: false,
    stepCount: 0,
    boundingBox: [],
    releaseAmount: parseFloat(dom.releaseAmountField.value),
    releaseDuration: parseFloat(dom.releaseDurationField.value),
    legendCollapsed: false,
    landmaskPromise: null,
    currentMarker: null,
}

