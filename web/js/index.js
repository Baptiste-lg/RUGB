import initGb, { WasmEmulator } from '../pkg/rugb/rugb.js';
import initGba, { WasmGbaEmulator } from '../pkg/rugba/rugba.js';
import { LinkCable } from './link-cable.js';
import { DebugTools } from './debug-tools.js';
import { WebGLRenderer } from './webgl-renderer.js';
import { CloudSaves } from './cloud-saves.js';

function sanitizeTitle(raw) {
    return raw.replace(/[^\x20-\x7E]/g, '').slice(0, 32);
}

let emu = null;
let currentSystem = null; // 'gb' or 'gba'
let gbWasm = null;
let gbaWasm = null;
let animationId = null;
let paused = false;
let romBytes = null;
let wasm = null; // Active WASM module (gb or gba)
let speed = 1;
let muted = false;
let fastForward = false;
let turboA = false;
let frameBlending = false;
const prevFrameBuf = new Uint8ClampedArray(160 * 144 * 4); // Pre-allocated blend buffer
let showFps = false;
let fpsFrameCount = 0;
let fpsLastTime = 0;
let turboB = false;
let turboFrame = 0;

// --- Link cable ---
let linkCable = null;

// --- Debug tools ---
let debugTools = null;

// --- Rewind state ---
const REWIND_MAX_FRAMES = 300; // ~5 seconds at 60fps
const REWIND_CAPTURE_INTERVAL = 4; // Capture every 4 frames
let rewindBuffer = [];
let rewindFrameCounter = 0;
let rewinding = false;

// --- Boot ROM ---
let bootRomData = null;

// --- Web Worker mode ---
let worker = null;
let useWorker = typeof OffscreenCanvas !== 'undefined';

function emuSetButton(btn, pressed) {
    if (worker) {
        worker.postMessage({ cmd: 'input', button: btn, pressed });
    } else if (emu) {
        emu.set_button(btn, pressed);
    }
}

// Worker toggle
const workerToggle = document.getElementById('worker-toggle');
if (!('OffscreenCanvas' in self)) {
    useWorker = false;
    if (workerToggle) { workerToggle.checked = false; workerToggle.disabled = true; }
} else {
    const savedWorker = localStorage.getItem('rugb-use-worker');
    useWorker = savedWorker !== 'false';
    if (workerToggle) workerToggle.checked = useWorker;
}
if (workerToggle) {
    workerToggle.addEventListener('change', () => {
        useWorker = workerToggle.checked;
        localStorage.setItem('rugb-use-worker', useWorker);
    });
}

let canvas = document.getElementById('screen');
let ctx = canvas.getContext('2d');
let glRenderer = null;
const pauseBtn = document.getElementById('pause-btn');
const resetBtn = document.getElementById('reset-btn');
const muteBtn = document.getElementById('mute-btn');
const romInput = document.getElementById('rom-input');
const speedBtns = document.querySelectorAll('.speed-btn');
const paletteBtns = document.querySelectorAll('.palette-btn');
const menuToggle = document.getElementById('menu-toggle');
const sideMenu = document.getElementById('side-menu');
const viewGbBtn = document.getElementById('view-gb');
const viewScreenBtn = document.getElementById('view-screen');
const gameboy = document.querySelector('.gameboy');

// --- Side menu toggle is handled by dock.js ---

// --- View toggle (Game Boy / Screen Only) ---
const savedView = localStorage.getItem('rugb-view') || 'gb';
if (savedView === 'screen') {
    document.querySelectorAll('.gameboy, .gbc, .gba').forEach(el => el.classList.add('screen-only'));
    viewGbBtn.classList.remove('active');
    viewScreenBtn.classList.add('active');
}

viewGbBtn.addEventListener('click', () => {
    document.querySelectorAll('.gameboy, .gbc, .gba').forEach(el => el.classList.remove('screen-only'));
    viewGbBtn.classList.add('active');
    viewScreenBtn.classList.remove('active');
    localStorage.setItem('rugb-view', 'gb');
});

viewScreenBtn.addEventListener('click', () => {
    document.querySelectorAll('.gameboy, .gbc, .gba').forEach(el => el.classList.add('screen-only'));
    viewScreenBtn.classList.add('active');
    viewGbBtn.classList.remove('active');
    localStorage.setItem('rugb-view', 'screen');
});

// --- Shell switch buttons ---
// Restore saved shell override active state
document.querySelectorAll('.shell-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.shell === shellOverride);
});
document.querySelectorAll('.shell-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        shellOverride = btn.dataset.shell;
        localStorage.setItem('rugb-shell-override', shellOverride);
        if (currentSystem) {
            const detectedSystem = currentSystem;
            switchShell(detectedSystem);
        }
    });
});

// --- Resize observer: keep --gb-w in sync with actual width ---
const gbcShell = document.querySelector('.gbc');
const resizeObs = new ResizeObserver(entries => {
    for (const entry of entries) {
        const box = entry.borderBoxSize?.[0];
        const w = box ? box.inlineSize : entry.target.offsetWidth;
        entry.target.style.setProperty('--gb-w', w + 'px');
    }
});
resizeObs.observe(gameboy);
resizeObs.observe(gbcShell);

// --- Edge/corner resize ---
const EDGE = 8; // px from edge to trigger resize cursor

function getResizeEdge(e) {
    const r = gameboy.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const onLeft = x < EDGE;
    const onRight = x > r.width - EDGE;
    const onTop = y < EDGE;
    const onBottom = y > r.height - EDGE;
    if (!onLeft && !onRight && !onTop && !onBottom) return null;
    return { onLeft, onRight, onTop, onBottom };
}

function getCursorStyle(edge) {
    if (!edge) return '';
    const { onLeft, onRight, onTop, onBottom } = edge;
    if ((onTop && onLeft) || (onBottom && onRight)) return 'nwse-resize';
    if ((onTop && onRight) || (onBottom && onLeft)) return 'nesw-resize';
    if (onLeft || onRight) return 'ew-resize';
    if (onTop || onBottom) return 'ns-resize';
    return '';
}

let resizeDrag = null;

gameboy.addEventListener('mousemove', (e) => {
    if (resizeDrag) return;
    const edge = getResizeEdge(e);
    gameboy.style.cursor = getCursorStyle(edge);
});

gameboy.addEventListener('mouseleave', () => {
    if (!resizeDrag) gameboy.style.cursor = '';
});

gameboy.addEventListener('mousedown', (e) => {
    const edge = getResizeEdge(e);
    if (!edge) return;
    e.preventDefault();
    const startX = e.clientX;
    const startY = e.clientY;
    const startW = gameboy.offsetWidth;
    const isScreenOnly = gameboy.classList.contains('screen-only');

    const onMove = (ev) => {
        const dx = ev.clientX - startX;
        const dy = ev.clientY - startY;
        let newW = startW;
        if (edge.onRight) newW = startW + dx;
        else if (edge.onLeft) newW = startW - dx;
        if (!isScreenOnly) {
            // Game Boy mode: width only
            newW = Math.max(280, Math.min(newW, window.innerWidth * 0.95));
            gameboy.style.width = newW + 'px';
        } else {
            // Screen-only: free resize via width (height follows aspect ratio)
            if (edge.onTop || edge.onBottom) {
                const startH = gameboy.offsetHeight;
                const dh = edge.onBottom ? dy : -dy;
                const newH = Math.max(100, startH + dh);
                // Convert height to width using 160:144 aspect
                newW = newH * 160 / 144;
            }
            newW = Math.max(160, Math.min(newW, window.innerWidth * 0.95));
            gameboy.style.width = newW + 'px';
        }
    };

    const onUp = () => {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        resizeDrag = null;
        gameboy.style.cursor = '';
    };

    resizeDrag = edge;
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
});

// --- Audio setup ---

let audioCtx = null;
let audioWorkletNode = null;
let gainNode = null;
let analyserNode = null;
const AUDIO_SAMPLE_RATE = 48000;

// Adaptive buffer management
let targetBufferMs = 6;
let consecutiveUnderruns = 0;

async function initAudio() {
    if (audioCtx) return;
    audioCtx = new AudioContext({ sampleRate: AUDIO_SAMPLE_RATE });
    gainNode = audioCtx.createGain();
    gainNode.gain.value = parseInt(document.getElementById('volume-slider').value) / 100;
    analyserNode = audioCtx.createAnalyser();
    analyserNode.fftSize = 256;
    gainNode.connect(analyserNode);
    analyserNode.connect(audioCtx.destination);

    try {
        await audioCtx.audioWorklet.addModule('audio-processor.js');
        audioWorkletNode = new AudioWorkletNode(audioCtx, 'rugb-audio-processor', {
            outputChannelCount: [2],
        });
        audioWorkletNode.port.onmessage = (e) => {
            if (e.data.type === 'status') {
                if (e.data.underrun) {
                    consecutiveUnderruns++;
                    if (consecutiveUnderruns > 3) {
                        targetBufferMs = Math.min(targetBufferMs + 1, 12);
                    }
                } else {
                    consecutiveUnderruns = 0;
                    targetBufferMs = Math.max(targetBufferMs - 0.1, 4);
                }
            }
        };
        audioWorkletNode.connect(gainNode);
    } catch (err) {
        console.warn('AudioWorklet unavailable, using ScriptProcessor fallback', err);
        initAudioFallback();
    }
}

function initAudioFallback() {
    const bufferSize = 2048;
    const audioProcessor = audioCtx.createScriptProcessor(bufferSize, 0, 2);
    audioProcessor.onaudioprocess = (e) => {
        const left = e.outputBuffer.getChannelData(0);
        const right = e.outputBuffer.getChannelData(1);
        if (!emu || muted) {
            left.fill(0);
            right.fill(0);
            return;
        }
        let available = emu.audio_ring_available();
        let samplePairs = Math.floor(available / 2);

        const maxLatency = AUDIO_SAMPLE_RATE * 0.04;
        if (samplePairs > maxLatency) {
            const skip = samplePairs - left.length;
            if (skip > 0) {
                emu.audio_ring_consume(skip * 2);
                samplePairs = left.length;
            }
        }

        const count = Math.min(samplePairs, left.length);
        if (count > 0) {
            const ptr = emu.audio_ring_ptr();
            const capacity = emu.audio_ring_capacity();
            const wasmBuf = new Float32Array(wasm.memory.buffer, ptr, capacity);
            let rp = emu.audio_ring_read_pos();
            for (let i = 0; i < count; i++) {
                left[i] = wasmBuf[rp];
                rp = (rp + 1) % capacity;
                right[i] = wasmBuf[rp];
                rp = (rp + 1) % capacity;
            }
        }
        for (let i = count; i < left.length; i++) {
            left[i] = 0;
            right[i] = 0;
        }
        emu.audio_ring_consume(count * 2);
    };
    audioProcessor.connect(gainNode);
}

function feedAudioWorklet() {
    if (!audioWorkletNode || !emu || muted) return;

    const available = emu.audio_ring_available();
    if (available < 2) return;

    let samplePairs = Math.floor(available / 2);

    // Skip if WASM buffer exceeds 40ms
    const maxWasmBuffer = Math.floor(AUDIO_SAMPLE_RATE * 0.04);
    if (samplePairs > maxWasmBuffer) {
        const targetSamples = Math.floor(AUDIO_SAMPLE_RATE * targetBufferMs / 1000);
        const skip = samplePairs - targetSamples;
        if (skip > 0) {
            emu.audio_ring_consume(skip * 2);
            samplePairs = targetSamples;
        }
    }

    const count = Math.min(samplePairs, Math.floor(emu.audio_ring_available() / 2));
    if (count === 0) return;

    const ptr = emu.audio_ring_ptr();
    const capacity = emu.audio_ring_capacity();
    const mask = capacity - 1;
    const wasmBuf = new Float32Array(wasm.memory.buffer, ptr, capacity);
    let rp = emu.audio_ring_read_pos();

    const left = new Float32Array(count);
    const right = new Float32Array(count);
    for (let i = 0; i < count; i++) {
        left[i] = wasmBuf[rp];
        rp = (rp + 1) & mask;
        right[i] = wasmBuf[rp];
        rp = (rp + 1) & mask;
    }

    emu.audio_ring_consume(count * 2);
    audioWorkletNode.port.postMessage(
        { type: 'samples', left, right },
        [left.buffer, right.buffer]
    );
}

// --- Palettes ---

const PALETTES = {
    green:  { colors: ['#9bbc0f', '#8bac0f', '#306230', '#0f380f'] },
    gray:   { colors: ['#ffffff', '#aaaaaa', '#555555', '#000000'] },
    bw:     { colors: ['#ffffff', '#b0b0b0', '#404040', '#000000'] },
    custom: { colors: ['#e0f8d0', '#88c070', '#346856', '#081820'] },
};

// Restore custom palette from localStorage
const savedCustom = localStorage.getItem('rugb-custom-palette');
if (savedCustom) {
    try { PALETTES.custom.colors = JSON.parse(savedCustom); } catch {}
}

let currentPalette = localStorage.getItem('rugb-palette') || 'gray';

// Pre-computed palette lookup table: shade byte → [R, G, B]
// Rebuilt only when palette changes (not every frame)
let paletteLUT = null;

function buildPaletteLUT(pal) {
    if (!pal) {
        paletteLUT = null;
        if (glRenderer && glRenderer.ready) glRenderer.setPalette(null);
        return;
    }
    // Map shade values 0xFF, 0xAA, 0x55, 0x00 to RGB
    const lut = new Uint8Array(256 * 3); // 256 possible shade values × 3 channels
    const shades = [0xFF, 0xAA, 0x55, 0x00];
    for (let i = 0; i < 4; i++) {
        const hex = pal.colors[i];
        const r = parseInt(hex.slice(1, 3), 16);
        const g = parseInt(hex.slice(3, 5), 16);
        const b = parseInt(hex.slice(5, 7), 16);
        const idx = shades[i] * 3;
        lut[idx] = r;
        lut[idx + 1] = g;
        lut[idx + 2] = b;
    }
    paletteLUT = lut;
    // Upload palette to GPU if WebGL is active
    if (glRenderer && glRenderer.ready && currentSystem === 'gb') {
        glRenderer.setPalette(lut);
    }
}

function applyPalette(imageData) {
    if (!paletteLUT) return imageData;
    const data = imageData.data;
    for (let i = 0; i < data.length; i += 4) {
        const idx = data[i] * 3;
        data[i] = paletteLUT[idx];
        data[i + 1] = paletteLUT[idx + 1];
        data[i + 2] = paletteLUT[idx + 2];
    }
    return imageData;
}

// Build initial LUT
if (currentPalette !== 'gray') {
    buildPaletteLUT(PALETTES[currentPalette]);
}

// --- Emulator lifecycle ---

let batterySaveTimer = null;

function saveBatteryRAM() {
    if (worker) {
        // In worker mode, request battery RAM from worker (handled in onmessage)
        worker.postMessage({ cmd: 'battery_ram' });
        return;
    }
    if (!emu || !emu.has_battery()) return;
    try {
        const title = emu.title();
        if (!title) return;
        const len = emu.battery_ram_len();
        if (len === 0) return;
        const ptr = emu.battery_ram_ptr();
        const ram = new Uint8Array(new Uint8Array(wasm.memory.buffer, ptr, len));
        const b64 = uint8ToBase64(ram);
        localStorage.setItem(`rugb-sram-${title}`, b64);
    } catch {
        showToast('Storage full — cannot save game progress');
    }
}

function loadBatteryRAM() {
    if (!emu || !emu.has_battery()) return;
    const title = emu.title();
    if (!title) return;
    const b64 = localStorage.getItem(`rugb-sram-${title}`);
    if (!b64) return;
    if (b64.length > 2 * 1024 * 1024) return;
    const data = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
    emu.load_battery_ram(data);
}

// --- IPS/BPS patch support ---

function applyIpsPatch(rom, patch) {
    const view = new DataView(patch.buffer, patch.byteOffset, patch.byteLength);
    // Verify "PATCH" header
    const header = String.fromCharCode(patch[0], patch[1], patch[2], patch[3], patch[4]);
    if (header !== 'PATCH') return null;
    const result = new Uint8Array(rom);
    let pos = 5;
    while (pos < patch.length - 3) {
        const offset = (patch[pos] << 16) | (patch[pos + 1] << 8) | patch[pos + 2];
        pos += 3;
        if (offset === 0x454F46) break; // "EOF"
        const size = (patch[pos] << 8) | patch[pos + 1];
        pos += 2;
        if (size === 0) {
            // RLE record
            const rleSize = (patch[pos] << 8) | patch[pos + 1];
            pos += 2;
            const rleVal = patch[pos++];
            for (let i = 0; i < rleSize; i++) {
                if (offset + i < result.length) result[offset + i] = rleVal;
            }
        } else {
            for (let i = 0; i < size; i++) {
                if (offset + i < result.length) result[offset + i] = patch[pos + i];
            }
            pos += size;
        }
    }
    return result.buffer;
}

function applyBpsPatch(rom, patch) {
    const src = new Uint8Array(rom);
    const p = new Uint8Array(patch.buffer, patch.byteOffset, patch.byteLength);
    let pos = 0;
    function readVlq() {
        let value = 0, shift = 1;
        while (true) {
            const b = p[pos++];
            value += (b & 0x7F) * shift;
            if (b & 0x80) return value;
            value += shift;
            shift <<= 7;
        }
    }
    // Verify "BPS1" header
    if (p[0] !== 0x42 || p[1] !== 0x50 || p[2] !== 0x53 || p[3] !== 0x31) return null;
    pos = 4;
    const srcSize = readVlq();
    const targetSize = readVlq();
    if (targetSize > 64 * 1024 * 1024) return null;
    readVlq(); // metadata size (skip)
    const target = new Uint8Array(targetSize);
    let outPos = 0, srcRelOff = 0, tgtRelOff = 0;
    const dataEnd = p.length - 12; // last 12 bytes are checksums
    while (pos < dataEnd) {
        const cmd = readVlq();
        const length = (cmd >> 2) + 1;
        const action = cmd & 3;
        switch (action) {
            case 0: // SourceRead
                for (let i = 0; i < length; i++) target[outPos + i] = src[outPos + i];
                outPos += length;
                break;
            case 1: // TargetRead
                for (let i = 0; i < length; i++) target[outPos++] = p[pos++];
                break;
            case 2: { // SourceCopy
                const raw = readVlq();
                srcRelOff += (raw & 1 ? -(raw >> 1) : (raw >> 1));
                for (let i = 0; i < length; i++) target[outPos++] = src[srcRelOff++];
                break;
            }
            case 3: { // TargetCopy
                const raw = readVlq();
                tgtRelOff += (raw & 1 ? -(raw >> 1) : (raw >> 1));
                for (let i = 0; i < length; i++) target[outPos++] = target[tgtRelOff++];
                break;
            }
        }
    }
    return target.buffer;
}

function tryApplyPatch(romBytes, patchBytes) {
    const patch = new Uint8Array(patchBytes);
    const header = String.fromCharCode(patch[0], patch[1], patch[2], patch[3], patch[4] || 0);
    if (header.startsWith('PATCH')) {
        return applyIpsPatch(new Uint8Array(romBytes), patch);
    }
    if (patch[0] === 0x42 && patch[1] === 0x50 && patch[2] === 0x53 && patch[3] === 0x31) {
        return applyBpsPatch(romBytes, patch);
    }
    return null;
}

// --- Rewind helpers ---

function captureRewindState() {
    if (!emu) return;
    rewindBuffer.push(emu.save_state());
    if (rewindBuffer.length > REWIND_MAX_FRAMES) {
        rewindBuffer.shift();
    }
}

function rewindStep() {
    if (!emu || rewindBuffer.length === 0) return false;
    const state = rewindBuffer.pop();
    emu.load_state(state);
    return true;
}

// --- Auto-save on exit ---

async function autoSaveState() {
    if (worker) {
        const data = await workerSaveState();
        if (data) {
            try {
                const b64 = uint8ToBase64(new Uint8Array(data));
                // Use romBytes title since emu is in worker
                const u8 = new Uint8Array(romBytes);
                let title = '';
                for (let i = 0x134; i < 0x144 && i < u8.length; i++) {
                    if (u8[i] === 0) break;
                    title += String.fromCharCode(u8[i]);
                }
                title = sanitizeTitle(title);
                if (title) localStorage.setItem(`rugb-autosave-${title}`, b64);
            } catch {}
        }
        return;
    }
    if (!emu || !romBytes) return;
    try {
        const title = emu.title();
        if (!title) return;
        const state = emu.save_state();
        const b64 = uint8ToBase64(state);
        localStorage.setItem(`rugb-autosave-${title}`, b64);
    } catch {}
}

function autoLoadState() {
    if (!emu) return false;
    const title = emu.title();
    if (!title) return false;
    const b64 = localStorage.getItem(`rugb-autosave-${title}`);
    if (!b64) return false;
    if (b64.length > 2 * 1024 * 1024) return false;
    try {
        const data = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
        emu.load_state(data);
        localStorage.removeItem(`rugb-autosave-${title}`);
        return true;
    } catch { return false; }
}

function detectSystem(bytes) {
    const u8 = new Uint8Array(bytes);
    // GBA ROMs have fixed value 0x96 at offset 0xB2
    if (u8.length >= 0xB3 && u8[0xB2] === 0x96) return 'gba';
    // CGB flag at 0x0143: 0x80 = CGB-compatible, 0xC0 = CGB-only
    if (u8.length >= 0x144 && (u8[0x143] === 0x80 || u8[0x143] === 0xC0)) return 'gbc';
    return 'gb';
}

const VALID_SHELLS = new Set(['auto', 'dmg', 'gbc', 'gba']);
const _rawShell = localStorage.getItem('rugb-shell-override');
let shellOverride = VALID_SHELLS.has(_rawShell) ? _rawShell : 'auto';

function switchShell(system) {
    const gb = document.querySelector('.gameboy');
    const gbc = document.querySelector('.gbc');
    const gba = document.querySelector('.gba');
    const shoulders = document.getElementById('touch-shoulders');

    // Determine which shell to show
    let shell = system;
    if (shellOverride !== 'auto') {
        // GBA emulation needs 240x160 canvas — only allow GBA shell
        // GB/GBC emulation needs 160x144 — allow DMG or GBC shell
        if (currentSystem === 'gba') {
            shell = 'gba';
        } else {
            shell = (shellOverride === 'gba') ? system : shellOverride;
        }
    }

    gb.style.display = 'none';
    gbc.style.display = 'none';
    gba.style.display = 'none';

    if (shell === 'gba') {
        gba.style.display = '';
        canvas = document.getElementById('gba-screen');
        screenW = 240; screenH = 160;
        if (shoulders) shoulders.style.display = '';
    } else if (shell === 'gbc') {
        gbc.style.display = '';
        canvas = document.getElementById('gbc-screen');
        screenW = 160; screenH = 144;
        if (shoulders) shoulders.style.display = 'none';
    } else {
        gb.style.display = '';
        canvas = document.getElementById('screen');
        screenW = 160; screenH = 144;
        if (shoulders) shoulders.style.display = 'none';
    }
    screenBytes = screenW * screenH * 4;
    // Recreate renderer on the new canvas — try WebGL first, fall back to 2D
    if (glRenderer) glRenderer.destroy();
    glRenderer = new WebGLRenderer(canvas);
    if (glRenderer.ready) {
        ctx = null; // WebGL owns the canvas, no 2D context needed
        const filterMap = { scanlines: 'crt', lcd: 'lcd', smooth: 'smooth', ghosting: 'ghost' };
        const savedF = localStorage.getItem('rugb-filter') || 'none';
        glRenderer.setFilter(filterMap[savedF] || 'none');
        if (savedF === 'ghosting') glRenderer.setBlend(0.5);
        if (currentSystem === 'gb' && paletteLUT) {
            glRenderer.setPalette(paletteLUT);
        } else {
            glRenderer.setPalette(null);
        }
    } else {
        glRenderer = null;
        ctx = canvas.getContext('2d');
    }
    cachedImageData = new ImageData(screenW, screenH);
    frameMs = currentSystem === 'gba' ? GBA_FRAME_MS : GB_FRAME_MS;

    // Carry screen-only mode to the new shell
    const savedView = localStorage.getItem('rugb-view') || 'gb';
    if (savedView === 'screen') {
        [gb, gbc, gba].forEach(el => el.classList.add('screen-only'));
    }

    // Update shell button active state
    document.querySelectorAll('.shell-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.shell === shellOverride);
        // Disable DMG/GBC when running GBA
        if (currentSystem === 'gba' && (btn.dataset.shell === 'dmg' || btn.dataset.shell === 'gbc')) {
            btn.disabled = true;
        } else {
            btn.disabled = false;
        }
    });
}

// --- Worker-mode message handler ---

function setupWorkerHandlers(w) {
    w.onmessage = (e) => {
        const msg = e.data;
        switch (msg.cmd) {
            case 'ready':
                if (msg.title) {
                    const title = sanitizeTitle(msg.title);
                    document.title = `RUGB — ${title}`;
                    addRecentRom(title);
                    // Load battery RAM into worker
                    const b64 = localStorage.getItem(`rugb-sram-${title}`);
                    if (b64 && b64.length <= 2 * 1024 * 1024) {
                        const data = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
                        w.postMessage({ cmd: 'load_battery_ram', data }, [data.buffer]);
                    }
                    // Load auto-save state into worker
                    const autoB64 = localStorage.getItem(`rugb-autosave-${title}`);
                    if (autoB64 && autoB64.length <= 2 * 1024 * 1024) {
                        try {
                            const state = Uint8Array.from(atob(autoB64), c => c.charCodeAt(0));
                            w.postMessage({ cmd: 'load_state', data: state }, [state.buffer]);
                            localStorage.removeItem(`rugb-autosave-${title}`);
                        } catch {}
                    }
                }
                break;
            case 'audio':
                if (audioWorkletNode && !muted) {
                    audioWorkletNode.port.postMessage(
                        { type: 'samples', left: msg.left, right: msg.right },
                        [msg.left.buffer, msg.right.buffer]
                    );
                }
                break;
            case 'frame':
                // Fallback: worker sent raw pixels (no OffscreenCanvas)
                if (ctx && msg.pixels) {
                    cachedImageData.data.set(msg.pixels);
                    let imageData = cachedImageData;
                    if (currentSystem === 'gb') imageData = applyPalette(imageData);
                    ctx.putImageData(imageData, 0, 0);
                }
                break;
            case 'state':
                // Save state response — store in pending callback
                if (_pendingStateCb) { _pendingStateCb(msg.data); _pendingStateCb = null; }
                break;
            case 'battery_ram':
                if (msg.data && msg.title) {
                    try {
                        const b64 = uint8ToBase64(new Uint8Array(msg.data));
                        localStorage.setItem(`rugb-sram-${msg.title}`, b64);
                    } catch {}
                }
                break;
            case 'rumble':
                try {
                    const gamepads = navigator.getGamepads();
                    if (gamepads) {
                        for (const gp of gamepads) {
                            if (gp && gp.vibrationActuator) {
                                gp.vibrationActuator.playEffect('dual-rumble', {
                                    duration: 16, strongMagnitude: 0.5, weakMagnitude: 0.3,
                                });
                            }
                        }
                    }
                } catch {}
                break;
            case 'serial_out':
                if (linkCable) linkCable.sendByte(msg.byte);
                break;
        }
    };
}

let _pendingStateCb = null;

function workerSaveState() {
    return new Promise((resolve) => {
        _pendingStateCb = resolve;
        worker.postMessage({ cmd: 'save_state' });
        // Timeout fallback
        setTimeout(() => { if (_pendingStateCb) { _pendingStateCb(null); _pendingStateCb = null; } }, 2000);
    });
}

async function startEmulator(bytes) {
    // Save previous game's battery RAM before loading new one
    saveBatteryRAM();
    if (batterySaveTimer) clearInterval(batterySaveTimer);
    if (worker) { worker.terminate(); worker = null; }

    romBytes = bytes;
    const system = detectSystem(bytes);
    currentSystem = system;

    switchShell(system);
    await initAudio();

    pauseBtn.disabled = false;
    resetBtn.disabled = false;
    muteBtn.disabled = false;
    screenshotBtn.disabled = false;
    if (linkBtn) linkBtn.disabled = (system === 'gba');
    const debugBtn = document.getElementById('debug-btn');
    if (debugBtn) debugBtn.disabled = false;
    paused = false;
    pauseBtn.textContent = 'Pause';

    if (useWorker && canvas.transferControlToOffscreen) {
        await startEmulatorWorker(bytes, system);
    } else {
        await startEmulatorMainThread(bytes, system);
    }

    // Auto-save battery RAM every 5 seconds
    batterySaveTimer = setInterval(saveBatteryRAM, 5000);
}

async function startEmulatorWorker(bytes, system) {
    worker = new Worker('./js/emu-worker.js', { type: 'module' });
    setupWorkerHandlers(worker);

    // Transfer canvas to worker for direct rendering
    const offscreen = canvas.transferControlToOffscreen();
    const transfer = [offscreen, bytes.slice(0)];
    const initMsg = {
        cmd: 'init',
        rom: bytes,
        system,
        offscreen,
        bootRom: bootRomData ? bootRomData.slice(0) : null,
    };
    worker.postMessage(initMsg, transfer);

    emu = null; // emu lives in worker
    wasm = null;
    debugTools = new DebugTools(() => null, () => currentSystem, () => null);
}

async function startEmulatorMainThread(bytes, system) {
    if (system === 'gba') {
        if (!gbaWasm) gbaWasm = await initGba();
        wasm = gbaWasm;
        emu = new WasmGbaEmulator(new Uint8Array(bytes));
    } else {
        if (!gbWasm) gbWasm = await initGb();
        wasm = gbWasm;
        if (bootRomData) {
            emu = WasmEmulator.new_with_boot(new Uint8Array(bytes), new Uint8Array(bootRomData));
        } else {
            emu = new WasmEmulator(new Uint8Array(bytes));
        }
    }

    const title = sanitizeTitle(emu.title() || '');
    if (title) {
        document.title = `RUGB — ${title}`;
        addRecentRom(title);
    }

    loadBatteryRAM();
    autoLoadState();

    debugTools = new DebugTools(() => emu, () => currentSystem, () => wasm);

    // Reset rewind buffer for new ROM
    rewindBuffer = [];
    rewindFrameCounter = 0;

    // Reset frame timing
    lastFrameTs = 0;
    frameDebt = 0;

    if (animationId) cancelAnimationFrame(animationId);
    animationId = requestAnimationFrame(frame);
}

// --- Button action constants (shared by keyboard & gamepad remapping) ---

const ACTIONS = ['right', 'left', 'up', 'down', 'a', 'b', 'start', 'select'];
const ACTION_TO_BTN = { right: 0, left: 1, up: 2, down: 3, a: 4, b: 5, start: 6, select: 7 };

// --- Gamepad support ---

// Standard Gamepad API button names (Xbox / PlayStation / Switch)
function gpBtnName(index) {
    const names = {
        0: 'A / \u2A2F / B',
        1: 'B / \u25CB / A',
        2: 'X / \u25A1 / Y',
        3: 'Y / \u25B3 / X',
        4: 'LB / L1 / L',
        5: 'RB / R1 / R',
        6: 'LT / L2 / ZL',
        7: 'RT / R2 / ZR',
        8: 'Back / Share / -',
        9: 'Start / Options / +',
        10: 'L3 / L3 / LS',
        11: 'R3 / R3 / RS',
        12: 'D-pad Up',
        13: 'D-pad Down',
        14: 'D-pad Left',
        15: 'D-pad Right',
        16: 'Guide / PS / Home',
    };
    return names[index] || `Button ${index}`;
}

const DEFAULT_GP_MAP = {
    a: 0, b: 1, select: 8, start: 9,
    up: 12, down: 13, left: 14, right: 15,
};

function loadGpMap() {
    const saved = localStorage.getItem('rugb-gpmap');
    if (saved) {
        try { return { ...DEFAULT_GP_MAP, ...JSON.parse(saved) }; } catch {}
    }
    return { ...DEFAULT_GP_MAP };
}

let gpMap = loadGpMap();

function saveGpMap() {
    localStorage.setItem('rugb-gpmap', JSON.stringify(gpMap));
}

function buildGpButtonMap() {
    const map = {};
    for (const action of ACTIONS) {
        if (gpMap[action] !== undefined) {
            map[gpMap[action]] = ACTION_TO_BTN[action];
        }
    }
    return map;
}

let GP_BUTTON_MAP = buildGpButtonMap();

const AXIS_THRESHOLD = 0.5;
const gamepadPrev = {};

function pollGamepad() {
    if (!emu) return;
    const gamepads = navigator.getGamepads();
    if (!gamepads) return;

    for (const gp of gamepads) {
        if (!gp) continue;
        const id = gp.index;
        if (!gamepadPrev[id]) gamepadPrev[id] = {};
        const prev = gamepadPrev[id];

        // Mapped buttons from remap config
        for (const [btnIdxStr, gbBtn] of Object.entries(GP_BUTTON_MAP)) {
            const btnIdx = parseInt(btnIdxStr);
            if (btnIdx >= gp.buttons.length) continue;
            const pressed = gp.buttons[btnIdx].pressed;
            if (pressed !== prev[`b${btnIdx}`]) {
                emuSetButton(gbBtn, pressed);
                prev[`b${btnIdx}`] = pressed;
            }
        }

        // Left stick as D-pad (always active)
        if (gp.axes.length >= 2) {
            const lx = gp.axes[0];
            const ly = gp.axes[1];

            const left = lx < -AXIS_THRESHOLD;
            const right = lx > AXIS_THRESHOLD;
            const up = ly < -AXIS_THRESHOLD;
            const down = ly > AXIS_THRESHOLD;

            if (left !== prev.axL) { emuSetButton(1, left); prev.axL = left; }
            if (right !== prev.axR) { emuSetButton(0, right); prev.axR = right; }
            if (up !== prev.axU) { emuSetButton(2, up); prev.axU = up; }
            if (down !== prev.axD) { emuSetButton(3, down); prev.axD = down; }
        }
    }
}

// --- Gamepad remap overlay ---

const gpRemapOverlay = document.getElementById('gamepad-remap-overlay');
const gpRemapBtns = document.querySelectorAll('.gp-remap-btn');
const gpRemapResetBtn = document.getElementById('gp-remap-reset');
const gpRemapCloseBtn = document.getElementById('gp-remap-close');
const gpRemapBtn = document.getElementById('remap-gp-btn');
const gpStatus = document.getElementById('gp-status');
let gpRemapListening = null;
let gpRemapPollId = null;

function updateGpRemapButtons() {
    gpRemapBtns.forEach(btn => {
        const action = btn.dataset.action;
        btn.textContent = gpBtnName(gpMap[action]);
    });
}

function updateGpStatus() {
    const gamepads = navigator.getGamepads();
    let found = null;
    if (gamepads) {
        for (const gp of gamepads) {
            if (gp) { found = gp; break; }
        }
    }
    if (found) {
        gpStatus.textContent = found.id.slice(0, 200);
        gpStatus.classList.add('connected');
    } else {
        gpStatus.textContent = 'No controller detected — press a button on your controller';
        gpStatus.classList.remove('connected');
    }
}

function gpRemapPoll() {
    if (!gpRemapOverlay.classList.contains('visible')) return;

    updateGpStatus();

    if (gpRemapListening) {
        const gamepads = navigator.getGamepads();
        if (gamepads) {
            for (const gp of gamepads) {
                if (!gp) continue;
                for (let i = 0; i < gp.buttons.length; i++) {
                    if (gp.buttons[i].pressed) {
                        gpMap[gpRemapListening] = i;
                        saveGpMap();
                        GP_BUTTON_MAP = buildGpButtonMap();
                        gpRemapBtns.forEach(b => b.classList.remove('listening'));
                        updateGpRemapButtons();
                        gpRemapListening = null;
                        return;
                    }
                }
            }
        }
    }

    gpRemapPollId = requestAnimationFrame(gpRemapPoll);
}

gpRemapBtn.addEventListener('click', () => {
    sideMenu.classList.remove('open');
    updateGpRemapButtons();
    gpRemapOverlay.classList.add('visible');
    gpRemapPollId = requestAnimationFrame(gpRemapPoll);
});

gpRemapCloseBtn.addEventListener('click', () => {
    gpRemapListening = null;
    gpRemapBtns.forEach(b => b.classList.remove('listening'));
    gpRemapOverlay.classList.remove('visible');
    if (gpRemapPollId) cancelAnimationFrame(gpRemapPollId);
});

gpRemapResetBtn.addEventListener('click', () => {
    gpMap = { ...DEFAULT_GP_MAP };
    saveGpMap();
    GP_BUTTON_MAP = buildGpButtonMap();
    updateGpRemapButtons();
});

gpRemapBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        gpRemapBtns.forEach(b => b.classList.remove('listening'));
        btn.classList.add('listening');
        btn.textContent = 'Press button...';
        gpRemapListening = btn.dataset.action;
    });
});

// Frame timing constants
const GB_FRAME_MS = 70224 / 4194304 * 1000;   // ~16.74ms
const GBA_FRAME_MS = 280896 / 16777216 * 1000; // ~16.74ms
function getFrameMs() { return currentSystem === 'gba' ? GBA_FRAME_MS : GB_FRAME_MS; }
function getScreenSize() { return currentSystem === 'gba' ? { w: 240, h: 160 } : { w: 160, h: 144 }; }
// Cached screen dimensions (updated on system switch)
let screenW = 160, screenH = 144, screenBytes = 160 * 144 * 4;
let cachedImageData = null;
let frameMs = 16.74;
let lastFrameTs = 0;
let frameDebt = 0;

function frame(timestamp) {
    if (paused || !emu) return;

    // Time-based throttle: run the correct number of frames regardless of display refresh rate
    if (lastFrameTs === 0) {
        lastFrameTs = timestamp;
        animationId = requestAnimationFrame(frame);
        return;
    }

    let delta = timestamp - lastFrameTs;
    lastFrameTs = timestamp;

    // Cap delta to avoid spiral of death after tab was backgrounded
    const FRAME_MS = frameMs;
    if (delta > 100) delta = FRAME_MS;

    let framesRun = 0;

    // Rewind mode: step backwards instead of running frames
    if (rewinding) {
        if (!rewindStep()) rewinding = false;
        framesRun = 1;
    } else {
        const effectiveSpeed = fastForward ? 16 : speed;
        frameDebt += delta * effectiveSpeed;

        const maxFrames = fastForward ? 32 : Math.max(4, 4 * speed);
        while (frameDebt >= FRAME_MS && framesRun < maxFrames) {
            pollGamepad();
            // Turbo: toggle A/B every other frame
            turboFrame++;
            const turboOn = (turboFrame & 1) === 0;
            if (turboA) emuSetButton(4, turboOn);
            if (turboB) emuSetButton(5, turboOn);
            emu.run_frame();
            applyGameSharkCheats();
            frameDebt -= FRAME_MS;
            framesRun++;

            // Capture rewind state periodically
            rewindFrameCounter++;
            if (rewindFrameCounter >= REWIND_CAPTURE_INTERVAL) {
                rewindFrameCounter = 0;
                captureRewindState();
            }
        }
        // Prevent debt accumulation during fast forward
        if (fastForward && frameDebt > FRAME_MS * 4) frameDebt = 0;

        if (framesRun > 0) {
            feedAudioWorklet();
            // Link cable tick
            if (linkCable) linkCable.tick();
            // Rumble feedback via Gamepad haptics (GB only)
            if (currentSystem === 'gb' && emu.rumble()) {
                try {
                    const gamepads = navigator.getGamepads();
                    if (gamepads) {
                        for (const gp of gamepads) {
                            if (gp && gp.vibrationActuator) {
                                gp.vibrationActuator.playEffect('dual-rumble', {
                                    duration: 16, strongMagnitude: 0.5, weakMagnitude: 0.3,
                                });
                            }
                        }
                    }
                } catch {}
            }
        }
    }

    if (framesRun > 0) {
        const ptr = emu.framebuffer_ptr();
        const srcView = new Uint8ClampedArray(wasm.memory.buffer, ptr, screenBytes);

        // WebGL path: palette + blending handled by GPU shader
        if (glRenderer && glRenderer.ready) {
            glRenderer.render(srcView, screenW, screenH);
        } else {
            // Canvas 2D fallback
            cachedImageData.data.set(srcView);
            let imageData = cachedImageData;
            if (currentSystem === 'gb') imageData = applyPalette(imageData);
            if (frameBlending) {
                const cur = imageData.data;
                for (let i = 0; i < cur.length; i += 4) {
                    const r = cur[i], g = cur[i + 1], b = cur[i + 2];
                    cur[i]     = (r + prevFrameBuf[i])     >> 1;
                    cur[i + 1] = (g + prevFrameBuf[i + 1]) >> 1;
                    cur[i + 2] = (b + prevFrameBuf[i + 2]) >> 1;
                    prevFrameBuf[i] = r;
                    prevFrameBuf[i + 1] = g;
                    prevFrameBuf[i + 2] = b;
                }
            }
            ctx.putImageData(imageData, 0, 0);
        }

        // FPS counter
        if (showFps) {
            fpsFrameCount += framesRun;
            const now = performance.now();
            if (now - fpsLastTime >= 1000) {
                const fps = fpsFrameCount;
                const pct = Math.round(fps / 59.73 * 100);
                document.getElementById('fps-overlay').textContent = `${fps} FPS (${pct}%)`;
                fpsFrameCount = 0;
                fpsLastTime = now;
            }
        }

        // Audio visualizer (drawn in main loop, no separate RAF)
        drawViz();

        // Debug panel auto-update
        if (debugTools?.autoUpdate) debugTools.update();
    }

    animationId = requestAnimationFrame(frame);
}

// --- ROM loading ---

romInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (file) loadFile(file);
});

function loadFile(file) {
    const name = file.name.toLowerCase();

    // Handle patch files (IPS/BPS) — apply to currently loaded ROM
    if (name.endsWith('.ips') || name.endsWith('.bps')) {
        if (!romBytes) { showToast('Load a ROM first, then apply a patch'); return; }
        const reader = new FileReader();
        reader.onload = () => {
            const patched = tryApplyPatch(romBytes, reader.result);
            if (patched) {
                showToast('Patch applied');
                startEmulator(patched);
            } else {
                showToast('Invalid patch file');
            }
        };
        reader.readAsArrayBuffer(file);
        return;
    }

    // Handle boot ROM file
    if (name === 'dmg_boot.bin' || name === 'boot.bin' || name === 'bootrom.bin') {
        const reader = new FileReader();
        reader.onload = () => {
            const data = reader.result;
            if (data.byteLength === 256) {
                bootRomData = data;
                showToast('Boot ROM loaded');
            } else {
                showToast('Invalid boot ROM (must be 256 bytes)');
            }
        };
        reader.readAsArrayBuffer(file);
        return;
    }

    const reader = new FileReader();
    reader.onload = async () => {
        const buf = reader.result;
        const view = new DataView(buf);
        // ZIP magic: PK\x03\x04
        if (view.byteLength >= 4 && view.getUint32(0, true) === 0x04034b50) {
            const rom = await extractRomFromZip(buf);
            if (rom) startEmulator(rom);
        } else {
            startEmulator(buf);
        }
    };
    reader.readAsArrayBuffer(file);
}

async function extractRomFromZip(buffer) {
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);

    // Find End of Central Directory record (search backwards from end)
    let eocd = -1;
    for (let i = bytes.length - 22; i >= 0; i--) {
        if (view.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
    }
    if (eocd === -1) { showToast('Invalid zip file'); return null; }

    const entryCount = view.getUint16(eocd + 10, true);
    const cdOffset = view.getUint32(eocd + 16, true);

    // Walk central directory entries looking for a ROM file
    let off = cdOffset;
    for (let i = 0; i < entryCount; i++) {
        if (off + 46 > bytes.length) break;
        if (view.getUint32(off, true) !== 0x02014b50) break;

        const method = view.getUint16(off + 10, true);
        const compSize = view.getUint32(off + 20, true);
        const nameLen = view.getUint16(off + 28, true);
        const extraLen = view.getUint16(off + 30, true);
        const commentLen = view.getUint16(off + 32, true);
        const localOff = view.getUint32(off + 42, true);
        const name = new TextDecoder().decode(bytes.slice(off + 46, off + 46 + nameLen));

        if (/\.(gb|gbc|bin)$/i.test(name)) {
            // Read past the local file header to reach the raw data
            const localNameLen = view.getUint16(localOff + 26, true);
            const localExtraLen = view.getUint16(localOff + 28, true);
            const dataOff = localOff + 30 + localNameLen + localExtraLen;
            const compData = bytes.slice(dataOff, dataOff + compSize);

            if (method === 0) return compData.buffer;          // stored
            if (method === 8) return deflateRaw(compData);     // deflate
            showToast('Unsupported zip compression');
            return null;
        }

        off += 46 + nameLen + extraLen + commentLen;
        if (off > bytes.length) break;
    }

    showToast('No .gb ROM found in zip');
    return null;
}

async function deflateRaw(compressed) {
    const ds = new DecompressionStream('deflate-raw');
    const writer = ds.writable.getWriter();
    writer.write(compressed);
    writer.close();
    const reader = ds.readable.getReader();
    const chunks = [];
    const MAX_ROM_BYTES = 64 * 1024 * 1024;
    let totalBytes = 0;
    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
        totalBytes += value.length;
        if (totalBytes > MAX_ROM_BYTES) { reader.cancel(); return null; }
    }
    const total = chunks.reduce((s, c) => s + c.length, 0);
    const result = new Uint8Array(total);
    let pos = 0;
    for (const c of chunks) { result.set(c, pos); pos += c.length; }
    return result.buffer;
}

// Drag & drop (full page)
document.body.addEventListener('dragover', (e) => {
    e.preventDefault();
    canvas.classList.add('drag-over');
});
document.body.addEventListener('dragleave', (e) => {
    // Only remove highlight when leaving the page entirely
    if (!e.relatedTarget) canvas.classList.remove('drag-over');
});
document.body.addEventListener('drop', (e) => {
    e.preventDefault();
    canvas.classList.remove('drag-over');
    const file = e.dataTransfer.files[0];
    if (file) loadFile(file);
});

// --- Controls ---

pauseBtn.addEventListener('click', () => {
    paused = !paused;
    pauseBtn.textContent = paused ? 'Resume' : 'Pause';
    if (worker) {
        worker.postMessage({ cmd: paused ? 'pause' : 'resume' });
    } else if (!paused) {
        lastFrameTs = 0;
        frameDebt = 0;
        animationId = requestAnimationFrame(frame);
    }
});

resetBtn.addEventListener('click', () => {
    if (romBytes) startEmulator(romBytes);
});

// Screenshot
const screenshotBtn = document.getElementById('screenshot-btn');
screenshotBtn.addEventListener('click', () => {
    const link = document.createElement('a');
    link.download = `rugb-${(emu ? emu.title() : 'screenshot').replace(/[^a-zA-Z0-9]/g, '_')}.png`;
    link.href = canvas.toDataURL('image/png');
    link.click();
    showToast('Screenshot saved');
});

// Fullscreen
const fullscreenBtn = document.getElementById('fullscreen-btn');
fullscreenBtn.addEventListener('click', () => {
    sideMenu.classList.remove('open');
    const target = gameboy.classList.contains('screen-only') ? canvas : gameboy;
    if (!document.fullscreenElement) {
        target.requestFullscreen().catch(() => {});
    } else {
        document.exitFullscreen();
    }
});
document.addEventListener('fullscreenchange', () => {
    fullscreenBtn.textContent = document.fullscreenElement ? 'Exit Fullscreen' : 'Fullscreen';
});

muteBtn.addEventListener('click', () => {
    muted = !muted;
    muteBtn.textContent = muted ? 'Unmute' : 'Mute';
    if (worker) worker.postMessage({ cmd: 'mute', muted });
});

// Display filters — maps data-filter values to WebGL shader names
const FILTER_TO_SHADER = { none: 'none', scanlines: 'crt', lcd: 'lcd', smooth: 'smooth', ghosting: 'ghost' };
const filterBtns = document.querySelectorAll('.filter-btn');
const screenFrame = document.querySelector('.gb-screen-frame');
filterBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        const filter = btn.dataset.filter;
        filterBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        frameBlending = false;

        if (worker) {
            // Worker owns the renderer
            worker.postMessage({ cmd: 'set_filter', filter: FILTER_TO_SHADER[filter] || 'none', blend: filter === 'ghosting' ? 0.5 : 0.0 });
        } else if (glRenderer && glRenderer.ready) {
            // WebGL path: switch shader program
            glRenderer.setFilter(FILTER_TO_SHADER[filter] || 'none');
            glRenderer.setBlend(filter === 'ghosting' ? 0.5 : 0.0);
        } else {
            // CSS fallback
            canvas.classList.remove('filter-smooth');
            screenFrame.classList.remove('filter-scanlines', 'filter-lcd');
            if (filter === 'smooth') canvas.classList.add('filter-smooth');
            if (filter === 'scanlines') screenFrame.classList.add('filter-scanlines');
            if (filter === 'lcd') screenFrame.classList.add('filter-lcd');
            if (filter === 'ghosting') frameBlending = true;
        }
        localStorage.setItem('rugb-filter', filter);
    });
});
// Restore saved filter
const VALID_FILTERS = new Set(['none', 'scanlines', 'lcd', 'smooth', 'ghosting']);
const savedFilter = localStorage.getItem('rugb-filter');
if (savedFilter && VALID_FILTERS.has(savedFilter) && savedFilter !== 'none') {
    document.querySelector(`.filter-btn[data-filter="${savedFilter}"]`)?.click();
}

// Volume slider
document.getElementById('volume-slider').addEventListener('input', (e) => {
    const vol = parseInt(e.target.value) / 100;
    if (gainNode) gainNode.gain.value = vol;
});

// Per-channel mute
document.querySelectorAll('.ch-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        btn.classList.toggle('active');
        const ch = parseInt(btn.dataset.ch);
        const muted = !btn.classList.contains('active');
        if (emu) emu.set_channel_mute(ch, muted);
    });
});

// Speed buttons
speedBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        speed = parseFloat(btn.dataset.speed);
        speedBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        if (worker) worker.postMessage({ cmd: 'set_speed', speed });
    });
});

// Palette buttons
const customPaletteEl = document.getElementById('custom-palette');
paletteBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        currentPalette = btn.dataset.palette;
        localStorage.setItem('rugb-palette', currentPalette);
        paletteBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        customPaletteEl.style.display = currentPalette === 'custom' ? 'flex' : 'none';
        buildPaletteLUT(currentPalette === 'gray' ? null : PALETTES[currentPalette]);
    });
});

// Custom palette color pickers
const palColors = document.querySelectorAll('.pal-color');
// Init picker values from stored palette
palColors.forEach(input => {
    input.value = PALETTES.custom.colors[parseInt(input.dataset.idx)];
});
palColors.forEach(input => {
    input.addEventListener('input', () => {
        PALETTES.custom.colors[parseInt(input.dataset.idx)] = input.value;
        localStorage.setItem('rugb-custom-palette', JSON.stringify(PALETTES.custom.colors));
        if (currentPalette === 'custom') buildPaletteLUT(PALETTES.custom);
    });
});

// Set initial active palette button
document.querySelector(`.palette-btn[data-palette="${currentPalette}"]`)?.classList.add('active');
if (currentPalette === 'custom') customPaletteEl.style.display = 'flex';

// --- Key remapping ---

const DEFAULT_KEYS = {
    right: 'ArrowRight', left: 'ArrowLeft', up: 'ArrowUp', down: 'ArrowDown',
    a: 'z', b: 'x', start: 'Enter', select: 'Shift',
    pause: 'p', mute: 'm', quicksave: 'F5', quickload: 'F8',
};

function loadKeyMap() {
    const saved = localStorage.getItem('rugb-keymap');
    if (saved) {
        try {
            const VALID_ACTIONS = new Set(Object.keys(DEFAULT_KEYS));
            const parsed = JSON.parse(saved);
            const merged = { ...DEFAULT_KEYS };
            for (const [k, v] of Object.entries(parsed)) {
                if (VALID_ACTIONS.has(k) && typeof v === 'string' && v.length > 0 && v.length < 32) {
                    merged[k] = v;
                }
            }
            return merged;
        } catch {}
    }
    return { ...DEFAULT_KEYS };
}

let keyMap = loadKeyMap();

function saveKeyMap() {
    localStorage.setItem('rugb-keymap', JSON.stringify(keyMap));
}

function buildButtonMap() {
    const map = {};
    for (const action of ACTIONS) {
        map[keyMap[action]] = ACTION_TO_BTN[action];
    }
    return map;
}

let BUTTON_MAP = buildButtonMap();

function keyDisplayName(key) {
    const names = { ' ': 'Space', 'ArrowUp': '\u2191', 'ArrowDown': '\u2193', 'ArrowLeft': '\u2190', 'ArrowRight': '\u2192' };
    return names[key] || key;
}

// --- Remap overlay ---

const remapOverlay = document.getElementById('remap-overlay');
const remapBtns = document.querySelectorAll('.remap-btn');
const remapResetBtn = document.getElementById('remap-reset');
const remapCloseBtn = document.getElementById('remap-close');
const remapExportBtn = document.getElementById('remap-export');
const remapImportBtn = document.getElementById('remap-import');
const remapFileInput = document.getElementById('remap-file');
const remapBtn = document.getElementById('remap-btn');
let remapListening = null;

function updateRemapButtons() {
    remapBtns.forEach(btn => {
        const action = btn.dataset.action;
        btn.textContent = keyDisplayName(keyMap[action]);
    });
}

remapBtn.addEventListener('click', () => {
    sideMenu.classList.remove('open');
    updateRemapButtons();
    remapOverlay.classList.add('visible');
});

remapCloseBtn.addEventListener('click', () => {
    remapListening = null;
    remapBtns.forEach(b => b.classList.remove('listening'));
    remapOverlay.classList.remove('visible');
});

remapResetBtn.addEventListener('click', () => {
    keyMap = { ...DEFAULT_KEYS };
    saveKeyMap();
    BUTTON_MAP = buildButtonMap();
    updateRemapButtons();
});

remapExportBtn.addEventListener('click', () => {
    const blob = new Blob([JSON.stringify(keyMap, null, 2)], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'rugb-keybinds.json';
    a.click();
    URL.revokeObjectURL(a.href);
});

remapImportBtn.addEventListener('click', () => remapFileInput.click());

remapFileInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
        try {
            const imported = JSON.parse(reader.result);
            keyMap = { ...DEFAULT_KEYS, ...imported };
            saveKeyMap();
            BUTTON_MAP = buildButtonMap();
            updateRemapButtons();
        } catch {
            showToast('Invalid keybinds file');
        }
    };
    reader.readAsText(file);
    remapFileInput.value = '';
});

remapBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        remapBtns.forEach(b => b.classList.remove('listening'));
        btn.classList.add('listening');
        btn.textContent = 'Press a key...';
        remapListening = btn.dataset.action;
    });
});

// --- Keyboard ---

document.addEventListener('keydown', (e) => {
    // Remap mode: capture the key for the selected action
    if (remapListening) {
        if (!e.key || e.key === 'Unidentified' || e.key === 'Dead' || e.key === 'Process') return;
        e.preventDefault();
        if (e.key === 'Escape') {
            remapBtns.forEach(b => b.classList.remove('listening'));
            updateRemapButtons();
            remapListening = null;
            return;
        }
        keyMap[remapListening] = e.key;
        saveKeyMap();
        BUTTON_MAP = buildButtonMap();
        remapBtns.forEach(b => b.classList.remove('listening'));
        updateRemapButtons();
        remapListening = null;
        return;
    }

    if (e.key === '?') { document.getElementById('shortcuts-overlay').classList.toggle('visible'); return; }
    if (e.key === 'Escape') { sideMenu.classList.toggle('open'); return; }
    if (e.key === 'F3') {
        e.preventDefault();
        showFps = !showFps;
        const el = document.getElementById('fps-overlay');
        el.style.display = showFps ? 'block' : 'none';
        if (!showFps) { fpsFrameCount = 0; fpsLastTime = 0; }
        else { fpsLastTime = performance.now(); }
        return;
    }
    if (e.key === 'F7') {
        e.preventDefault();
        const code = prompt('Enter cheat code (Game Genie: XXX-XXX-XXX or GameShark: AABBCCDD):');
        if (code) {
            if (addCheat(code.trim())) showToast('Cheat added');
            else showToast('Invalid cheat code');
        }
        return;
    }
    if (e.key === 'F6') { e.preventDefault(); promptRtcOffset(); return; }
    if (e.key === 'F9') { e.preventDefault(); toggleRecording(); return; }
    if (e.key === 'F10') { e.preventDefault(); generateShareLink(); return; }
    if (e.key === 'F11') { e.preventDefault(); fullscreenBtn.click(); return; }
    if (e.key === ' ') {
        e.preventDefault(); fastForward = true;
        if (worker) worker.postMessage({ cmd: 'fast_forward', enabled: true });
        return;
    }
    if (e.key === 'r') {
        e.preventDefault(); rewinding = true;
        if (worker) worker.postMessage({ cmd: 'rewind_start' });
        return;
    }
    if (e.key === 'q') {
        turboA = !turboA; showToast(turboA ? 'Turbo A ON' : 'Turbo A OFF');
        if (worker) worker.postMessage({ cmd: 'turbo', button: 'a', enabled: turboA });
        return;
    }
    if (e.key === 'w') {
        turboB = !turboB; showToast(turboB ? 'Turbo B ON' : 'Turbo B OFF');
        if (worker) worker.postMessage({ cmd: 'turbo', button: 'b', enabled: turboB });
        return;
    }
    if (e.key === keyMap.pause) { pauseBtn.click(); return; }
    if (e.key === keyMap.mute) { muteBtn.click(); return; }
    if (e.key === '1') { document.querySelector('.speed-btn[data-speed="1"]')?.click(); return; }
    if (e.key === '2') { document.querySelector('.speed-btn[data-speed="2"]')?.click(); return; }
    if (e.key === '4') { document.querySelector('.speed-btn[data-speed="4"]')?.click(); return; }
    // Quick save/load
    if (emu || worker) {
        if (e.key === keyMap.quicksave) { e.preventDefault(); doQuickSave(); return; }
        if (e.key === keyMap.quickload) { e.preventDefault(); doQuickLoad(); return; }
    }
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined) {
        if (!e.repeat && (emu || worker)) emuSetButton(btn, true);
        if (btnIndexToEl[btn]) btnIndexToEl[btn].classList.add('pressed');
        e.preventDefault();
    }
});

document.addEventListener('keyup', (e) => {
    if (e.key === ' ') {
        fastForward = false;
        if (worker) worker.postMessage({ cmd: 'fast_forward', enabled: false });
        return;
    }
    if (e.key === 'r') {
        rewinding = false;
        if (worker) worker.postMessage({ cmd: 'rewind_stop' });
        return;
    }
    if (remapListening) return;
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined) {
        if (emu) emuSetButton(btn, false);
        if (btnIndexToEl[btn]) btnIndexToEl[btn].classList.remove('pressed');
        e.preventDefault();
    }
});

// --- Visual Game Boy buttons ---

const gbInputBtns = document.querySelectorAll('.gb-input');

// Map GB button index to its visual element for keyboard feedback
const btnIndexToEl = {};
gbInputBtns.forEach(el => {
    btnIndexToEl[parseInt(el.dataset.btn)] = el;
});

// Mouse / touch on visual buttons
gbInputBtns.forEach(el => {
    const gbBtn = parseInt(el.dataset.btn);
    const press = (e) => {
        e.preventDefault();
        el.classList.add('pressed');
        if (e.type === 'touchstart') haptic(15);
        if (emu) emuSetButton(gbBtn, true);
    };
    const release = (e) => {
        e.preventDefault();
        if (!el.classList.contains('pressed')) return;
        el.classList.remove('pressed');
        if (emu) emuSetButton(gbBtn, false);
    };
    el.addEventListener('mousedown', press);
    el.addEventListener('mouseup', release);
    el.addEventListener('mouseleave', release);
    el.addEventListener('touchstart', press);
    el.addEventListener('touchend', release);
    el.addEventListener('touchcancel', release);
});

// --- Visual GBA buttons ---
document.querySelectorAll('.gba-input').forEach(el => {
    const gbaBtn = parseInt(el.dataset.btn);
    const press = (e) => {
        e.preventDefault();
        el.classList.add('pressed');
        if (e.type === 'touchstart') haptic(15);
        if (emu) emuSetButton(gbaBtn, true);
    };
    const release = (e) => {
        e.preventDefault();
        if (!el.classList.contains('pressed')) return;
        el.classList.remove('pressed');
        if (emu) emuSetButton(gbaBtn, false);
    };
    el.addEventListener('mousedown', press);
    el.addEventListener('mouseup', release);
    el.addEventListener('mouseleave', release);
    el.addEventListener('touchstart', press);
    el.addEventListener('touchend', release);
    el.addEventListener('touchcancel', release);
});

// --- Haptic feedback ---
function haptic(ms) {
    if (navigator.vibrate) navigator.vibrate(ms);
}

// --- Mobile touch controls ---

document.querySelectorAll('.touch-btn[data-btn]').forEach(el => {
    const gbBtn = parseInt(el.dataset.btn);
    const press = (e) => {
        e.preventDefault();
        el.classList.add('pressed');
        haptic(15);
        if (emu) emuSetButton(gbBtn, true);
    };
    const release = (e) => {
        e.preventDefault();
        el.classList.remove('pressed');
        if (emu) emuSetButton(gbBtn, false);
    };
    el.addEventListener('touchstart', press);
    el.addEventListener('touchend', release);
    el.addEventListener('touchcancel', release);
});

// --- Save States ---

const savestatesBtn = document.getElementById('savestates-btn');
const savestatesOverlay = document.getElementById('savestates-overlay');
const savestatesClose = document.getElementById('savestates-close');
const toast = document.getElementById('toast');

let quickSaveData = null;
const saveSlots = {}; // slot number -> { data: Uint8Array, timestamp: string }

// Load saved slots from localStorage
for (let i = 1; i <= 5; i++) {
    const stored = localStorage.getItem(`rugb-slot-${i}`);
    if (stored) {
        try {
            const parsed = JSON.parse(stored);
            saveSlots[i] = { data: Uint8Array.from(atob(parsed.data), c => c.charCodeAt(0)), timestamp: parsed.timestamp, title: parsed.title || '' };
        } catch {}
    }
}

function showToast(msg) {
    toast.textContent = msg;
    toast.classList.add('show');
    setTimeout(() => toast.classList.remove('show'), 1500);
}

async function doQuickSave() {
    if (worker) {
        const data = await workerSaveState();
        if (data) { quickSaveData = new Uint8Array(data); showToast('Quick Save'); }
        return;
    }
    if (!emu) return;
    quickSaveData = emu.save_state();
    showToast('Quick Save');
}

function doQuickLoad() {
    if (worker) {
        if (!quickSaveData) { showToast('No quick save'); return; }
        worker.postMessage({ cmd: 'load_state', data: quickSaveData });
        showToast('Quick Load');
        return;
    }
    if (!emu || !quickSaveData) { showToast('No quick save'); return; }
    emu.load_state(quickSaveData);
    showToast('Quick Load');
}

function uint8ToBase64(data) {
    let binary = '';
    for (let i = 0; i < data.length; i++) {
        binary += String.fromCharCode(data[i]);
    }
    return btoa(binary);
}

async function saveToSlot(slot) {
    let data, title;
    if (worker) {
        data = await workerSaveState();
        if (!data) return;
        data = new Uint8Array(data);
        // Extract title from ROM header
        const u8 = new Uint8Array(romBytes);
        title = '';
        for (let i = 0x134; i < 0x144 && i < u8.length; i++) {
            if (u8[i] === 0) break;
            title += String.fromCharCode(u8[i]);
        }
        title = sanitizeTitle(title);
    } else {
        if (!emu) return;
        data = emu.save_state();
        title = emu.title() || '';
    }
    const timestamp = new Date().toLocaleString();
    saveSlots[slot] = { data, timestamp, title };
    const b64 = uint8ToBase64(data);
    localStorage.setItem(`rugb-slot-${slot}`, JSON.stringify({ data: b64, timestamp, title }));
    updateSlotUI();
    showToast(`Saved to Slot ${slot}`);
}

function loadFromSlot(slot) {
    if (!saveSlots[slot]) return;
    if (worker) {
        worker.postMessage({ cmd: 'load_state', data: saveSlots[slot].data });
        showToast(`Loaded Slot ${slot}`);
        return;
    }
    if (!emu) return;
    emu.load_state(saveSlots[slot].data);
    showToast(`Loaded Slot ${slot}`);
}

function updateSlotUI() {
    for (let i = 1; i <= 5; i++) {
        const info = document.getElementById(`slot-info-${i}`);
        const loadBtn = document.querySelector(`.slot-load[data-slot="${i}"]`);
        const exportBtn = document.querySelector(`.slot-export[data-slot="${i}"]`);
        const importBtn = document.querySelector(`.slot-import[data-slot="${i}"]`);
        if (saveSlots[i]) {
            info.textContent = saveSlots[i].timestamp;
            info.classList.add('has-data');
            loadBtn.disabled = false;
            exportBtn.disabled = false;
            importBtn.disabled = true;
        } else {
            info.textContent = 'Empty';
            info.classList.remove('has-data');
            loadBtn.disabled = true;
            exportBtn.disabled = true;
            importBtn.disabled = false;
        }
    }
}

savestatesBtn.addEventListener('click', () => {
    sideMenu.classList.remove('open');
    updateSlotUI();
    savestatesOverlay.classList.add('visible');
});

savestatesClose.addEventListener('click', () => {
    savestatesOverlay.classList.remove('visible');
});

document.querySelectorAll('.slot-save').forEach(btn => {
    btn.addEventListener('click', () => saveToSlot(parseInt(btn.dataset.slot)));
});

document.querySelectorAll('.slot-load').forEach(btn => {
    btn.addEventListener('click', () => loadFromSlot(parseInt(btn.dataset.slot)));
});

// Export save state as JSON
document.querySelectorAll('.slot-export').forEach(btn => {
    btn.addEventListener('click', () => {
        const slot = parseInt(btn.dataset.slot);
        const s = saveSlots[slot];
        if (!s) return;
        const b64 = uint8ToBase64(s.data);
        const json = JSON.stringify({ title: s.title, timestamp: s.timestamp, data: b64 }, null, 2);
        const a = document.createElement('a');
        a.href = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
        a.download = `rugb-save-${(s.title || 'unknown').replace(/[^a-zA-Z0-9]/g, '_')}-slot${slot}.json`;
        a.click();
        URL.revokeObjectURL(a.href);
    });
});

// Import save state from JSON into an empty slot
const savestateFileInput = document.getElementById('savestate-file');
let importTargetSlot = null;

document.querySelectorAll('.slot-import').forEach(btn => {
    btn.addEventListener('click', () => {
        importTargetSlot = parseInt(btn.dataset.slot);
        savestateFileInput.click();
    });
});

savestateFileInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file || importTargetSlot === null) return;
    const slot = importTargetSlot;
    const reader = new FileReader();
    reader.onload = () => {
        try {
            const imported = JSON.parse(reader.result);
            if (!imported.data || !imported.title) {
                showToast('Invalid save state file');
                return;
            }
            const currentTitle = emu ? (emu.title() || '') : '';
            if (imported.title !== currentTitle) {
                showToast(`Wrong ROM: expected "${imported.title}", loaded "${currentTitle || 'none'}"`);
                return;
            }
            if (imported.data.length > 2 * 1024 * 1024) { showToast('Invalid save state file'); return; }
            const data = Uint8Array.from(atob(imported.data), c => c.charCodeAt(0));
            const timestamp = imported.timestamp || new Date().toLocaleString();
            saveSlots[slot] = { data, timestamp, title: imported.title };
            const b64 = imported.data;
            localStorage.setItem(`rugb-slot-${slot}`, JSON.stringify({ data: b64, timestamp, title: imported.title }));
            updateSlotUI();
            showToast(`Imported to Slot ${slot}`);
        } catch {
            showToast('Invalid save state file');
        }
    };
    reader.readAsText(file);
    savestateFileInput.value = '';
});

// Initialize slot UI
updateSlotUI();

// --- Recent ROMs ---

const recentRomsEl = document.getElementById('recent-roms');
const MAX_RECENT = 5;

function getRecentRoms() {
    try { return JSON.parse(localStorage.getItem('rugb-recent') || '[]'); } catch { return []; }
}

function addRecentRom(name) {
    let list = getRecentRoms().filter(n => n !== name);
    list.unshift(name);
    if (list.length > MAX_RECENT) list = list.slice(0, MAX_RECENT);
    localStorage.setItem('rugb-recent', JSON.stringify(list));
    renderRecentRoms();
}

function renderRecentRoms() {
    recentRomsEl.replaceChildren();
    const list = getRecentRoms();
    for (const name of list) {
        const el = document.createElement('div');
        el.className = 'recent-rom';
        el.textContent = name;
        el.title = name;
        recentRomsEl.appendChild(el);
    }
}

renderRecentRoms();

// Save battery RAM and auto-save state when leaving the page
window.addEventListener('beforeunload', () => {
    saveBatteryRAM();
    autoSaveState();
});
document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
        saveBatteryRAM();
        autoSaveState();
    }
});

// --- Audio visualizer (called from main frame loop, no separate RAF) ---
const vizCanvas = document.getElementById('audio-viz');
const vizCtx = vizCanvas.getContext('2d');
let vizData = null; // Pre-allocated, created when analyser is ready

// Per-channel mini visualizers
const CH_COLORS = ['#50fa7b', '#ff79c6', '#8be9fd', '#ffb86c'];
const chVizCanvases = [
    document.getElementById('viz-ch1'),
    document.getElementById('viz-ch2'),
    document.getElementById('viz-ch3'),
    document.getElementById('viz-ch4'),
];
const chVizCtxs = chVizCanvases.map(c => c ? c.getContext('2d') : null);

function drawViz() {
    if (!analyserNode) return;
    if (!vizData) vizData = new Uint8Array(analyserNode.frequencyBinCount);
    analyserNode.getByteTimeDomainData(vizData);

    // Main waveform
    const w = vizCanvas.width;
    const h = vizCanvas.height;
    vizCtx.fillStyle = '#0a0e1a';
    vizCtx.fillRect(0, 0, w, h);
    vizCtx.lineWidth = 1.5;
    vizCtx.strokeStyle = '#8be9fd';
    vizCtx.beginPath();
    const sliceW = w / vizData.length;
    for (let i = 0; i < vizData.length; i++) {
        const y = (vizData[i] / 255) * h;
        if (i === 0) vizCtx.moveTo(0, y);
        else vizCtx.lineTo(i * sliceW, y);
    }
    vizCtx.stroke();

    // Per-channel mini waveforms — split frequency bands as approximation
    // CH1/CH2 (square) = lower freqs, CH3 (wave) = mid, CH4 (noise) = high
    const binCount = vizData.length;
    const bandSize = Math.floor(binCount / 4);
    for (let ch = 0; ch < 4; ch++) {
        const ctx = chVizCtxs[ch];
        if (!ctx) continue;
        const cw = chVizCanvases[ch].width;
        const ch2 = chVizCanvases[ch].height;
        ctx.fillStyle = '#0a0e1a';
        ctx.fillRect(0, 0, cw, ch2);
        ctx.lineWidth = 1;
        ctx.strokeStyle = CH_COLORS[ch];
        ctx.beginPath();
        const start = ch * bandSize;
        const end = start + bandSize;
        const sw = cw / bandSize;
        for (let i = start; i < end; i++) {
            const y = (vizData[i] / 255) * ch2;
            if (i === start) ctx.moveTo(0, y);
            else ctx.lineTo((i - start) * sw, y);
        }
        ctx.stroke();
    }
}

// --- Cheat codes (Game Genie / GameShark) ---

let activeCheats = []; // { type: 'gg'|'gs', code, addr, val, compare? }

function parseGameGenie(code) {
    // GB Game Genie: XXX-XXX or XXX-XXX-XXX
    const clean = code.replace(/-/g, '').toUpperCase();
    if (clean.length !== 6 && clean.length !== 9) return null;
    const hex = (c) => parseInt(c, 16);
    // 6-char: new_data-address (encoded)
    // Encoding: ABCDEF -> new=ADB, addr=EFCA (XOR/rotate)
    const n0 = hex(clean[0]), n1 = hex(clean[1]), n2 = hex(clean[2]);
    const n3 = hex(clean[3]), n4 = hex(clean[4]), n5 = hex(clean[5]);
    const newVal = (n0 << 4) | n1;
    const addr = 0x0000 | ((n5 & 0xF) << 12) | ((n2 & 0xF) << 8) | ((n3 & 0xF) << 4) | (n4 & 0xF);
    // Flip bit 12 of address
    const realAddr = (addr ^ 0xF000) & 0x7FFF;
    if (clean.length === 6) {
        return { addr: realAddr, val: newVal, compare: null };
    }
    const n6 = hex(clean[6]), n7 = hex(clean[7]), n8 = hex(clean[8]);
    const compare = (n6 << 4) | n7;
    // Rotate compare: bit 0 of n8 determines rotation
    const realCompare = ((compare >> 2) | ((compare & 3) << 6)) ^ 0xBA;
    return { addr: realAddr, val: newVal, compare: realCompare };
}

function parseGameShark(code) {
    // GB GameShark: 8-char hex AABBCCDD
    // AA = RAM bank (01 = WRAM), BB = value, CCDD = address
    const clean = code.replace(/[\s-]/g, '').toUpperCase();
    if (clean.length !== 8 || !/^[0-9A-F]{8}$/.test(clean)) return null;
    const val = parseInt(clean.substring(2, 4), 16);
    const addrLow = parseInt(clean.substring(4, 6), 16);
    const addrHigh = parseInt(clean.substring(6, 8), 16);
    const addr = (addrHigh << 8) | addrLow;
    return { addr, val };
}

function addCheat(code) {
    if (!emu) return false;
    const gg = parseGameGenie(code);
    if (gg) {
        emu.add_gg_cheat(gg.addr, gg.val, gg.compare !== null ? gg.compare : 0xFF);
        activeCheats.push({ type: 'gg', code, ...gg });
        return true;
    }
    const gs = parseGameShark(code);
    if (gs) {
        activeCheats.push({ type: 'gs', code, ...gs });
        return true;
    }
    return false;
}

function clearAllCheats() {
    if (emu) emu.clear_cheats();
    activeCheats = [];
}

// --- Cheat database (loaded from cheats.json) ---

let cheatDb = null;

async function loadCheatDb() {
    if (cheatDb !== null) return;
    try {
        const resp = await fetch('cheats.json');
        if (resp.ok) cheatDb = await resp.json();
        else cheatDb = {};
    } catch {
        cheatDb = {};
    }
}

function populateCheatsUI(title) {
    const section = document.getElementById('cheats-section');
    const list = document.getElementById('cheats-list');
    list.replaceChildren();

    if (!cheatDb || !title) { section.style.display = 'none'; return; }

    const key = title.toUpperCase();
    const entry = cheatDb[key];
    if (!entry || !entry.cheats || entry.cheats.length === 0) {
        section.style.display = 'none';
        return;
    }

    section.style.display = '';
    for (const cheat of entry.cheats) {
        const item = document.createElement('label');
        item.className = 'cheat-item';
        const cb = document.createElement('input');
        cb.type = 'checkbox';
        cb.addEventListener('change', () => {
            if (cb.checked) {
                addCheat(cheat.code);
            } else {
                // Remove this cheat — rebuild all active cheats
                activeCheats = activeCheats.filter(c => c.code !== cheat.code);
                if (emu) {
                    emu.clear_cheats();
                    for (const c of activeCheats) {
                        if (c.type === 'gg') {
                            emu.add_gg_cheat(c.addr, c.val, c.compare !== null ? c.compare : 0xFF);
                        }
                    }
                }
            }
        });
        const span = document.createElement('span');
        span.textContent = cheat.desc;
        item.appendChild(cb);
        item.appendChild(span);
        list.appendChild(item);
    }
}

function applyGameSharkCheats() {
    if (!emu) return;
    for (const cheat of activeCheats) {
        if (cheat.type === 'gs') {
            emu.poke_byte(cheat.addr, cheat.val);
        }
    }
}

// --- Video recording (WebM via MediaRecorder) ---

let mediaRecorder = null;
let recordedChunks = [];

function startRecording() {
    if (mediaRecorder && mediaRecorder.state === 'recording') return;
    const stream = canvas.captureStream(60);
    // Add audio if available
    if (audioCtx && audioCtx.state === 'running') {
        const dest = audioCtx.createMediaStreamDestination();
        if (gainNode) gainNode.connect(dest);
        for (const track of dest.stream.getAudioTracks()) {
            stream.addTrack(track);
        }
    }
    recordedChunks = [];
    mediaRecorder = new MediaRecorder(stream, { mimeType: 'video/webm; codecs=vp9' });
    mediaRecorder.ondataavailable = (e) => {
        if (e.data.size > 0) recordedChunks.push(e.data);
    };
    mediaRecorder.onstop = () => {
        const blob = new Blob(recordedChunks, { type: 'video/webm' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `rugb-${(emu ? emu.title() : 'recording').replace(/[^a-zA-Z0-9]/g, '_')}.webm`;
        a.click();
        URL.revokeObjectURL(url);
        recordedChunks = [];
    };
    mediaRecorder.start();
    showToast('Recording started');
}

function stopRecording() {
    if (mediaRecorder && mediaRecorder.state === 'recording') {
        mediaRecorder.stop();
        showToast('Recording saved');
    }
}

function toggleRecording() {
    if (mediaRecorder && mediaRecorder.state === 'recording') {
        stopRecording();
    } else {
        startRecording();
    }
}

// --- RTC time override (F8 prompts for hour offset) ---

function promptRtcOffset() {
    if (!emu) { showToast('Load a ROM first'); return; }
    const input = prompt('Set RTC offset in hours (e.g. +6, -3, 12):');
    if (input === null) return;
    const hours = parseFloat(input);
    if (isNaN(hours)) { showToast('Invalid number'); return; }
    emu.set_rtc_offset(Math.round(hours * 3600));
    showToast(`RTC offset: ${hours > 0 ? '+' : ''}${hours}h`);
}

// --- Shareable state links ---

function compressState(data) {
    // Use deflate-raw via CompressionStream
    const cs = new CompressionStream('deflate-raw');
    const writer = cs.writable.getWriter();
    writer.write(data);
    writer.close();
    return new Response(cs.readable).arrayBuffer().then(b => new Uint8Array(b));
}

function decompressState(data) {
    const ds = new DecompressionStream('deflate-raw');
    const writer = ds.writable.getWriter();
    writer.write(data);
    writer.close();
    return new Response(ds.readable).arrayBuffer().then(b => new Uint8Array(b));
}

async function generateShareLink() {
    if (!emu && !worker) { showToast('No emulator running'); return; }
    let state;
    if (worker) {
        const data = await workerSaveState();
        if (!data) { showToast('Failed to save state'); return; }
        state = new Uint8Array(data);
    } else {
        state = emu.save_state();
    }
    const compressed = await compressState(state);
    // Base64url encode (chunked to avoid call stack overflow on large states)
    let raw = '';
    for (let i = 0; i < compressed.length; i += 8192) {
        raw += String.fromCharCode(...compressed.subarray(i, i + 8192));
    }
    let b64 = btoa(raw);
    b64 = b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    const url = `${location.origin}${location.pathname}#state=${b64}`;
    if (navigator.clipboard) {
        await navigator.clipboard.writeText(url);
        showToast('Share link copied to clipboard');
    } else {
        prompt('Share link:', url);
    }
}

async function loadShareLink() {
    const hash = location.hash;
    if (!hash.startsWith('#state=')) return false;
    const b64 = hash.slice(7).replace(/-/g, '+').replace(/_/g, '/');
    if (b64.length > 2 * 1024 * 1024) return false;
    const padded = b64 + '='.repeat((4 - b64.length % 4) % 4);
    try {
        const binary = atob(padded);
        const compressed = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) compressed[i] = binary.charCodeAt(i);
        const state = await decompressState(compressed);
        return state;
    } catch {
        return null;
    }
}

// --- ROM library (IndexedDB) ---

const DB_NAME = 'rugb-roms';
const DB_STORE = 'roms';

function openRomDb() {
    return new Promise((resolve, reject) => {
        const req = indexedDB.open(DB_NAME, 1);
        req.onupgradeneeded = () => {
            req.result.createObjectStore(DB_STORE, { keyPath: 'title' });
        };
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

async function saveRomToLibrary(title, data) {
    try {
        const db = await openRomDb();
        const tx = db.transaction(DB_STORE, 'readwrite');
        tx.objectStore(DB_STORE).put({ title, data: new Uint8Array(data), savedAt: Date.now() });
        await new Promise((res, rej) => { tx.oncomplete = res; tx.onerror = rej; });
        db.close();
    } catch {}
}

async function loadRomFromLibrary(title) {
    try {
        const db = await openRomDb();
        const tx = db.transaction(DB_STORE, 'readonly');
        const req = tx.objectStore(DB_STORE).get(title);
        const result = await new Promise((res, rej) => { req.onsuccess = () => res(req.result); req.onerror = rej; });
        db.close();
        return result ? result.data.buffer : null;
    } catch { return null; }
}

async function listRomLibrary() {
    try {
        const db = await openRomDb();
        const tx = db.transaction(DB_STORE, 'readonly');
        const req = tx.objectStore(DB_STORE).getAll();
        const result = await new Promise((res, rej) => { req.onsuccess = () => res(req.result); req.onerror = rej; });
        db.close();
        return result;
    } catch { return []; }
}

async function deleteRomFromLibrary(title) {
    try {
        const db = await openRomDb();
        const tx = db.transaction(DB_STORE, 'readwrite');
        tx.objectStore(DB_STORE).delete(title);
        await new Promise((res, rej) => { tx.oncomplete = res; tx.onerror = rej; });
        db.close();
    } catch {}
}

// Save ROM to library after successful load + apply shared state + load cheats
const origStartEmulator = startEmulator;
startEmulator = async function(bytes) {
    await origStartEmulator(bytes);
    if (emu || worker) {
        let title = '';
        if (emu) {
            title = emu.title() || '';
        } else {
            // Extract title from ROM header for worker mode
            const u8 = new Uint8Array(bytes);
            for (let i = 0x134; i < 0x144 && i < u8.length; i++) {
                if (u8[i] === 0) break;
                title += String.fromCharCode(u8[i]);
            }
            title = sanitizeTitle(title);
        }
        if (title) saveRomToLibrary(title, bytes);
        await loadCheatDb();
        populateCheatsUI(title);
        if (pendingSharedState) {
            if (worker) {
                worker.postMessage({ cmd: 'load_state', data: pendingSharedState });
            } else {
                emu.load_state(pendingSharedState);
            }
            pendingSharedState = null;
            location.hash = '';
            showToast('Loaded shared state');
        }
    }
};

// Make recent ROM list clickable (loads from IndexedDB)
const recentRomsEl2 = document.getElementById('recent-roms');
recentRomsEl2.addEventListener('click', async (e) => {
    const el = e.target.closest('.recent-rom');
    if (!el) return;
    const title = el.textContent;
    const data = await loadRomFromLibrary(title);
    if (data) {
        startEmulator(data);
    } else {
        showToast('ROM not in library — load the file again');
    }
});

// Load shared state from URL after emulator starts
let pendingSharedState = null;
(async () => {
    const state = await loadShareLink();
    if (state) pendingSharedState = state;
})();

// Register service worker for PWA / offline support
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('sw.js').catch(() => {});
}

// --- Debug Tools UI ---
{
    const debugBtn = document.getElementById('debug-btn');
    const debugOverlay = document.getElementById('debug-overlay');
    const debugCloseBtn = document.getElementById('debug-close');
    const debugStepBtn = document.getElementById('debug-step');
    const debugAutoChk = document.getElementById('debug-auto');
    const debugTabs = document.querySelectorAll('.debug-tab');

    if (debugBtn) {
        debugBtn.addEventListener('click', () => {
            sideMenu.classList.remove('open');
            debugOverlay.classList.add('visible');
            if (debugTools) debugTools.update();
        });
    }

    if (debugCloseBtn) {
        debugCloseBtn.addEventListener('click', () => {
            debugOverlay.classList.remove('visible');
            if (debugTools) debugTools.autoUpdate = false;
            if (debugAutoChk) debugAutoChk.checked = false;
        });
    }

    if (debugStepBtn) {
        debugStepBtn.addEventListener('click', () => {
            if (!emu) return;
            emu.step();
            if (debugTools) debugTools.update();
        });
    }

    if (debugAutoChk) {
        debugAutoChk.addEventListener('change', () => {
            if (debugTools) debugTools.autoUpdate = debugAutoChk.checked;
        });
    }

    debugTabs.forEach(tab => {
        tab.addEventListener('click', () => {
            debugTabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');
            if (debugTools) {
                debugTools.activeTab = tab.dataset.tab;
                debugTools.update();
            }
        });
    });
}

// --- Link cable UI ---
const linkBtn = document.getElementById('link-btn');
const linkOverlay = document.getElementById('link-overlay');
const linkStatus = document.getElementById('link-status');
const linkModeSelect = document.getElementById('link-mode-select');
const linkLocalPanel = document.getElementById('link-local-panel');
const linkWebrtcPanel = document.getElementById('link-webrtc-panel');
const linkDisconnectBtn = document.getElementById('link-disconnect');
const linkBackBtn = document.getElementById('link-back');
const linkCodeDisplay = document.getElementById('link-code-display');
const linkRoomCode = document.getElementById('link-room-code');

// State tracking for WebRTC SDP flow
let linkWebrtcStep = null; // 'offer-created' | 'answer-created' | null

function showLinkPanel(panel) {
    linkModeSelect.style.display = 'none';
    linkLocalPanel.style.display = 'none';
    linkWebrtcPanel.style.display = 'none';
    linkBackBtn.style.display = 'none';
    if (panel === 'mode') {
        linkModeSelect.style.display = '';
    } else if (panel === 'local') {
        linkLocalPanel.style.display = '';
        linkBackBtn.style.display = '';
    } else if (panel === 'webrtc') {
        linkWebrtcPanel.style.display = '';
        linkBackBtn.style.display = '';
    }
}

function updateLinkUI(status, data) {
    linkStatus.className = 'link-status';
    linkDisconnectBtn.style.display = 'none';
    linkCodeDisplay.style.display = 'none';

    const sdpDisplay = document.getElementById('link-sdp-display');
    const sdpOutput = document.getElementById('link-sdp-output');
    const sdpLabel = document.getElementById('link-sdp-label');

    switch (status) {
        case 'waiting-local':
            linkStatus.textContent = 'Waiting for player...';
            linkStatus.classList.add('waiting');
            linkCodeDisplay.style.display = '';
            linkRoomCode.textContent = data;
            linkDisconnectBtn.style.display = '';
            break;
        case 'offer-ready':
            linkStatus.textContent = 'Offer created — share the string below';
            linkStatus.classList.add('waiting');
            if (sdpDisplay) sdpDisplay.style.display = '';
            if (sdpOutput) sdpOutput.value = data;
            if (sdpLabel) sdpLabel.textContent = 'Send this to your friend:';
            linkWebrtcStep = 'offer-created';
            linkDisconnectBtn.style.display = '';
            break;
        case 'answer-ready':
            linkStatus.textContent = 'Answer created — send this back to the host';
            linkStatus.classList.add('waiting');
            if (sdpDisplay) sdpDisplay.style.display = '';
            if (sdpOutput) sdpOutput.value = data;
            if (sdpLabel) sdpLabel.textContent = 'Send this back to the host:';
            linkWebrtcStep = 'answer-created';
            linkDisconnectBtn.style.display = '';
            break;
        case 'connecting':
            linkStatus.textContent = 'Connecting...';
            linkStatus.classList.add('waiting');
            linkDisconnectBtn.style.display = '';
            break;
        case 'connected':
            linkStatus.textContent = 'Connected!';
            linkStatus.classList.add('connected');
            linkDisconnectBtn.style.display = '';
            if (sdpDisplay) sdpDisplay.style.display = 'none';
            break;
        case 'peer-disconnected':
            linkStatus.textContent = 'Peer disconnected';
            linkWebrtcStep = null;
            break;
        case 'disconnected':
        default:
            linkStatus.textContent = 'Disconnected';
            linkWebrtcStep = null;
            if (sdpDisplay) sdpDisplay.style.display = 'none';
            showLinkPanel('mode');
            break;
    }
}

if (linkBtn) {
    linkBtn.addEventListener('click', () => {
        if (!linkCable || !linkCable.connected) showLinkPanel('mode');
        linkOverlay.classList.add('visible');
    });
}

// Mode selection
document.getElementById('link-mode-local')?.addEventListener('click', () => {
    showLinkPanel('local');
});
document.getElementById('link-mode-webrtc')?.addEventListener('click', () => {
    showLinkPanel('webrtc');
    linkWebrtcStep = null;
    const sdpDisplay = document.getElementById('link-sdp-display');
    if (sdpDisplay) sdpDisplay.style.display = 'none';
    const sdpInput = document.getElementById('link-sdp-input');
    if (sdpInput) sdpInput.value = '';
});

// Local mode
document.getElementById('link-local-host')?.addEventListener('click', () => {
    if (!emu || currentSystem === 'gba') return;
    if (linkCable) linkCable.disconnect();
    linkCable = new LinkCable(emu, updateLinkUI);
    linkCable.hostLocal();
});
document.getElementById('link-local-join')?.addEventListener('click', () => {
    if (!emu || currentSystem === 'gba') return;
    const code = document.getElementById('link-local-input')?.value?.trim();
    if (!code || code.length < 4) return;
    if (linkCable) linkCable.disconnect();
    linkCable = new LinkCable(emu, updateLinkUI);
    linkCable.joinLocal(code);
});

// WebRTC mode
document.getElementById('link-create-offer')?.addEventListener('click', async () => {
    if (!emu || currentSystem === 'gba') return;
    if (linkCable) linkCable.disconnect();
    linkCable = new LinkCable(emu, updateLinkUI);
    linkStatus.textContent = 'Generating offer...';
    await linkCable.createOffer();
});

document.getElementById('link-copy-sdp')?.addEventListener('click', () => {
    const sdpOutput = document.getElementById('link-sdp-output');
    if (sdpOutput) {
        navigator.clipboard.writeText(sdpOutput.value).then(
            () => { sdpOutput.select(); },
            () => { sdpOutput.select(); sdpOutput.focus(); }
        );
    }
});

document.getElementById('link-accept-sdp')?.addEventListener('click', async () => {
    if (!emu || currentSystem === 'gba') return;
    const sdpInput = document.getElementById('link-sdp-input');
    const value = sdpInput?.value?.trim();
    if (!value) return;

    try {
        if (!linkCable || !linkWebrtcStep) {
            // Guest: accepting an offer
            if (linkCable) linkCable.disconnect();
            linkCable = new LinkCable(emu, updateLinkUI);
            await linkCable.acceptOffer(value);
        } else if (linkWebrtcStep === 'offer-created') {
            // Host: accepting the answer
            await linkCable.completeConnection(value);
        }
        if (sdpInput) sdpInput.value = '';
    } catch (e) {
        linkStatus.textContent = 'Invalid connection string';
        linkStatus.className = 'link-status';
    }
});

// Common controls
if (linkDisconnectBtn) {
    linkDisconnectBtn.addEventListener('click', () => {
        if (linkCable) linkCable.disconnect();
        linkCable = null;
        linkWebrtcStep = null;
    });
}

if (linkBackBtn) {
    linkBackBtn.addEventListener('click', () => {
        if (linkCable && !linkCable.connected) {
            linkCable.disconnect();
            linkCable = null;
        }
        linkWebrtcStep = null;
        showLinkPanel('mode');
        linkStatus.textContent = 'Disconnected';
        linkStatus.className = 'link-status';
    });
}

document.querySelectorAll('.link-close-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        linkOverlay.classList.remove('visible');
    });
});

// --- Cloud Saves ---

let cloudSaves = null;
const cloudBtn = document.getElementById('cloud-btn');
const cloudOverlay = document.getElementById('cloud-overlay');
const cloudStatus = document.getElementById('cloud-status');
const cloudSignIn = document.getElementById('cloud-sign-in');
const cloudSync = document.getElementById('cloud-sync');
const cloudSignOut = document.getElementById('cloud-sign-out');
const cloudClose = document.getElementById('cloud-close');
const cloudList = document.getElementById('cloud-saves-list');

function updateCloudUI() {
    if (!cloudSaves) return;
    const signedIn = cloudSaves.isSignedIn();
    cloudStatus.textContent = signedIn ? 'Connected to Google Drive' : 'Not connected';
    cloudSignIn.style.display = signedIn ? 'none' : '';
    cloudSync.style.display = signedIn ? '' : 'none';
    cloudSignOut.style.display = signedIn ? '' : 'none';
}

async function refreshCloudList() {
    if (!cloudSaves || !cloudSaves.isSignedIn()) { cloudList.innerHTML = ''; return; }
    try {
        const files = await cloudSaves.listSaves();
        cloudList.innerHTML = '';
        if (files.length === 0) {
            cloudList.innerHTML = '<p style="color:#666;font-size:0.85rem">No cloud saves yet</p>';
            return;
        }
        for (const f of files) {
            const div = document.createElement('div');
            div.className = 'cloud-save-item';
            const props = f.appProperties || {};
            const label = props.gameTitle ? `${f.name} (${props.gameTitle})` : f.name;
            div.innerHTML = `<span>${label}</span><span style="color:#666">${new Date(f.modifiedTime).toLocaleDateString()}</span>`;
            cloudList.appendChild(div);
        }
    } catch { cloudList.innerHTML = '<p style="color:#c66">Failed to load cloud saves</p>'; }
}

if (cloudBtn) {
    cloudBtn.addEventListener('click', async () => {
        cloudOverlay.classList.add('visible');
        if (!cloudSaves) {
            cloudSaves = new CloudSaves();
            try {
                cloudStatus.textContent = 'Loading...';
                await cloudSaves.init();
            } catch {
                cloudStatus.textContent = 'Failed to load Google API';
                return;
            }
        }
        updateCloudUI();
        refreshCloudList();
    });
}

if (cloudSignIn) {
    cloudSignIn.addEventListener('click', async () => {
        if (!cloudSaves) return;
        cloudStatus.textContent = 'Signing in...';
        await cloudSaves.signIn();
        updateCloudUI();
        refreshCloudList();
    });
}

if (cloudSync) {
    cloudSync.addEventListener('click', async () => {
        if (!cloudSaves || !cloudSaves.isSignedIn()) return;
        cloudSync.disabled = true;
        cloudSync.textContent = 'Syncing...';
        try {
            const result = await cloudSaves.syncAll();
            showToast(`Sync complete: ${result.uploaded} up, ${result.downloaded} down`);
            refreshCloudList();
        } catch {
            showToast('Sync failed');
        }
        cloudSync.disabled = false;
        cloudSync.textContent = 'Sync Now';
    });
}

if (cloudSignOut) {
    cloudSignOut.addEventListener('click', () => {
        if (!cloudSaves) return;
        cloudSaves.signOut();
        updateCloudUI();
        cloudList.innerHTML = '';
        showToast('Signed out from Google Drive');
    });
}

if (cloudClose) {
    cloudClose.addEventListener('click', () => {
        cloudOverlay.classList.remove('visible');
    });
}

// --- ROM Library UI ---

const libraryBtn = document.getElementById('library-btn');
const libraryOverlay = document.getElementById('library-overlay');
const librarySearch = document.getElementById('library-search');
const libraryGrid = document.getElementById('library-grid');
const libraryClose = document.getElementById('library-close');

async function renderLibrary(filter = '') {
    const roms = await listRomLibrary();
    libraryGrid.innerHTML = '';
    const filtered = filter
        ? roms.filter(r => r.title.toLowerCase().includes(filter.toLowerCase()))
        : roms;
    if (filtered.length === 0) {
        libraryGrid.innerHTML = `<div class="library-empty">${filter ? 'No matches' : 'No ROMs saved yet — load a ROM to add it'}</div>`;
        return;
    }
    // Sort by most recently saved
    filtered.sort((a, b) => (b.savedAt || 0) - (a.savedAt || 0));
    for (const rom of filtered) {
        const item = document.createElement('div');
        item.className = 'library-item';
        const date = rom.savedAt ? new Date(rom.savedAt).toLocaleDateString() : '';
        item.innerHTML = `
            <span class="library-item-title">${rom.title}</span>
            <span class="library-item-date">${date}</span>
            <button class="library-item-delete" title="Delete">&times;</button>
        `;
        item.querySelector('.library-item-title').addEventListener('click', async () => {
            const data = await loadRomFromLibrary(rom.title);
            if (data) {
                libraryOverlay.classList.remove('visible');
                startEmulator(data);
            } else {
                showToast('Failed to load ROM');
            }
        });
        item.querySelector('.library-item-delete').addEventListener('click', async (e) => {
            e.stopPropagation();
            await deleteRomFromLibrary(rom.title);
            renderLibrary(librarySearch.value);
            showToast(`Deleted ${rom.title}`);
        });
        libraryGrid.appendChild(item);
    }
}

if (libraryBtn) {
    libraryBtn.addEventListener('click', () => {
        libraryOverlay.classList.add('visible');
        librarySearch.value = '';
        renderLibrary();
        librarySearch.focus();
    });
}
if (librarySearch) {
    librarySearch.addEventListener('input', () => renderLibrary(librarySearch.value));
}
if (libraryClose) {
    libraryClose.addEventListener('click', () => libraryOverlay.classList.remove('visible'));
}

// --- Per-game settings persistence ---

function getGameSettingsKey() {
    if (emu && emu.title) return `rugb-game-${emu.title()}`;
    if (romBytes) {
        const u8 = new Uint8Array(romBytes);
        let t = '';
        for (let i = 0x134; i < 0x144 && i < u8.length; i++) {
            if (u8[i] === 0) break;
            t += String.fromCharCode(u8[i]);
        }
        return `rugb-game-${sanitizeTitle(t)}`;
    }
    return null;
}

function saveGameSettings() {
    const key = getGameSettingsKey();
    if (!key) return;
    const settings = {
        palette: currentPalette,
        filter: localStorage.getItem('rugb-filter') || 'none',
        speed,
    };
    try { localStorage.setItem(key, JSON.stringify(settings)); } catch {}
}

function loadGameSettings() {
    const key = getGameSettingsKey();
    if (!key) return;
    const raw = localStorage.getItem(key);
    if (!raw) return;
    try {
        const s = JSON.parse(raw);
        // Apply palette
        if (s.palette && PALETTES[s.palette]) {
            document.querySelector(`.palette-btn[data-palette="${s.palette}"]`)?.click();
        }
        // Apply filter
        if (s.filter) {
            document.querySelector(`.filter-btn[data-filter="${s.filter}"]`)?.click();
        }
        // Apply speed
        if (s.speed) {
            speed = s.speed;
            document.querySelector(`.speed-btn[data-speed="${s.speed}"]`)?.classList.add('active');
        }
    } catch {}
}

// Hook into startEmulator to load/save per-game settings
const origStartEmulator2 = startEmulator;
startEmulator = async function(bytes) {
    // Save current game's settings before switching
    saveGameSettings();
    await origStartEmulator2(bytes);
    // Load new game's settings
    loadGameSettings();
};

// Save settings periodically when a game is running
setInterval(() => { if (emu || worker) saveGameSettings(); }, 10000);

// --- Dark/light theme toggle ---

const themeToggle = document.getElementById('theme-toggle');
const savedTheme = localStorage.getItem('rugb-theme');
if (savedTheme === 'light') {
    document.documentElement.classList.add('light-theme');
    if (themeToggle) themeToggle.checked = true;
}
if (themeToggle) {
    themeToggle.addEventListener('change', () => {
        document.documentElement.classList.toggle('light-theme', themeToggle.checked);
        localStorage.setItem('rugb-theme', themeToggle.checked ? 'light' : 'dark');
    });
}

// --- Input HUD ---

const inputHud = document.getElementById('input-hud');
const hudToggle = document.getElementById('hud-toggle');
const hudBtns = inputHud ? inputHud.querySelectorAll('.hud-btn') : [];
let hudEnabled = false;

if (hudToggle) {
    const savedHud = localStorage.getItem('rugb-hud') === 'true';
    hudEnabled = savedHud;
    hudToggle.checked = savedHud;
    if (inputHud) inputHud.style.display = savedHud ? '' : 'none';

    hudToggle.addEventListener('change', () => {
        hudEnabled = hudToggle.checked;
        localStorage.setItem('rugb-hud', hudEnabled);
        if (inputHud) inputHud.style.display = hudEnabled ? '' : 'none';
    });
}

// Update HUD button states from the emulator's button state
function updateHud(btn, pressed) {
    if (!hudEnabled) return;
    hudBtns.forEach(el => {
        if (parseInt(el.dataset.hud) === btn) {
            el.classList.toggle('active', pressed);
        }
    });
}

// Patch emuSetButton to also update HUD
const _origEmuSetButton = emuSetButton;
emuSetButton = function(btn, pressed) {
    updateHud(btn, pressed);
    _origEmuSetButton(btn, pressed);
};

console.log('RUGB ready — load a ROM to start');
