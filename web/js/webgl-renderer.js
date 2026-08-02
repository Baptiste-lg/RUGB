/**
 * WebGL renderer for RUGB — renders emulator framebuffer with GPU-based
 * shader filters (CRT, LCD, smooth, ghost) and optional palette lookup.
 * Falls back to Canvas 2D if WebGL is unavailable.
 */

/* ── shared vertex shader ─────────────────────────────────────────── */

const VERT = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    v_uv.y = 1.0 - v_uv.y;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

/* ── fragment shaders ─────────────────────────────────────────────── */

const FRAG_NONE = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_frame;
uniform sampler2D u_prev;
uniform sampler2D u_palette;
uniform float u_blend;
uniform float u_use_palette;
void main() {
    vec4 c = texture2D(u_frame, v_uv);
    if (u_use_palette > 0.5) {
        c = texture2D(u_palette, vec2(c.r, 0.5));
    }
    vec4 p = texture2D(u_prev, v_uv);
    gl_FragColor = mix(c, p, u_blend);
}`;

const FRAG_CRT = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_frame;
uniform sampler2D u_prev;
uniform sampler2D u_palette;
uniform float u_blend;
uniform float u_use_palette;
uniform vec2 u_resolution;
uniform vec2 u_output_size;

vec2 barrel(vec2 uv, float k) {
    vec2 d = uv - 0.5;
    float r2 = dot(d, d);
    return uv + d * r2 * k;
}

void main() {
    vec2 uv = barrel(v_uv, 0.15);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec4 c = texture2D(u_frame, uv);
    if (u_use_palette > 0.5) {
        c = texture2D(u_palette, vec2(c.r, 0.5));
    }
    vec4 p = texture2D(u_prev, uv);
    c = mix(c, p, u_blend);

    // scanlines
    float scanline = sin(uv.y * u_output_size.y * 3.14159265) * 0.5 + 0.5;
    scanline = mix(1.0, scanline, 0.25);
    c.rgb *= scanline;

    // color bleed — slight horizontal offset per channel
    float bleed = 0.5 / u_output_size.x;
    vec4 left = texture2D(u_frame, uv + vec2(-bleed, 0.0));
    vec4 right = texture2D(u_frame, uv + vec2(bleed, 0.0));
    if (u_use_palette > 0.5) {
        left = texture2D(u_palette, vec2(left.r, 0.5));
        right = texture2D(u_palette, vec2(right.r, 0.5));
    }
    c.r = mix(c.r, left.r, 0.08);
    c.b = mix(c.b, right.b, 0.08);

    // vignette
    float vig = 1.0 - smoothstep(0.4, 0.75, length(uv - 0.5));
    c.rgb *= vig;

    // subtle phosphor glow
    c.rgb *= vec3(1.05, 1.0, 0.95);

    gl_FragColor = c;
}`;

const FRAG_LCD = `
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_frame;
uniform sampler2D u_prev;
uniform sampler2D u_palette;
uniform float u_blend;
uniform float u_use_palette;
uniform vec2 u_output_size;

void main() {
    vec4 c = texture2D(u_frame, v_uv);
    if (u_use_palette > 0.5) {
        c = texture2D(u_palette, vec2(c.r, 0.5));
    }
    vec4 p = texture2D(u_prev, v_uv);
    c = mix(c, p, u_blend);

    // LCD dot matrix grid
    vec2 pixel = v_uv * u_output_size;
    vec2 cell = mod(pixel, 3.0);

    // RGB sub-pixel dots within each 3px cell
    float mask = 0.75;
    if (cell.x < 1.0)      mask = mix(0.85, 1.0, c.r);
    else if (cell.x < 2.0) mask = mix(0.85, 1.0, c.g);
    else                    mask = mix(0.85, 1.0, c.b);

    // dark gap between cells
    float gap = smoothstep(0.0, 0.4, cell.x) * smoothstep(0.0, 0.4, cell.y)
              * smoothstep(3.0, 2.6, cell.x) * smoothstep(3.0, 2.6, cell.y);
    mask *= mix(0.7, 1.0, gap);

    c.rgb *= mask;
    gl_FragColor = c;
}`;

/* smooth uses the same passthrough as 'none' but with LINEAR texture filtering */
const FRAG_SMOOTH = FRAG_NONE;

const FRAG_GHOST = FRAG_NONE; // ghost = 'none' shader + blend > 0

/* ── shader source map ────────────────────────────────────────────── */

const SHADERS = {
    none:     FRAG_NONE,
    crt:      FRAG_CRT,
    lcd:      FRAG_LCD,
    smooth:   FRAG_SMOOTH,
    ghost:    FRAG_GHOST,
};

/* ── renderer class ───────────────────────────────────────────────── */

export class WebGLRenderer {
    constructor(canvas) {
        this.canvas = canvas;
        this.gl = canvas.getContext('webgl', {
            alpha: false,
            antialias: false,
            preserveDrawingBuffer: true, // needed for screenshots
        });
        this.ready = false;
        this.blendAmount = 0.0;
        this.activeFilter = 'none';
        this.programs = {};
        this.uniforms = {};
        this.usePalette = false;
        this.width = 0;
        this.height = 0;

        if (!this.gl) return;
        const gl = this.gl;

        // compile shared vertex shader
        this._vs = this._compileShader(gl.VERTEX_SHADER, VERT);
        if (!this._vs) return;

        // compile all filter programs
        for (const [name, frag] of Object.entries(SHADERS)) {
            const prog = this._buildProgram(frag);
            if (!prog) return;
            this.programs[name] = prog;
            this.uniforms[name] = this._getUniforms(prog);
        }

        // full-screen quad (shared across programs)
        this._quadBuf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this._quadBuf);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, 1,1]), gl.STATIC_DRAW);

        // textures: unit 0 = frame, unit 1 = prev, unit 2 = palette
        this.frameTex = this._createTexture(0, gl.NEAREST);
        this.prevTex  = this._createTexture(1, gl.NEAREST);
        this.paletteTex = this._createTexture(2, gl.NEAREST);

        // activate default program
        this._useProgram('none');

        // context loss handling
        canvas.addEventListener('webglcontextlost', (e) => {
            e.preventDefault();
            this.ready = false;
        });
        canvas.addEventListener('webglcontextrestored', () => {
            this._restore();
        });

        this.ready = true;
    }

    /* ── shader compilation helpers ───────────────────────────────── */

    _compileShader(type, source) {
        const gl = this.gl;
        const s = gl.createShader(type);
        gl.shaderSource(s, source);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
            gl.deleteShader(s);
            return null;
        }
        return s;
    }

    _buildProgram(fragSrc) {
        const gl = this.gl;
        const fs = this._compileShader(gl.FRAGMENT_SHADER, fragSrc);
        if (!fs) return null;
        const prog = gl.createProgram();
        gl.attachShader(prog, this._vs);
        gl.attachShader(prog, fs);
        gl.linkProgram(prog);
        if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
        return prog;
    }

    _getUniforms(prog) {
        const gl = this.gl;
        return {
            uFrame:      gl.getUniformLocation(prog, 'u_frame'),
            uPrev:       gl.getUniformLocation(prog, 'u_prev'),
            uPalette:    gl.getUniformLocation(prog, 'u_palette'),
            uBlend:      gl.getUniformLocation(prog, 'u_blend'),
            uUsePalette: gl.getUniformLocation(prog, 'u_use_palette'),
            uResolution: gl.getUniformLocation(prog, 'u_resolution'),
            uOutputSize: gl.getUniformLocation(prog, 'u_output_size'),
            aPos:        gl.getAttribLocation(prog, 'a_pos'),
        };
    }

    _useProgram(name) {
        const gl = this.gl;
        const prog = this.programs[name];
        const u = this.uniforms[name];
        gl.useProgram(prog);

        // bind quad
        gl.bindBuffer(gl.ARRAY_BUFFER, this._quadBuf);
        gl.enableVertexAttribArray(u.aPos);
        gl.vertexAttribPointer(u.aPos, 2, gl.FLOAT, false, 0, 0);

        // bind texture units
        gl.uniform1i(u.uFrame, 0);
        gl.uniform1i(u.uPrev, 1);
        gl.uniform1i(u.uPalette, 2);

        this._currentUniforms = u;
        this._currentProgram = name;
    }

    /* ── texture helpers ──────────────────────────────────────────── */

    _createTexture(unit, filter) {
        const gl = this.gl;
        gl.activeTexture(gl.TEXTURE0 + unit);
        const tex = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        return tex;
    }

    /* ── public API ───────────────────────────────────────────────── */

    setFilter(name) {
        if (!this.ready || !this.programs[name]) return;
        this.activeFilter = name;
        this._useProgram(name);

        // smooth filter uses LINEAR texture mag, others use NEAREST
        const gl = this.gl;
        const mag = name === 'smooth' ? gl.LINEAR : gl.NEAREST;
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.frameTex);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, mag);

        // force re-upload resolution uniforms on next render
        this.width = 0;
    }

    setBlend(amount) {
        this.blendAmount = amount;
    }

    setPalette(lut) {
        if (!this.ready) return;
        const gl = this.gl;
        gl.activeTexture(gl.TEXTURE2);
        gl.bindTexture(gl.TEXTURE_2D, this.paletteTex);
        if (lut) {
            // lut is a Uint8Array(256*3) — expand to 256x1 RGBA
            const rgba = new Uint8Array(256 * 4);
            for (let i = 0; i < 256; i++) {
                rgba[i * 4]     = lut[i * 3];
                rgba[i * 4 + 1] = lut[i * 3 + 1];
                rgba[i * 4 + 2] = lut[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 256, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, rgba);
            this.usePalette = true;
        } else {
            this.usePalette = false;
        }
    }

    render(pixels, w, h) {
        if (!this.ready) return false;
        const gl = this.gl;
        const u = this._currentUniforms;

        if (w !== this.width || h !== this.height) {
            this.width = w;
            this.height = h;
            gl.viewport(0, 0, this.canvas.width, this.canvas.height);

            // allocate prev and frame textures at the new resolution
            gl.activeTexture(gl.TEXTURE1);
            gl.bindTexture(gl.TEXTURE_2D, this.prevTex);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, this.frameTex);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);

            if (u.uResolution) gl.uniform2f(u.uResolution, w, h);
            if (u.uOutputSize) gl.uniform2f(u.uOutputSize, this.canvas.width, this.canvas.height);
        }

        // copy current rendered frame to prev texture for next frame blending
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, this.prevTex);
        gl.copyTexSubImage2D(gl.TEXTURE_2D, 0, 0, 0, 0, 0, this.canvas.width, this.canvas.height);

        // upload new frame (sub-image update, no reallocation)
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.frameTex);
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, pixels);

        // set per-frame uniforms (only update if changed)
        const paletteVal = this.usePalette ? 1.0 : 0.0;
        if (this._lastBlend !== this.blendAmount) {
            gl.uniform1f(u.uBlend, this.blendAmount);
            this._lastBlend = this.blendAmount;
        }
        if (this._lastPalette !== paletteVal) {
            gl.uniform1f(u.uUsePalette, paletteVal);
            this._lastPalette = paletteVal;
        }

        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        return true;
    }

    destroy() {
        if (!this.gl) return;
        const gl = this.gl;
        for (const prog of Object.values(this.programs)) {
            gl.deleteProgram(prog);
        }
        if (this._vs) gl.deleteShader(this._vs);
        if (this.frameTex) gl.deleteTexture(this.frameTex);
        if (this.prevTex) gl.deleteTexture(this.prevTex);
        if (this.paletteTex) gl.deleteTexture(this.paletteTex);
        if (this._quadBuf) gl.deleteBuffer(this._quadBuf);
        this.ready = false;
        this.gl = null;
    }

    _restore() {
        // re-init after context loss — reconstruct everything
        const canvas = this.canvas;
        this.gl = canvas.getContext('webgl', {
            alpha: false, antialias: false, preserveDrawingBuffer: true,
        });
        if (!this.gl) return;
        const gl = this.gl;

        this._vs = this._compileShader(gl.VERTEX_SHADER, VERT);
        if (!this._vs) return;
        this.programs = {};
        this.uniforms = {};
        for (const [name, frag] of Object.entries(SHADERS)) {
            const prog = this._buildProgram(frag);
            if (!prog) return;
            this.programs[name] = prog;
            this.uniforms[name] = this._getUniforms(prog);
        }
        this._quadBuf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, this._quadBuf);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, 1,1]), gl.STATIC_DRAW);
        this.frameTex = this._createTexture(0, gl.NEAREST);
        this.prevTex  = this._createTexture(1, gl.NEAREST);
        this.paletteTex = this._createTexture(2, gl.NEAREST);
        this.width = 0;
        this.height = 0;
        this._useProgram(this.activeFilter);
        this.ready = true;
    }
}
