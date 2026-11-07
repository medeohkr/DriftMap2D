// particles.browser.ts checks real shader output and compares source update latency

// @ts-expect-error the application loads MapLibre from the same CDN
import * as maplibregl from "https://unpkg.com/maplibre-gl@^6.6.0/dist/maplibre-gl.mjs";
import { ParticleLayer } from "../src/lib/particleLayer";

const results: Record<string, unknown> = {};
const layer = new ParticleLayer();
const empty = new Float32Array();
const map = new maplibregl.Map({
    container: "map",
    style: { version: 8, sources: {}, layers: [{ id: "background", type: "background", paint: { "background-color": "black" } }] },
    center: [5, 60], zoom: 8,
    attributionControl: false,
    canvasContextAttributes: { preserveDrawingBuffer: true },
});

function check(condition: boolean, message: string) {
    if (!condition) throw new Error(message);
}

async function idle() {
    await Promise.race([
        new Promise(resolve => { map.once("idle", resolve); map.triggerRepaint(); }),
        new Promise((_, reject) => setTimeout(() => reject(new Error("map did not become idle")), 15000)),
    ]);
}

function pixel(lon: number, lat: number, offsetX = 0) {
    const point = map.project([lon, lat]);
    point.x += offsetX;
    const canvas = map.getCanvas();
    const gl = canvas.getContext("webgl2")!;
    const ratio = canvas.width / canvas.clientWidth;
    const rgba = new Uint8Array(4);
    gl.readPixels(Math.round(point.x * ratio), canvas.height - 1 - Math.round(point.y * ratio), 1, 1,
        gl.RGBA, gl.UNSIGNED_BYTE, rgba);
    return [...rgba];
}

function colored(lon: number, lat: number, stranded = false) {
    const rgba = pixel(lon, lat);
    check(rgba[0] > 100 && (stranded ? rgba[1] < 90 : rgba[1] > 100),
        `incorrect ${stranded ? "stranded" : "active"} pixel at ${lon}, ${lat} ${rgba}`);
}

async function run() {
    await new Promise(resolve => map.once("load", resolve));
    map.addLayer(layer);
    layer.setRadius(4);
    layer.setPositions(new Float32Array([5, 60]), new Float32Array([5.05, 60]));
    await idle();
    colored(5, 60);
    colored(5.05, 60, true);
    check(pixel(5, 60, 3)[0] > 100 && pixel(5, 60, 6)[0] === 0, "particle radius differs from CSS pixels");
    const edge = pixel(5, 60, 4)[0];
    check(edge > 0 && edge < 170, "particle edge is not antialiased");
    results.colors = "passed";
    results.pixelRatio = window.devicePixelRatio;
    results.radiusAndAntialiasing = "passed";

    map.jumpTo({ center: [5.025, 60], zoom: 10, bearing: 63, pitch: 45 });
    await idle();
    colored(5, 60);
    colored(5.05, 60, true);
    results.rotationAndPitch = "passed";

    const close = new Float32Array([5.00003, 60]);
    map.jumpTo({ center: [5.000015, 60], zoom: 22, bearing: 0, pitch: 0 });
    layer.setPositions(new Float32Array([5, 60]), close);
    await idle();
    colored(5, 60);
    colored(close[0], close[1], true);
    results.highZoom = "passed";

    const across = new Float32Array([179.9, 10, -179.9, 10]);
    layer.setPositions(across, empty);
    map.jumpTo({ center: [540, 10], zoom: 8 });
    await idle();
    colored(across[0] + 360, 10);
    colored(across[2] + 720, 10);
    results.antimeridianAndUnwrappedCenter = "passed";

    map.jumpTo({ center: [0, 0], zoom: 0 });
    layer.setPositions(new Float32Array([0, 0]), empty);
    await idle();
    colored(-360, 0);
    colored(0, 0);
    colored(360, 0);
    map.setRenderWorldCopies(false);
    await idle();
    colored(0, 0);
    check(pixel(360, 0)[0] === 0 && pixel(-360, 0)[0] === 0, "disabled world copies still visible");
    results.worldCopies = "passed";

    layer.setVisible(false);
    await idle();
    check(pixel(0, 0)[0] === 0, "hidden particle still visible");
    layer.setPositions(new Float32Array([10, 0]), empty);
    layer.setVisible(true);
    await idle();
    colored(10, 0);
    layer.clear();
    await idle();
    check(pixel(10, 0)[0] === 0, "reset left a particle visible");
    results.visibilityAndReset = "passed";

    layer.setPositions(new Float32Array([0, 0]), empty);
    const gl = map.getCanvas().getContext("webgl2")!;
    const extension = gl.getExtension("WEBGL_lose_context");
    if (extension) {
        const lost = new Promise(resolve => map.once("webglcontextlost", resolve));
        extension.loseContext();
        await lost;
        await new Promise(resolve => setTimeout(resolve, 100));
        const restored = new Promise(resolve => map.once("webglcontextrestored", resolve));
        extension.restoreContext();
        await restored;
        await idle();
        colored(0, 0);
        results.contextRecovery = "passed";
    }

    map.jumpTo({ center: [5, 60], zoom: 7 });
    layer.setRadius(1.5);
    const positions = new Float32Array(200000);
    for (let i = 0; i < 100000; i++) {
        positions[i * 2] = 3 + (i % 317) / 317 * 4;
        positions[i * 2 + 1] = 59 + Math.floor(i / 317) / 316 * 2;
    }
    let instances = 0;
    const originalDraw = gl.drawArraysInstanced.bind(gl);
    gl.drawArraysInstanced = (mode, first, count, instanceCount) => {
        instances += instanceCount;
        originalDraw(mode, first, count, instanceCount);
    };
    layer.setPositions(positions, empty);
    await idle();
    gl.drawArraysInstanced = originalDraw;
    check(instances >= 100000, "not all 100k particles were drawn");
    results.drawnParticlesPerFrame = 100000;

    const gpuUpdates: number[] = [];
    for (let i = 0; i < 8; i++) {
        const start = performance.now();
        layer.setPositions(positions.slice(), empty);
        await idle();
        gpuUpdates.push(performance.now() - start);
    }
    layer.setVisible(false);
    const data = () => ({
        type: "FeatureCollection", features: Array.from({ length: 100000 }, (_, i) => ({
            type: "Feature", geometry: { type: "Point", coordinates: [positions[i * 2], positions[i * 2 + 1]] },
        })),
    });
    map.addSource("reference", { type: "geojson", data: data() });
    map.addLayer({ id: "reference", type: "circle", source: "reference",
        paint: { "circle-radius": 1.5, "circle-color": "white", "circle-opacity": 0.7 } });
    await idle();
    const geojsonUpdates: number[] = [];
    for (let i = 0; i < 8; i++) {
        const start = performance.now();
        map.getSource("reference").setData(data());
        await idle();
        geojsonUpdates.push(performance.now() - start);
    }
    const median = (times: number[]) => times.sort((a, b) => a - b)[Math.floor(times.length / 2)];
    results.updateToIdleMedianMs = { webgl: median(gpuUpdates), geojson: median(geojsonUpdates) };
    map.removeLayer("reference");
    map.removeSource("reference");
    map.removeLayer(layer.id);
    check(gl.getError() === gl.NO_ERROR, "WebGL error after rendering");
    results.cleanup = "passed";
}

run().then(() => {
    (window as any).particleResults = { passed: true, ...results };
}).catch(error => {
    (window as any).particleResults = { passed: false, ...results, error: String(error), stack: error.stack };
}).finally(() => {
    document.getElementById("result")!.textContent = JSON.stringify((window as any).particleResults, null, 2);
});
