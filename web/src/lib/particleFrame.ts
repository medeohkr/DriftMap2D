// particleFrame.ts shares owned position arrays and statistics across consumers of a step

import type { Stats } from "./stores/stats.svelte";

interface RawFrame {
    activePositions: Float32Array;
    strandedPositions: Float32Array;
    boundingBox: Float32Array;
    /** active count, stranded percent, evaporation percent, emulsification percent, floating tons */
    summary: Float64Array;
}

interface FrameSource {
    current_day(): number;
    get_particle_frame(): RawFrame;
}

export interface ParticleFrame {
    activePositions: Float32Array;
    strandedPositions: Float32Array;
    boundingBox: Float32Array;
    stats: Stats;
}

export class ParticleFrameCache {
    private cached: { source: FrameSource; day: number; frame: ParticleFrame } | null = null;

    get(source: FrameSource): ParticleFrame {
        const day = source.current_day();
        if (this.cached?.source === source && this.cached.day === day) return this.cached.frame;
        const raw = source.get_particle_frame();
        const frame = {
            activePositions: raw.activePositions,
            strandedPositions: raw.strandedPositions,
            boundingBox: raw.boundingBox,
            stats: {
                activeParticles: String(raw.summary[0]),
                stranded: raw.summary[1].toFixed(1),
                evaporated: raw.summary[2].toFixed(1),
                emulsified: raw.summary[3].toFixed(1),
                totalMass: raw.summary[4].toFixed(1),
            },
        };
        this.cached = { source,day,frame };
        return frame;
    }

    clear() { this.cached = null; }

    peek(source: FrameSource) {
        return this.cached?.source === source ? this.cached.frame : null;
    }
}
