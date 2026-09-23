export interface ObjectOverrides {
    query: string;
    id: string;
    jibeProbability: number;
    capsizing: boolean;
    capsizeThreshold: number;
    capsizeFraction: number;
    capsizeSigma: number;

}

export const objectOverrides: ObjectOverrides = $state({
    query: "",
    id: "",
    jibeProbability: 4,
    capsizing: false,
    capsizeThreshold: 30,
    capsizeFraction: 40,
    capsizeSigma: 5
});