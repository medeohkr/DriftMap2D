import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { build } from '../../web/node_modules/vite/dist/node/index.js';
import { svelte } from '../../web/node_modules/@sveltejs/vite-plugin-svelte/src/index.js';

const root = fileURLToPath(new URL('../../web', import.meta.url));
const stores = `${root}/src/lib/stores/index.svelte.ts`.replaceAll('\\', '/');
const entry = `
    import { mount, flushSync } from 'svelte';
    import Timeline from '${root.replaceAll('\\', '/')}/src/components/Timeline.svelte';
    import { timeline, history } from '$lib/stores/index.svelte';
    mount(Timeline, { target: document.body });
    const results = [];
    const snapshot = index => ({
        dateStr: 'snapshot ' + index,
        stats: { activeParticles: '10', stranded: '0', emulsified: '0', evaporated: '0', totalMass: '1' },
        unstrandedGeojson: { index }, strandedGeojson: { index }, heatmapGeojson: { index },
    });
    const inspect = label => {
        const slider = document.querySelector('#timeline-slider');
        const result = { label, value: +slider.value, max: +slider.max, index: timeline.timelineDay,
            date: document.querySelector('.timeline-labels').textContent.trim(), rendered: window.rendered };
        results.push(result);
        if (result.value !== result.index || result.rendered !== result.index || result.date !== 'snapshot ' + result.index) {
            throw new Error('slider and snapshot diverged ' + JSON.stringify(result));
        }
    };
    try {
        for (const count of [241, 241, 97, 289]) {
            flushSync(() => { timeline.playbackMode = false; history.simulationHistory = []; });
            history.simulationHistory = Array.from({ length: count }, (_, i) => snapshot(i));
            flushSync(() => { timeline.playbackMode = true; });
            inspect('completed ' + count);
            document.querySelector('#timeline-rewind').click();
            flushSync();
            inspect('rewound');
            const slider = document.querySelector('#timeline-slider');
            slider.value = '12';
            slider.dispatchEvent(new Event('input', { bubbles: true }));
            flushSync();
            inspect('scrubbed');
            // leave the previous run at its endpoint before starting another run
            slider.value = String(count - 1);
            slider.dispatchEvent(new Event('input', { bubbles: true }));
            flushSync();
        }
        document.body.innerHTML = '<pre id="results">' + JSON.stringify({ passed: true, results }) + '</pre>';
    } catch (error) {
        document.body.innerHTML = '<pre id="results">' + JSON.stringify({ passed: false, error: String(error), results }) + '</pre>';
    }
`;

const output = await build({
    root, configFile: false, logLevel: 'error',
    resolve: { alias: { $lib: `${root}/src/lib` } },
    plugins: [{
        name: 'timeline-fixture', enforce: 'pre',
        resolveId(id) { if (id === 'timeline-fixture') return '\0timeline-fixture'; },
        load(id) {
            if (id === '\0timeline-fixture') return entry;
            const normalized = id.replaceAll('\\', '/');
            if (normalized === stores) return `
                export { timeline } from './timeline.svelte';
                export { history } from './history.svelte';
                export const config = { timeStepMin: 30 };
                export const visualization = { snapshotInterval: 1 };
                export const stats = {};
            `;
            if (normalized === `${root}/src/lib/map.ts`.replaceAll('\\', '/')) {
                return 'export const map = { getSource: () => ({ setData: data => { window.rendered = data.index; } }) };';
            }
        },
    }, svelte()],
    build: {
        write: false, minify: false,
        rollupOptions: { input: 'timeline-fixture', output: { format: 'iife' } },
    },
});
const javascript = output.output.find(item => item.type === 'chunk').code;
const directory = await mkdtemp(path.join(tmpdir(), 'driftmap-timeline-'));
try {
    const html = path.join(directory, 'test.html');
    await writeFile(html, '<!doctype html><body><script>' + javascript.replaceAll('</script', '<\\/script') + '</script>');
    const browser = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
    const result = spawnSync(browser, ['--headless=new', '--disable-gpu', '--no-first-run',
        `--user-data-dir=${path.join(directory, 'profile')}`, '--dump-dom', '--virtual-time-budget=3000',
        pathToFileURL(html).href], { encoding: 'utf8', windowsHide: true, timeout: 30000, maxBuffer: 4 * 1024 * 1024 });
    if (result.error) throw result.error;
    const match = result.stdout.match(/<pre id="results">(.*?)<\/pre>/s);
    assert.ok(match, 'browser did not return test results ' + result.stderr.slice(-1000));
    const report = JSON.parse(match[1].replaceAll('&quot;', '"').replaceAll('&amp;', '&'));
    console.log(JSON.stringify(report, null, 2));
    assert.equal(report.passed, true, report.error);
} finally {
    assert.equal(path.dirname(directory), path.resolve(tmpdir()));
    assert.ok(path.basename(directory).startsWith('driftmap-timeline-'));
    await rm(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
