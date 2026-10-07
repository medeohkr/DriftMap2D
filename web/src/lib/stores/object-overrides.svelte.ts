export interface ObjectOverrides {
    query: string;
    id: string;
    jibeProbability: number;
    randomOrientation: boolean,
    capsizing: boolean;
    capsizeThreshold: number;
    capsizeFraction: number;
    capsizeSigma: number;

}

export const objectOverrides: ObjectOverrides = $state({
    query: "Person in water, unknown state (mean values)",
    id: "Person in water, unknown state (mean values)",
    jibeProbability: 4,
    randomOrientation: true,
    capsizing: false,
    capsizeThreshold: 30,
    capsizeFraction: 40,
    capsizeSigma: 5
});