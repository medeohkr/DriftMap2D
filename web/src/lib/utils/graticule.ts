/** graticule.ts controls grid spacing and readable edge labels */
export function getGraticuleSpacing(zoom: number): number {
    const spacing = [20, 10, 5, 2, 1, 0.5, 0.25, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005, 0.002, 0.001];
    const level = Math.max(0, Math.floor(zoom));
    return spacing[Math.min(level, spacing.length - 1)] / 2 ** Math.max(0, level - spacing.length + 1);
}

/** show only the decimal places needed by the current grid spacing */
export function formatGraticuleLabel(value: number, spacing: number): string {
    let precision = 0;
    while (precision < 8 && Math.abs(spacing * 10 ** precision - Math.round(spacing * 10 ** precision)) > 1e-9) {
        precision++;
    }
    return `${Number(value.toFixed(precision))}°`;
}

/** hide colliding labels while preserving all grid lines */
export function hideOverlappingGraticuleLabels(container: HTMLElement): void {
    const accepted: DOMRect[] = [];
    const labels = Array.from(container.querySelectorAll<HTMLElement>(".geogrid__label"));
    const bounds = labels.map(label => label.getBoundingClientRect());
    labels.forEach((label, index) => {
        const rect = bounds[index];
        if (!rect.width || !rect.height) return;
        const overlaps = accepted.some(other => rect.left < other.right + 8
            && rect.right + 8 > other.left && rect.top < other.bottom + 8
            && rect.bottom + 8 > other.top);
        label.style.visibility = overlaps ? "hidden" : "visible";
        if (!overlaps) accepted.push(rect);
    });
}
