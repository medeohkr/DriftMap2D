import { loadNorkystCoverage, loadNorkystCoverageMask } from "./forcingCoverage";

const layers = ["norkyst-coverage-fill", "norkyst-coverage-outline", "norkyst-unavailable-water"];
const revisions = new WeakMap<object, number>();
const drawnMasks = new WeakMap<object, string>();

export async function renderForcingCoverage(target: any, visible: boolean): Promise<void> {
    if (!target) return;
    if (!revisions.has(target)) target.once("remove", () => { revisions.delete(target); drawnMasks.delete(target); });
    const revision = (revisions.get(target) ?? 0) + 1;
    revisions.set(target, revision);
    const current = () => revisions.get(target) === revision;
    for (const id of layers) {
        if (target.getLayer(id)) target.setLayoutProperty(id, "visibility",
            visible && id !== "norkyst-unavailable-water" ? "visible" : "none");
    }
    if (!visible) return;
    if (!target.isStyleLoaded()) {
        target.once("idle", () => { if (current()) void renderForcingCoverage(target, visible); });
        return;
    }
    try {
        const coverage = await loadNorkystCoverage();
        if (!current() || !target.isStyleLoaded()) return;
        const before = target.getStyle().layers.find((layer: { type: string }) => layer.type === "symbol")?.id;
        if (!target.getSource("norkyst-coverage")) {
            target.addSource("norkyst-coverage", { type: "geojson", data: coverage.feature, tolerance: 0 });
            target.addLayer({
                id: "norkyst-coverage-fill", type: "fill", source: "norkyst-coverage",
                paint: { "fill-color": "#808080", "fill-opacity": 0.1 },
            }, before);
            target.addLayer({
                id: "norkyst-coverage-outline", type: "line", source: "norkyst-coverage",
                layout: { "line-cap": "round", "line-join": "round" },
                paint: { "line-color": "#707070", "line-width": 2, "line-dasharray": [0.1, 2] },
            }, before);
        } else {
            target.getSource("norkyst-coverage").setData(coverage.feature);
        }
        const support = await loadNorkystCoverageMask();
        if (!current() || !target.isStyleLoaded()) return;
        const existing = target.getSource("norkyst-unavailable-water");
        if (existing) {
            if (drawnMasks.get(target) !== support.revision) existing.setData(support.unavailableWater());
            drawnMasks.set(target, support.revision);
            target.setLayoutProperty("norkyst-unavailable-water", "visibility", "visible");
            return;
        }
        if (!target.hasImage("norkyst-hatch")) target.addImage("norkyst-hatch", hatchImage());
        target.addSource("norkyst-unavailable-water", { type: "geojson", data: support.unavailableWater(), tolerance: 0 });
        target.addLayer({
            id: "norkyst-unavailable-water", type: "fill", source: "norkyst-unavailable-water",
            paint: { "fill-pattern": "norkyst-hatch", "fill-opacity": 0.85, "fill-antialias": false },
        }, "norkyst-coverage-outline");
        drawnMasks.set(target, support.revision);
    } catch (error) {
        if (current()) console.warn("NorKyst coverage could not be loaded", error);
    }
}

function hatchImage() {
    const width = 80;
    const height = Math.round(width * Math.tan(35 * Math.PI / 180));
    const repeats = 8;
    const stripeWidth = 0.25;
    const edgeWidth = repeats * Math.hypot(1 / width, 1 / height);
    const data = new Uint8Array(width * height * 4);
    // a rectangular repeat keeps the rotated stripes seamless at tile edges
    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const offset = (y * width + x) * 4;
            const phase = (((x + 0.5) / width + (y + 0.5) / height) * repeats) % 1;
            const centerDistance = Math.abs(phase - stripeWidth / 2);
            const distance = Math.min(centerDistance, 1 - centerDistance);
            // blend over one pixel across each edge, including the repeat boundary
            const coverage = Math.max(0, Math.min(1, 0.5 + (stripeWidth / 2 - distance) / edgeWidth));
            data.set([150, 150, 150, Math.round(240 * coverage)], offset);
        }
    }
    return { width, height, data };
}
