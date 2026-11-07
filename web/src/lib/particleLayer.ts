// particleLayer.ts draws particle arrays in the map's WebGL context

const EMPTY = new Float32Array();
const MAX_LATITUDE = 85.0511287798066;

interface ParticleMap {
    triggerRepaint(): void;
    getBounds(): { getWest(): number; getEast(): number };
    getZoom(): number;
    getPitch(): number;
    getRenderWorldCopies(): boolean;
    getCanvas(): HTMLCanvasElement;
    getLayer(id: string): unknown;
    isStyleLoaded(): boolean;
    addLayer(layer: ParticleLayer): void;
    on(event: string, listener: () => void): void;
    off(event: string, listener: () => void): void;
}

interface RenderInput {
    defaultProjectionData: { mainMatrix: ArrayLike<number> };
}

const VERTEX_SHADER = `#version 300 es
precision highp float;
layout(location = 0) in vec2 a_corner;
layout(location = 1) in vec2 a_position;
uniform mat4 u_matrix;
uniform vec2 u_pixel_scale;
uniform float u_radius;
uniform float u_antialias;
uniform vec2 u_world_limits;
out vec2 v_offset;
out float v_world_x;
void main() {
    v_offset = a_corner * (u_radius + u_antialias);
    v_world_x = a_position.x;
    gl_Position = u_matrix * vec4(a_position, 0.0, 1.0);
    gl_Position.xy += v_offset * u_pixel_scale * gl_Position.w;
}`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;
uniform vec3 u_color;
uniform float u_radius;
uniform float u_antialias;
uniform vec2 u_world_limits;
in vec2 v_offset;
in float v_world_x;
out vec4 frag_color;
void main() {
    if (v_world_x < u_world_limits.x || v_world_x > u_world_limits.y) discard;
    float alpha = 0.7 * (1.0 - smoothstep(
        max(0.0, u_radius - u_antialias), u_radius + u_antialias, length(v_offset)));
    frag_color = vec4(u_color * alpha, alpha);
}`;

/** project coordinates relative to a nearby origin to preserve precision at high zoom */
export function projectParticles(positions: Float32Array, output: Float32Array) {
    let originX = 0;
    let originY = 0;
    let minX = Infinity;
    let maxX = -Infinity;
    let count = 0;
    for (let i = 0; i + 1 < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        if (!Number.isFinite(lon) || !Number.isFinite(lat)) continue;
        let x = (lon + 180) / 360;
        x -= Math.floor(x);
        const latitude = Math.max(-MAX_LATITUDE, Math.min(MAX_LATITUDE, lat));
        const y = (1 - Math.log(Math.tan(Math.PI / 4 + latitude * Math.PI / 360)) / Math.PI) / 2;
        if (count === 0) {
            originX = x;
            originY = y;
        }
        // keep particles on either side of the antimeridian together
        x -= Math.round(x - originX);
        output[count * 2] = x - originX;
        output[count * 2 + 1] = y - originY;
        minX = Math.min(minX, x);
        maxX = Math.max(maxX, x);
        count++;
    }
    return { originX, originY, minX, maxX, count };
}

class ParticleBatch {
    positions: Float32Array = EMPTY;
    projected = EMPTY;
    bounds = { originX: 0, originY: 0, minX: Infinity, maxX: -Infinity, count: 0 };
    buffer: WebGLBuffer | null = null;
    vao: WebGLVertexArrayObject | null = null;
    capacity = 0;
    dirty = true;

    /** retain only the latest update until MapLibre renders it */
    setPositions(positions: Float32Array) {
        if (this.positions === positions) return;
        this.positions = positions;
        this.dirty = true;
    }

    /** reuse projection storage and grow the GPU buffer only when needed */
    upload(gl: WebGL2RenderingContext) {
        if (!this.dirty) return;
        if (this.projected.length < this.positions.length) {
            this.projected = new Float32Array(2 ** Math.ceil(Math.log2(this.positions.length)));
        }
        this.bounds = projectParticles(this.positions, this.projected);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
        if (this.capacity < this.projected.byteLength) {
            this.capacity = this.projected.byteLength;
            gl.bufferData(gl.ARRAY_BUFFER, this.capacity, gl.DYNAMIC_DRAW);
        }
        if (this.bounds.count) {
            gl.bufferSubData(gl.ARRAY_BUFFER, 0, this.projected, 0, this.bounds.count * 2);
        }
        this.dirty = false;
    }
}

/** render active and stranded particles without GeoJSON or source workers */
export class ParticleLayer {
    readonly id = "particles";
    readonly type = "custom";
    readonly renderingMode = "2d";
    private map?: ParticleMap;
    private program: WebGLProgram | null = null;
    private corners: WebGLBuffer | null = null;
    private uniforms: Record<string, WebGLUniformLocation | null> = {};
    private batches = [new ParticleBatch(), new ParticleBatch()];
    private matrix = new Float32Array(16);
    private visible = true;
    private radius = 1.5;
    private recovering = false;

    private contextLost = () => {
        this.recovering = true;
        this.program = null;
        this.corners = null;
        for (const batch of this.batches) {
            batch.buffer = null;
            batch.vao = null;
            batch.capacity = 0;
            batch.dirty = true;
        }
    };
    private styleLoaded = () => {
        // MapLibre restores its serialized style without custom layers
        if (this.recovering && this.map && !this.map.getLayer(this.id)) this.map.addLayer(this);
    };
    private contextRestored = () => {
        if (this.map?.isStyleLoaded()) this.styleLoaded();
        this.map?.triggerRepaint();
    };
    private mapRemoved = () => this.detach();

    /** attach resources and recover them after a WebGL context loss */
    onAdd(map: ParticleMap, gl: WebGL2RenderingContext) {
        this.removeListeners();
        this.map = map;
        this.recovering = false;
        this.createResources(gl);
        map.on("webglcontextlost", this.contextLost);
        map.on("webglcontextrestored", this.contextRestored);
        map.on("style.load", this.styleLoaded);
        map.on("remove", this.mapRemoved);
    }

    /** accept immutable position arrays from the engine or a playback snapshot */
    setPositions(active: Float32Array, stranded: Float32Array) {
        this.batches[0].setPositions(active);
        this.batches[1].setPositions(stranded);
        this.map?.triggerRepaint();
    }

    setVisible(visible: boolean) {
        this.visible = visible;
        this.map?.triggerRepaint();
    }

    setRadius(radius: number) {
        this.radius = Number.isFinite(radius) ? Math.max(0, radius) : 0;
        this.map?.triggerRepaint();
    }

    clear() {
        this.setPositions(EMPTY, EMPTY);
    }

    /** draw instanced circles in each visible world copy using MapLibre's camera */
    render(gl: WebGL2RenderingContext, input: RenderInput) {
        const map = this.map;
        if (!map || !this.visible || this.radius <= 0 || gl.isContextLost()) return;
        if (!this.program) this.createResources(gl);
        const canvas = map.getCanvas();
        const pixelRatio = gl.drawingBufferWidth / Math.max(1, canvas.clientWidth);
        gl.useProgram(this.program);
        gl.uniform2f(this.uniforms.u_pixel_scale,
            2 * pixelRatio / gl.drawingBufferWidth, 2 * pixelRatio / gl.drawingBufferHeight);
        gl.uniform1f(this.uniforms.u_radius, this.radius);
        gl.uniform1f(this.uniforms.u_antialias, 1 / pixelRatio);
        gl.enable(gl.BLEND);
        gl.blendEquation(gl.FUNC_ADD);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

        const bounds = map.getBounds();
        const padding = (this.radius + 1) / (512 * 2 ** map.getZoom()) /
            Math.max(0.01, Math.cos(map.getPitch() * Math.PI / 180));
        const west = (bounds.getWest() + 180) / 360 - padding;
        const east = (bounds.getEast() + 180) / 360 + padding;
        const repeats = map.getRenderWorldCopies();
        const source = input.defaultProjectionData.mainMatrix;
        for (let index = 0; index < this.batches.length; index++) {
            const batch = this.batches[index];
            batch.upload(gl);
            const { originX, originY, minX, maxX, count } = batch.bounds;
            if (!count) continue;
            gl.bindVertexArray(batch.vao);
            gl.uniform3f(this.uniforms.u_color, 1, index ? 59 / 255 : 1, index ? 20 / 255 : 1);
            const first = Math.ceil((repeats ? west : Math.max(0, west)) - maxX);
            const last = Math.floor((repeats ? east : Math.min(1, east)) - minX);
            for (let wrap = first; wrap <= last; wrap++) {
                // translate in double precision before handing the matrix to the GPU
                const x = originX + wrap;
                for (let i = 0; i < 12; i++) this.matrix[i] = source[i];
                for (let i = 0; i < 4; i++) {
                    this.matrix[12 + i] = source[i] * x + source[4 + i] * originY + source[12 + i];
                }
                gl.uniformMatrix4fv(this.uniforms.u_matrix, false, this.matrix);
                gl.uniform2f(this.uniforms.u_world_limits, repeats ? -2 : -x, repeats ? 2 : 1 - x);
                gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
            }
        }
        gl.bindVertexArray(null);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
    }

    /** release GPU resources and listeners when the map removes the layer */
    onRemove(map: ParticleMap, gl: WebGL2RenderingContext) {
        if (gl.isContextLost()) {
            this.contextLost();
            return;
        }
        gl.deleteProgram(this.program);
        gl.deleteBuffer(this.corners);
        for (const batch of this.batches) {
            gl.deleteBuffer(batch.buffer);
            gl.deleteVertexArray(batch.vao);
        }
        this.contextLost();
        this.detach();
    }

    private removeListeners() {
        this.map?.off("webglcontextlost", this.contextLost);
        this.map?.off("webglcontextrestored", this.contextRestored);
        this.map?.off("style.load", this.styleLoaded);
        this.map?.off("remove", this.mapRemoved);
    }

    private detach() {
        this.removeListeners();
        this.batches = [new ParticleBatch(), new ParticleBatch()];
        this.recovering = false;
        this.map = undefined;
    }

    private createResources(gl: WebGL2RenderingContext) {
        const vertex = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
        const fragment = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
        const program = gl.createProgram();
        if (!program) throw new Error("Could not create particle shader program");
        gl.attachShader(program, vertex);
        gl.attachShader(program, fragment);
        gl.linkProgram(program);
        gl.deleteShader(vertex);
        gl.deleteShader(fragment);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
            const message = gl.getProgramInfoLog(program);
            gl.deleteProgram(program);
            throw new Error(`Could not link particle shaders ${message}`);
        }
        this.program = program;
        for (const name of ["u_matrix", "u_pixel_scale", "u_radius", "u_antialias", "u_world_limits", "u_color"]) {
            this.uniforms[name] = gl.getUniformLocation(program, name);
        }
        this.corners = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this.corners);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
        for (const batch of this.batches) {
            batch.buffer = gl.createBuffer();
            batch.vao = gl.createVertexArray();
            gl.bindVertexArray(batch.vao);
            gl.bindBuffer(gl.ARRAY_BUFFER, this.corners);
            gl.enableVertexAttribArray(0);
            gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
            gl.bindBuffer(gl.ARRAY_BUFFER, batch.buffer);
            gl.enableVertexAttribArray(1);
            gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 0, 0);
            gl.vertexAttribDivisor(1, 1);
            batch.dirty = true;
        }
        gl.bindVertexArray(null);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
    }
}

function compileShader(gl: WebGL2RenderingContext, type: number, source: string) {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("Could not create particle shader");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        const message = gl.getShaderInfoLog(shader);
        gl.deleteShader(shader);
        throw new Error(`Could not compile particle shader ${message}`);
    }
    return shader;
}
