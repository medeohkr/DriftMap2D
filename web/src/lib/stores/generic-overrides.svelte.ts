export interface GenericOverrides {
    windFactor: number;
    windDeflection: number;
}

export const genericOverrides: GenericOverrides = $state({
    windFactor: 2.0,
    windDeflection: 0.0,
});