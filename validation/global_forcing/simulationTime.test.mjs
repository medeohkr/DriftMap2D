import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';

const component = await readFile(new URL('../../web/src/components/config/SimulationTime.svelte', import.meta.url), 'utf8');
const script = stripTypeScriptTypes(component.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1].replace(/^\s*import .*;$/gm, ''));
const windows = {
    global: { start: Date.parse('2026-10-18T00:00Z'), end: Date.parse('2026-11-03T23:00Z') },
    norkyst: { start: Date.parse('2026-10-20T00:00Z'), end: Date.parse('2026-10-28T23:00Z') },
};

// exercise the component's asynchronous interval logic without rendering the date inputs
function fixture(load = model => Promise.resolve(windows[model])) {
    const config = { forcingModel: 'global', startDate: '2026-10-25', startTime: '00:00', endDate: '2026-10-30', endTime: '00:00' };
    const simulation = { simulationActive: false };
    const state = { version: 0 };
    const messages = [];
    let effect, cleanup;
    new Function('config', 'simulation', 'simulationTimeState', 'toast', 'showToast', 'loadForcingWindow',
        'utcInputTimestamp', '$state', '$derived', '$effect', script)(
        config, simulation, state, { visible: false, message: '' }, message => messages.push(message), load,
        (date, time) => Date.parse(`${date}T${time}:00Z`), value => value, value => value, callback => { effect = callback; });
    return {
        config, simulation, state, messages,
        run() { cleanup?.(); cleanup = effect(); },
        async switchTo(model) { config.forcingModel = model; this.run(); await Promise.resolve(); },
    };
}

test('one shared interval stays clipped when switching back to global', async () => {
    const f = fixture();
    await f.switchTo('global');
    assert.equal(f.config.endDate, '2026-10-30');
    assert.equal(f.messages.length, 0);
    await f.switchTo('norkyst');
    assert.equal(f.config.endDate, '2026-10-28');
    assert.equal(f.config.endTime, '23:00');
    assert.deepEqual(f.messages, ['Simulation interval adjusted to fit NorKyst availability']);
    await f.switchTo('global');
    assert.equal(f.config.endDate, '2026-10-28');
    assert.equal(f.config.endTime, '23:00');
    assert.equal(f.messages.length, 1);
});

test('imports and resets clip both endpoints even when the model stays unchanged', async () => {
    const f = fixture();
    f.config.startDate = '2026-10-01';
    f.config.endDate = '2026-11-30';
    f.state.version++;
    await f.switchTo('global');
    assert.equal(f.config.startDate, '2026-10-18');
    assert.equal(f.config.endDate, '2026-11-03');
    assert.equal(f.config.endTime, '23:00');
    assert.match(f.messages[0], /CMEMS/);
    await f.switchTo('norkyst');
    assert.equal(f.config.startDate, '2026-10-20');
    assert.equal(f.config.endDate, '2026-10-28');
    f.config.startDate = '2026-10-25';
    f.config.endDate = '2026-10-27';
    f.config.endTime = '00:00';
    f.state.version++;
    await f.switchTo('norkyst');
    assert.equal(f.config.endDate, '2026-10-27');
    assert.equal(f.messages.length, 2);
});

test('stale availability responses and active simulations cannot change the dates', async () => {
    const pending = [];
    const f = fixture(model => new Promise(resolve => pending.push(() => resolve(windows[model]))));
    f.config.forcingModel = 'norkyst';
    f.run();
    f.config.forcingModel = 'global';
    f.run();
    pending[0]();
    await Promise.resolve();
    assert.equal(f.config.endDate, '2026-10-30');
    pending[1]();
    await Promise.resolve();
    assert.equal(f.messages.length, 0);
    const active = fixture();
    active.simulation.simulationActive = true;
    await active.switchTo('norkyst');
    assert.equal(active.config.endDate, '2026-10-30');
    assert.equal(active.messages.length, 0);
});

test('the default marker moves to Norway once and stays there on global', async () => {
    const releases = { releases: [{ lat: 48.4, lon: -125 }] };
    const source = await readFile(new URL('../../web/src/lib/utils/releases.ts', import.meta.url), 'utf8');
    const code = stripTypeScriptTypes(source.replace(/^import .*;$/gm, ''));
    const getUpdate = new Function('releaseConfig', 'config', code.replaceAll('export ', '') + '\nreturn updateDefaultReleaseLocation;');
    const update = getUpdate(releases, {});
    assert.equal(update('norkyst'), true);
    assert.deepEqual(releases.releases[0], { lat: 60, lon: 4.5 });
    assert.equal(update('global'), false);
    assert.deepEqual(releases.releases[0], { lat: 60, lon: 4.5 });
    assert.equal(update('norkyst'), false);
    releases.releases[0] = { lat: 61, lon: 5 };
    assert.equal(update('norkyst'), false);
    releases.releases = [{ lat: 48.4, lon: -125 }, { lat: 61, lon: 5 }];
    assert.equal(update('norkyst'), false);
});
