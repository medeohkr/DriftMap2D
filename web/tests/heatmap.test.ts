// heatmap.test.ts checks shared heatmap results and cache invalidation

import assert from "node:assert/strict";
import test from "node:test";
import { HeatmapCache } from "../src/lib/heatmap.ts";

function source() {
    return {
        day: 0, calls: 0,
        current_day() { return this.day; },
        generate_heatmap(cell: number, smoothing: number, levels: Float32Array) {
            this.calls++;
            return JSON.stringify({ type: "FeatureCollection", features: [{cell, smoothing, levels: [...levels], day: this.day}] });
        },
    };
}

test("snapshot capture and display share the same result without mutating earlier snapshots", () => {
    const cache = new HeatmapCache(), engine = source();
    const snapshot = cache.get(engine, 0.01, 2, [0.25, 0.5]);
    const saved = JSON.stringify(snapshot);
    assert.equal(cache.get(engine, 0.01, 2, [0.25, 0.5]), snapshot);
    assert.equal(engine.calls, 1);
    engine.day = 0.25;
    assert.notEqual(cache.get(engine, 0.01, 2, [0.25, 0.5]), snapshot);
    assert.equal(engine.calls, 2);
    assert.equal(JSON.stringify(snapshot), saved);
});

test("settings changes, new engines, and resets invalidate the cache", () => {
    const cache = new HeatmapCache(), engine = source();
    cache.get(engine, 0.01, 2, [0.25]);
    cache.get(engine, 0.02, 2, [0.25]);
    cache.get(engine, 0.02, 3, [0.25]);
    cache.get(engine, 0.02, 3, [0.5]);
    assert.equal(engine.calls, 4);
    const next = source();
    cache.get(next, 0.02, 3, [0.5]);
    assert.equal(next.calls, 1);
    cache.clear();
    cache.get(next, 0.02, 3, [0.5]);
    assert.equal(next.calls, 2);
});

test("failed generations do not replace the last valid cached result", () => {
    const cache = new HeatmapCache(), engine = source();
    const valid = cache.get(engine, 0.01, 2, [0.25]);
    const generate = engine.generate_heatmap;
    engine.generate_heatmap = () => { throw new Error("generation failed"); };
    engine.day = 1;
    assert.throws(() => cache.get(engine, 0.01, 2, [0.25]), /generation failed/);
    engine.day = 0;
    assert.equal(cache.get(engine, 0.01, 2, [0.25]), valid);
    engine.generate_heatmap = generate;
});
