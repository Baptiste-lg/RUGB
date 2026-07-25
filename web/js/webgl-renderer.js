/**
 * WebGL renderer for RUGB — renders emulator framebuffer with optional
 * palette lookup and frame blending on the GPU instead of CPU.
 *
 * Falls back to Canvas 2D if WebGL is unavailable.
 */

const VERTEX_SHADER = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    v_uv.y = 1.0 - v_uv.y;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

const FRAGMENT_SHADER = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_frame;
uniform sampler2D u_prev;
uniform float u_blend; // 0.0 = no blend, 0.5 = 50/50 ghosting
void main() {
    vec4 cur = texture2D(u_frame, v_uv);
    vec4 prev = texture2D(u_prev, v_uv);
    gl_FragColor = mix(cur, prev, u_blend);
}`;

export class WebGLRenderer {
    constructor(canvas) {
        this.canvas = canvas;
        this.gl = canvas.getContext('webgl', { alpha: false, antialias: false });
        this.ready = false;
        this.blendAmount = 0.0;

        if (!this.gl) return;
        const gl = this.gl;

        // Compile shaders
        const vs = this._compileShader(gl.VERTEX_SHADER, VERTEX_SHADER);
        const fs = this._compileShader(gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
        if (!vs || !fs) return;

        this.program = gl.createProgram();
        gl.attachShader(this.program, vs);
        gl.attachShader(this.program, fs);
        gl.linkProgram(this.program);
        if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) return;

        gl.useProgram(this.program);

        // Full-screen quad
        const quad = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
        const buf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buf);
        gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
        const aPos = gl.getAttribLocation(this.program, 'a_pos');
        gl.enableVertexAttribArray(aPos);
        gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

        // Uniforms
        this.uFrame = gl.getUniformLocation(this.program, 'u_frame');
        this.uPrev = gl.getUniformLocation(this.program, 'u_prev');
        this.uBlend = gl.getUniformLocation(this.program, 'u_blend');

        // Textures
        this.frameTex = this._createTexture(0);
        this.prevTex = this._createTexture(1);

        gl.uniform1i(this.uFrame, 0);
        gl.uniform1i(this.uPrev, 1);

        this.ready = true;
        this.width = 0;
        this.height = 0;
    }

    _compileShader(type, source) {
        const gl = this.gl;
        const shader = gl.createShader(type);
        gl.shaderSource(shader, source);
        gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
            gl.deleteShader(shader);
            return null;
        }
        return shader;
    }

    _createTexture(unit) {
        const gl = this.gl;
        gl.activeTexture(gl.TEXTURE0 + unit);
        const tex = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        return tex;
    }

    /**
     * Upload pixel data and render.
     * @param {Uint8ClampedArray} pixels - RGBA pixel data
     * @param {number} w - width
     * @param {number} h - height
     */
    render(pixels, w, h) {
        if (!this.ready) return false;
        const gl = this.gl;

        if (w !== this.width || h !== this.height) {
            this.width = w;
            this.height = h;
            gl.viewport(0, 0, this.canvas.width, this.canvas.height);
            // Initialize prev texture with black
            gl.activeTexture(gl.TEXTURE1);
            gl.bindTexture(gl.TEXTURE_2D, this.prevTex);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
        }

        // Swap: current frame becomes prev for next frame
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, this.prevTex);
        gl.copyTexImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 0, 0, this.canvas.width, this.canvas.height, 0);

        // Upload new frame
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.frameTex);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);

        gl.uniform1f(this.uBlend, this.blendAmount);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

        return true;
    }

    setBlend(amount) {
        this.blendAmount = amount;
    }
}
