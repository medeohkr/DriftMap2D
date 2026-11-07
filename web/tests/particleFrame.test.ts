// particleFrame.test.ts checks shared frame exports and snapshot ownership

import assert from "node:assert/strict";
import test from "node:test";
import { ParticleFrameCache } from "../src/lib/particleFrame.ts";

function source(){
    return {day:0,calls:0,current_day(){return this.day;},get_particle_frame(){
        this.calls++;
        return {activePositions:new Float32Array([this.day,60]),strandedPositions:new Float32Array([5,61]),
            boundingBox:new Float32Array([this.day,this.day,60,60]),summary:new Float64Array([1,50,10,20,2.5])};
    }};
}

test("statistics, snapshots, and rendering share one export per step",()=>{
    const cache=new ParticleFrameCache(),engine=source();
    const saved=cache.get(engine),rendered=cache.get(engine);
    assert.equal(saved,rendered);assert.equal(engine.calls,1);
    assert.deepEqual(saved.stats,{activeParticles:"1",stranded:"50.0",evaporated:"10.0",emulsified:"20.0",totalMass:"2.5"});
    engine.day=1;const next=cache.get(engine);
    assert.notEqual(next.activePositions,saved.activePositions);
    assert.deepEqual([...saved.activePositions],[0,60]);assert.equal(engine.calls,2);
});

test("new engines and resets invalidate frames and peeking never calls WASM",()=>{
    const cache=new ParticleFrameCache(),engine=source(),next=source();
    assert.equal(cache.peek(engine),null);
    const frame=cache.get(engine);engine.day=1;
    assert.equal(cache.peek(engine),frame);assert.equal(engine.calls,1);
    cache.get(next);assert.equal(cache.peek(engine),null);
    cache.clear();assert.equal(cache.peek(next),null);
    cache.get(next);assert.equal(next.calls,2);
});
