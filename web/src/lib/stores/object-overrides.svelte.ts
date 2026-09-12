export interface ObjectOverrides {
    downwind: number[] | null;
    right: number[] | null;
    left: number[] | null;
    jibeProbability: number;
    capsizing: boolean;
    capsizeThreshold: number;
    capsizeFraction: number;
    capsizeSigma: number;

}

export const objectOverrides: ObjectOverrides = {
    downwind: null,
    right: null,
    left: null,
    jibeProbability: 0.04,
    capsizing: false,
    capsizeThreshold: 30,
    capsizeFraction: 0.4,
    capsizeSigma: 5
};