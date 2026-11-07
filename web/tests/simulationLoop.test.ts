// simulationLoop.test.ts checks animation frames, rendering, and asynchronous run controls

import assert from "node:assert/strict";
import test from "node:test";
import { SimulationLoop } from "../src/lib/simulationLoop.ts";

function deferred() {
    let resolve!: () => void;
    const promise = new Promise<void>(r => { resolve = r; });
    return { promise,resolve };
}

test("each physics step renders and waits for an animation frame before advancing", async () => {
    let steps=0,yields=0,completed=0;
    const draws: number[]=[];
    const loop=new SimulationLoop({
        advance:async()=>{assert.equal(yields,steps);return ++steps===100;},
        isCurrent:()=>true,
        render:()=>draws.push(steps),
        complete:()=>completed++,
        error:error=>{throw error;},
        yieldTask:async()=>{yields++;},
    });
    await loop.start();
    assert.equal(steps,100);
    assert.equal(completed,1);
    assert.equal(yields,99);
    assert.equal(draws.length,100);
    assert.equal(draws.at(-1),100);
});

test("pause and resume during a pending step never overlap or duplicate it", async () => {
    const gate=deferred();
    let steps=0,active=0,peak=0,completed=0;
    const loop=new SimulationLoop({
        advance:async()=>{active++;peak=Math.max(peak,active);steps++;if(steps===1)await gate.promise;active--;return steps===3;},
        isCurrent:()=>true,render:()=>{},complete:()=>completed++,error:error=>{throw error;},yieldTask:async()=>{},
    });
    const pending=loop.start();
    loop.pause();
    assert.equal(loop.start(),pending);
    gate.resolve();await pending;
    assert.equal(steps,3);assert.equal(peak,1);assert.equal(completed,1);
});

test("a paused run finishes its pending step and resumes at the following step", async () => {
    const gate=deferred();let steps=0,completed=0;
    const loop=new SimulationLoop({
        advance:async()=>{steps++;if(steps===1)await gate.promise;return steps===2;},
        isCurrent:()=>true,render:()=>{},complete:()=>completed++,error:error=>{throw error;},yieldTask:async()=>{},
    });
    const pending=loop.start();loop.pause();gate.resolve();await pending;
    assert.equal(steps,1);assert.equal(completed,0);
    await loop.start();assert.equal(steps,2);assert.equal(completed,1);
});

test("cancelled or replaced runs cannot render or complete after their pending step", async () => {
    for(const cancel of [false,true]){
        const gate=deferred();let current=true,draws=0,completed=0;
        const loop=new SimulationLoop({
            advance:async()=>{await gate.promise;return true;},isCurrent:()=>current,
            render:()=>draws++,complete:()=>completed++,error:error=>{throw error;},
        });
        const pending=loop.start();
        if(cancel)loop.cancel();else current=false;
        gate.resolve();await pending;
        assert.equal(draws,0);assert.equal(completed,0);
    }
});

test("steps wait for frames and errors stop the loop", async () => {
    let steps=0,yields=0,errors=0;
    const loop=new SimulationLoop({
        advance:async()=>{if(++steps===3)throw new Error("failed");return false;},
        isCurrent:()=>true,render:()=>{},complete:()=>assert.fail("unexpected completion"),
        error:error=>{assert.match(String(error),/failed/);errors++;},
        yieldTask:async()=>{yields++;},
    });
    await loop.start();assert.equal(steps,3);assert.equal(yields,2);assert.equal(errors,1);
});

test("cancelling a queued animation frame resolves the task", async (t) => {
    let steps=0;
    const cancelled: number[]=[];
    const originalRequest = Object.getOwnPropertyDescriptor(globalThis, "requestAnimationFrame");
    const originalCancel = Object.getOwnPropertyDescriptor(globalThis, "cancelAnimationFrame");
    Object.defineProperty(globalThis, "requestAnimationFrame", { configurable: true, value: () => 42 });
    Object.defineProperty(globalThis, "cancelAnimationFrame", { configurable: true, value: (id: number) => cancelled.push(id) });
    t.after(() => {
        for (const [key, descriptor] of [["requestAnimationFrame", originalRequest], ["cancelAnimationFrame", originalCancel]] as const) {
            if (descriptor) Object.defineProperty(globalThis, key, descriptor);
            else Reflect.deleteProperty(globalThis, key);
        }
    });
    const loop=new SimulationLoop({
        advance:async()=>{steps++;return false;},isCurrent:()=>true,
        render:()=>{},complete:()=>assert.fail("unexpected completion"),error:error=>{throw error;},
    });
    const pending=loop.start();
    await Promise.resolve();await Promise.resolve();
    loop.cancel();await pending;
    assert.equal(steps,1);
    assert.deepEqual(cancelled,[42]);
});
