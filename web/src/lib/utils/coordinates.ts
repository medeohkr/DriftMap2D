export function normalizeLongitude(lon: number) {
    return ((((lon + 180) % 360) + 360) % 360) - 180;
}
