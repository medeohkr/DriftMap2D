import assert from "node:assert/strict";
import test from "node:test";
import { ParticleLayer, projectParticles } from "../src/lib/particleLayer.ts";
import { exportSnapshot, importSnapshot } from "../src/lib/io/snapshots.ts";

test("projection keeps antimeridian particles together and skips invalid positions", () => {
    const positions = new Float32Array([179.9, 0, -179.9, 0, NaN, 45]);
    const output = new Float32Array(positions.length);
    const result = projectParticles(positions, output);
    assert.equal(result.count, 2);
    assert.ok(result.maxX - result.minX < 0.001);
    assert.equal(result.originY, 0.5);
    assert.ok(Math.abs(output[2] - (result.maxX - result.minX)) < 1e-9);
});

test("projection retains submeter offsets at high zoom", () => {
    const positions = new Float32Array([5, 60, 5.000001, 60.000004]);
    const output = new Float32Array(positions.length);
    const { originX, originY } = projectParticles(positions, output);
    const lon = (originX + output[2]) * 360 - 180;
    const lat = Math.atan(Math.sinh(Math.PI * (1 - 2 * (originY + output[3])))) * 180 / Math.PI;
    assert.ok(Math.abs(lon - positions[2]) < 1e-10);
    assert.ok(Math.abs(lat - positions[3]) < 1e-10);
});

test("snapshot roundtrip retains both particle groups and the existing GeoJSON format", () => {
    const snapshot = {
        day: 2, dateStr: "2026-11-06 00:00:00",
        activePositions: new Float32Array([179.9, 60, -179.9, 61]),
        strandedPositions: new Float32Array([5, 60]),
        heatmapGeojson: { type: "FeatureCollection", features: [] },
        stats: { activeParticles: "2", stranded: "33.3", emulsified: "0", evaporated: "0", totalMass: "1" },
    };
    const file = JSON.parse(JSON.stringify(exportSnapshot(snapshot)));
    assert.equal(file.geometry.geometries[0].type, "MultiPoint");
    assert.equal(file.geometry.geometries[1].coordinates.length, 1);
    assert.deepEqual(importSnapshot(file), snapshot);
    file.geometry.geometries[0].coordinates = [];
    delete file.properties.heatmap;
    assert.equal(importSnapshot(file).activePositions.length, 0);
    assert.equal(importSnapshot(file).heatmapGeojson, null);
});

test("snapshot import rejects invalid coordinates", () => {
    assert.throws(() => importSnapshot({
        properties: {}, geometry: { geometries: [{ coordinates: [[null, 60]] }, { coordinates: [] }] },
    } as any), /Invalid particle coordinates/);
});

// real shader compilation and pixel placement are checked in the browser harness
function environment() {
    const calls = { allocations: 0, uploads: 0, draws: 0, instances: 0, deleted: 0 };
    const listeners = new Map<string, () => void>();
    let west = -180;
    let east = 540;
    let repeats = true;
    const gl: any = {
        drawingBufferWidth: 1200, drawingBufferHeight: 800,
        createShader: () => ({}), shaderSource() {}, compileShader() {},
        getShaderParameter: () => true, deleteShader() {},
        createProgram: () => ({}), attachShader() {}, linkProgram() {},
        getProgramParameter: () => true, getUniformLocation: () => ({}),
        createBuffer: () => ({}), bindBuffer() {},
        bufferData: () => calls.allocations++, bufferSubData: () => calls.uploads++,
        createVertexArray: () => ({}), bindVertexArray() {},
        enableVertexAttribArray() {}, vertexAttribPointer() {}, vertexAttribDivisor() {},
        isContextLost: () => false, useProgram() {},
        uniform1f() {}, uniform2f() {}, uniform3f() {}, uniformMatrix4fv() {},
        enable() {}, blendEquation() {}, blendFunc() {},
        drawArraysInstanced: (_mode: number, _first: number, _vertices: number, count: number) => {
            calls.draws++;
            calls.instances += count;
        },
        deleteBuffer: () => calls.deleted++, deleteVertexArray: () => calls.deleted++,
        deleteProgram: () => calls.deleted++,
    };
    const map: any = {
        triggerRepaint() {},
        getBounds: () => ({ getWest: () => west, getEast: () => east }),
        getZoom: () => 1, getPitch: () => 0, getRenderWorldCopies: () => repeats,
        getCanvas: () => ({ clientWidth: 1200 }),
        getLayer: () => ({}), isStyleLoaded: () => true, addLayer() {},
        on: (event: string, listener: () => void) => listeners.set(event, listener),
        off: (event: string) => listeners.delete(event),
    };
    return { gl, map, calls, listeners, view: (w: number, e: number, r = true) => {
        west = w; east = e; repeats = r;
    } };
}

const input = { defaultProjectionData: { mainMatrix: new Float64Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]) } };

test("renderer draws 100k particles per world copy without uploading again on camera movement", () => {
    const { gl, map, calls, view } = environment();
    const layer = new ParticleLayer();
    const active = new Float32Array(200000);
    const empty = new Float32Array();
    layer.onAdd(map, gl);
    layer.setPositions(active, empty);
    layer.render(gl, input);
    assert.equal(calls.instances, 200000);
    assert.equal(calls.uploads, 1);
    const allocations = calls.allocations;
    view(180, 540);
    layer.render(gl, input);
    assert.equal(calls.uploads, 1);
    layer.setPositions(active.slice(), empty);
    layer.render(gl, input);
    assert.equal(calls.uploads, 2);
    assert.equal(calls.allocations, allocations);
    const draws = calls.draws;
    view(180, 540, false);
    layer.render(gl, input);
    assert.equal(calls.draws, draws);
});

test("hidden updates coalesce and reset removes all particle draws", () => {
    const { gl, map, calls } = environment();
    const layer = new ParticleLayer();
    layer.onAdd(map, gl);
    layer.setVisible(false);
    layer.setPositions(new Float32Array([0, 0]), new Float32Array());
    layer.render(gl, input);
    assert.equal(calls.uploads, 0);
    layer.setPositions(new Float32Array([10, 10, 11, 11]), new Float32Array());
    layer.setVisible(true);
    layer.render(gl, input);
    assert.equal(calls.uploads, 1);
    assert.equal(calls.instances, 4);
    const draws = calls.draws;
    layer.clear();
    layer.render(gl, input);
    assert.equal(calls.draws, draws);
});

test("context recovery reuploads particles and removal releases resources and listeners", () => {
    const { gl, map, calls, listeners } = environment();
    const layer = new ParticleLayer();
    layer.onAdd(map, gl);
    layer.setPositions(new Float32Array([0, 0]), new Float32Array([10, 10]));
    layer.render(gl, input);
    assert.equal(calls.uploads, 2);
    listeners.get("webglcontextlost")!();
    listeners.get("webglcontextrestored")!();
    layer.render(gl, input);
    assert.equal(calls.uploads, 4);
    layer.onRemove(map, gl);
    assert.equal(listeners.size, 0);
    assert.equal(calls.deleted, 6);
});
