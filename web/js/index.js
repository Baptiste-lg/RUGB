import init, { WasmEmulator } from '../pkg/rugb.js';

let emu = null;
let animationId = null;
let paused = false;
let romBytes = null;
let wasm = null;
let speed = 1;
let muted = false;
let fastForward = false;
let normalSpeed = 1;
let turboA = false;
let turboB = false;
let turboFrame = 0;

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
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

// --- Side menu toggle ---
menuToggle.addEventListener('click', () => sideMenu.classList.toggle('open'));

// --- View toggle (Game Boy / Screen Only) ---
const savedView = localStorage.getItem('rugb-view') || 'gb';
if (savedView === 'screen') {
    gameboy.classList.add('screen-only');
    viewGbBtn.classList.remove('active');
    viewScreenBtn.classList.add('active');
}

viewGbBtn.addEventListener('click', () => {
    gameboy.classList.remove('screen-only');
    viewGbBtn.classList.add('active');
    viewScreenBtn.classList.remove('active');
    localStorage.setItem('rugb-view', 'gb');
});

viewScreenBtn.addEventListener('click', () => {
    gameboy.classList.add('screen-only');
    viewScreenBtn.classList.add('active');
    viewGbBtn.classList.remove('active');
    localStorage.setItem('rugb-view', 'screen');
});

// --- Resize observer: keep --gb-w in sync with actual width ---
const resizeObs = new ResizeObserver(entries => {
    for (const entry of entries) {
        const box = entry.borderBoxSize?.[0];
        const w = box ? box.inlineSize : entry.target.offsetWidth;
        gameboy.style.setProperty('--gb-w', w + 'px');
    }
});
resizeObs.observe(gameboy);

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
let audioProcessor = null;
let gainNode = null;
const AUDIO_SAMPLE_RATE = 48000;

function initAudio() {
    if (audioCtx) return;
    audioCtx = new AudioContext({ sampleRate: AUDIO_SAMPLE_RATE });
    gainNode = audioCtx.createGain();
    gainNode.gain.value = parseInt(document.getElementById('volume-slider').value) / 100;
    gainNode.connect(audioCtx.destination);
    const bufferSize = 2048;
    audioProcessor = audioCtx.createScriptProcessor(bufferSize, 0, 2);
    audioProcessor.onaudioprocess = (e) => {
        const left = e.outputBuffer.getChannelData(0);
        const right = e.outputBuffer.getChannelData(1);
        if (!emu || muted) {
            left.fill(0);
            right.fill(0);
            return;
        }
        let len = emu.audio_buffer_len();
        let samplePairs = Math.floor(len / 2);

        // If buffer has built up too much latency (>80ms), skip ahead
        const maxLatency = AUDIO_SAMPLE_RATE * 0.08;
        if (samplePairs > maxLatency) {
            const skip = samplePairs - left.length;
            if (skip > 0) {
                emu.audio_buffer_consume(skip * 2);
                samplePairs = left.length;
            }
        }

        const count = Math.min(samplePairs, left.length);
        if (count > 0) {
            const ptr = emu.audio_buffer_ptr();
            const samples = new Float32Array(wasm.memory.buffer, ptr, count * 2);
            for (let i = 0; i < count; i++) {
                left[i] = samples[i * 2];
                right[i] = samples[i * 2 + 1];
            }
        }
        // Fill remaining with silence
        for (let i = count; i < left.length; i++) {
            left[i] = 0;
            right[i] = 0;
        }
        // Only consume what was actually read (keep excess for next callback)
        emu.audio_buffer_consume(count * 2);
    };
    audioProcessor.connect(gainNode);
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

function applyPalette(imageData) {
    if (currentPalette === 'gray') return imageData; // Default shades match gray
    const pal = PALETTES[currentPalette];
    if (!pal) return imageData;

    // Map shade values (0xFF, 0xAA, 0x55, 0x00) to palette colors
    const shadeMap = {};
    [0xFF, 0xAA, 0x55, 0x00].forEach((shade, i) => {
        const hex = pal.colors[i];
        shadeMap[shade] = [
            parseInt(hex.slice(1, 3), 16),
            parseInt(hex.slice(3, 5), 16),
            parseInt(hex.slice(5, 7), 16),
        ];
    });

    const data = imageData.data;
    for (let i = 0; i < data.length; i += 4) {
        const rgb = shadeMap[data[i]];
        if (rgb) {
            data[i] = rgb[0];
            data[i + 1] = rgb[1];
            data[i + 2] = rgb[2];
        }
    }
    return imageData;
}

// --- Emulator lifecycle ---

let batterySaveTimer = null;

function saveBatteryRAM() {
    if (!emu || !emu.has_battery()) return;
    try {
        const title = emu.title();
        if (!title) return;
        const len = emu.battery_ram_len();
        if (len === 0) return;
        const ptr = emu.battery_ram_ptr();
        const ram = new Uint8Array(wasm.memory.buffer, ptr, len);
        const b64 = uint8ToBase64(new Uint8Array(ram));
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
    const data = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
    emu.load_battery_ram(data);
}

async function startEmulator(bytes) {
    // Save previous game's battery RAM before loading new one
    saveBatteryRAM();
    if (batterySaveTimer) clearInterval(batterySaveTimer);

    if (!wasm) wasm = await init();
    romBytes = bytes;
    emu = new WasmEmulator(new Uint8Array(bytes));

    const title = emu.title();
    if (title) {
        document.title = `RUGB — ${title}`;
        addRecentRom(title);
    }

    // Restore battery-backed SRAM from localStorage
    loadBatteryRAM();

    pauseBtn.disabled = false;
    resetBtn.disabled = false;
    muteBtn.disabled = false;
    screenshotBtn.disabled = false;
    paused = false;
    pauseBtn.textContent = 'Pause';

    initAudio();

    // Auto-save battery RAM every 5 seconds
    batterySaveTimer = setInterval(saveBatteryRAM, 5000);

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
                emu.set_button(gbBtn, pressed);
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

            if (left !== prev.axL) { emu.set_button(1, left); prev.axL = left; }
            if (right !== prev.axR) { emu.set_button(0, right); prev.axR = right; }
            if (up !== prev.axU) { emu.set_button(2, up); prev.axU = up; }
            if (down !== prev.axD) { emu.set_button(3, down); prev.axD = down; }
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
        gpStatus.textContent = found.id;
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

// Game Boy frame time: 70224 T-cycles at 4.194304 MHz ≈ 16.74ms
const GB_FRAME_MS = 70224 / 4194304 * 1000;
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
    if (delta > 100) delta = GB_FRAME_MS;

    const effectiveSpeed = fastForward ? 16 : speed;
    frameDebt += delta * effectiveSpeed;

    let framesRun = 0;
    const maxFrames = fastForward ? 32 : Math.max(4, 4 * speed);
    while (frameDebt >= GB_FRAME_MS && framesRun < maxFrames) {
        pollGamepad();
        // Turbo: toggle A/B every other frame
        turboFrame++;
        const turboOn = (turboFrame & 1) === 0;
        if (turboA) emu.set_button(4, turboOn);
        if (turboB) emu.set_button(5, turboOn);
        emu.run_frame();
        frameDebt -= GB_FRAME_MS;
        framesRun++;
    }
    // Prevent debt accumulation during fast forward
    if (fastForward && frameDebt > GB_FRAME_MS * 4) frameDebt = 0;

    if (framesRun > 0) {
        const ptr = emu.framebuffer_ptr();
        const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, 160 * 144 * 4);
        let imageData = new ImageData(new Uint8ClampedArray(pixels), 160, 144);
        imageData = applyPalette(imageData);
        ctx.putImageData(imageData, 0, 0);
    }

    animationId = requestAnimationFrame(frame);
}

// --- ROM loading ---

romInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (file) loadFile(file);
});

function loadFile(file) {
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
    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
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
    if (!paused) {
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
});

// Display filters
const filterBtns = document.querySelectorAll('.filter-btn');
const screenFrame = document.querySelector('.gb-screen-frame');
filterBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        const filter = btn.dataset.filter;
        filterBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        // Remove all filter classes
        canvas.classList.remove('filter-smooth');
        screenFrame.classList.remove('filter-scanlines', 'filter-lcd');
        // Apply selected
        if (filter === 'smooth') canvas.classList.add('filter-smooth');
        if (filter === 'scanlines') screenFrame.classList.add('filter-scanlines');
        if (filter === 'lcd') screenFrame.classList.add('filter-lcd');
        localStorage.setItem('rugb-filter', filter);
    });
});
// Restore saved filter
const savedFilter = localStorage.getItem('rugb-filter');
if (savedFilter && savedFilter !== 'none') {
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
        try { return { ...DEFAULT_KEYS, ...JSON.parse(saved) }; } catch {}
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

    if (e.key === 'Escape') { sideMenu.classList.toggle('open'); return; }
    if (e.key === 'F11') { e.preventDefault(); fullscreenBtn.click(); return; }
    if (e.key === ' ') { e.preventDefault(); fastForward = true; return; }
    if (e.key === 'q') { turboA = !turboA; showToast(turboA ? 'Turbo A ON' : 'Turbo A OFF'); return; }
    if (e.key === 'w') { turboB = !turboB; showToast(turboB ? 'Turbo B ON' : 'Turbo B OFF'); return; }
    if (e.key === keyMap.pause) { pauseBtn.click(); return; }
    if (e.key === keyMap.mute) { muteBtn.click(); return; }
    if (e.key === '1') { document.querySelector('.speed-btn[data-speed="1"]')?.click(); return; }
    if (e.key === '2') { document.querySelector('.speed-btn[data-speed="2"]')?.click(); return; }
    if (e.key === '4') { document.querySelector('.speed-btn[data-speed="4"]')?.click(); return; }
    // Quick save/load
    if (emu) {
        if (e.key === keyMap.quicksave) { e.preventDefault(); doQuickSave(); return; }
        if (e.key === keyMap.quickload) { e.preventDefault(); doQuickLoad(); return; }
    }
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined) {
        if (!e.repeat && emu) emu.set_button(btn, true);
        if (btnIndexToEl[btn]) btnIndexToEl[btn].classList.add('pressed');
        e.preventDefault();
    }
});

document.addEventListener('keyup', (e) => {
    if (e.key === ' ') { fastForward = false; return; }
    if (remapListening) return;
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined) {
        if (emu) emu.set_button(btn, false);
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
        if (emu) emu.set_button(gbBtn, true);
    };
    const release = (e) => {
        e.preventDefault();
        if (!el.classList.contains('pressed')) return;
        el.classList.remove('pressed');
        if (emu) emu.set_button(gbBtn, false);
    };
    el.addEventListener('mousedown', press);
    el.addEventListener('mouseup', release);
    el.addEventListener('mouseleave', release);
    el.addEventListener('touchstart', press);
    el.addEventListener('touchend', release);
    el.addEventListener('touchcancel', release);
});

// --- Mobile touch controls ---

document.querySelectorAll('.touch-btn[data-btn]').forEach(el => {
    const gbBtn = parseInt(el.dataset.btn);
    const press = (e) => {
        e.preventDefault();
        el.classList.add('pressed');
        if (emu) emu.set_button(gbBtn, true);
    };
    const release = (e) => {
        e.preventDefault();
        el.classList.remove('pressed');
        if (emu) emu.set_button(gbBtn, false);
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

function doQuickSave() {
    if (!emu) return;
    quickSaveData = emu.save_state();
    showToast('Quick Save');
}

function doQuickLoad() {
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

function saveToSlot(slot) {
    if (!emu) return;
    const data = emu.save_state();
    const timestamp = new Date().toLocaleString();
    const title = emu.title() || '';
    saveSlots[slot] = { data, timestamp, title };
    const b64 = uint8ToBase64(data);
    localStorage.setItem(`rugb-slot-${slot}`, JSON.stringify({ data: b64, timestamp, title }));
    updateSlotUI();
    showToast(`Saved to Slot ${slot}`);
}

function loadFromSlot(slot) {
    if (!emu || !saveSlots[slot]) return;
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
    recentRomsEl.innerHTML = '';
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

// Save battery RAM when leaving the page
window.addEventListener('beforeunload', saveBatteryRAM);

console.log('RUGB ready — load a ROM to start');
