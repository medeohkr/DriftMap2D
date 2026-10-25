import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';

const source = await readFile(new URL('../../web/src/lib/utils/forcing.ts', import.meta.url), 'utf8');
const forcing = await import(`data:text/javascript;base64,${Buffer.from(stripTypeScriptTypes(source)).toString('base64')}`);
const timestamps = (start, count) => Array.from({ length: count }, (_, i) => Date.parse(start) / 1000 + i * 3600);

test('global availability follows the published partial final day', async () => {
    const original = globalThis.fetch;
    const days = [
        { timestamps: timestamps('2026-10-25T00:00:00Z', 25) },
        { timestamps: timestamps('2026-10-26T00:00:00Z', 24) },
    ];
    let requests = 0;
    globalThis.fetch = async (url, options) => {
        assert.equal(url, 'https://tiles.driftmap2d.com/tiles/manifest.json');
        assert.equal(options.cache, 'no-cache');
        requests++;
        return new Response(JSON.stringify({ days }));
    };
    try {
        const [first, second] = await Promise.all([forcing.loadForcingWindow('global'), forcing.loadForcingWindow('global')]);
        assert.deepEqual(first, second);
        assert.equal(requests, 1);
        assert.equal(first.end, Date.parse('2026-10-26T23:00:00Z'));
        assert.equal(first.start, forcing.globalForcingWindow().start);
    } finally { globalThis.fetch = original; }
});

test('older deployments fall back only when the manifest is absent', async () => {
    const original = globalThis.fetch;
    try {
        globalThis.fetch = async () => new Response(null, { status: 404 });
        assert.deepEqual(await forcing.loadForcingWindow('global'), forcing.globalForcingWindow());
        globalThis.fetch = async () => new Response(null, { status: 503 });
        await assert.rejects(forcing.loadForcingWindow('global'), /503/);
        globalThis.fetch = async () => new Response(JSON.stringify({ days: [{ timestamps: [0, 7200] }] }));
        await assert.rejects(forcing.loadForcingWindow('global'), /invalid timestamps/);
    } finally { globalThis.fetch = original; }
});

test('NorKyst availability still rejects gaps between days', () => {
    assert.throws(() => forcing.norkystForcingWindow({ days: [
        { timestamps: timestamps('2026-10-25T00:00:00Z', 25) },
        { timestamps: timestamps('2026-10-27T00:00:00Z', 24) },
    ] }), /gap/);
});
