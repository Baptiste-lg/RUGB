import init, { WasmEmulator } from '../pkg/rugb.js';

let emu = null;
let animationId = null;
let paused = false;
let romBytes = null;
let wasm = null;
let speed = 1;
let muted = false;

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const pauseBtn = document.getElementById('pause-btn');
const resetBtn = document.getElementById('reset-btn');
const muteBtn = document.getElementById('mute-btn');
const romInput = document.getElementById('rom-input');
const speedBtns = document.querySelectorAll('.speed-btn');
const paletteBtns = document.querySelectorAll('.palette-btn');
const helpOverlay = document.getElementById('help-overlay');

// --- Audio setup ---

let audioCtx = null;
const oscillators = [null, null]; // CH1, CH2 square wave oscillators
const gains = [null, null];

function initAudio() {
    if (audioCtx) return;
    audioCtx = new AudioContext();
    for (let i = 0; i < 2; i++) {
        const osc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();
        osc.type = 'square';
        osc.frequency.value = 0;
        gain.gain.value = 0;
        osc.connect(gain);
        gain.connect(audioCtx.destination);
        osc.start();
        oscillators[i] = osc;
        gains[i] = gain;
    }
}

function updateAudio() {
    if (!audioCtx || muted || !emu) return;
    for (let ch = 0; ch < 2; ch++) {
        const freq = emu.channel_freq(ch + 1);
        const vol = emu.channel_volume(ch + 1);
        oscillators[ch].frequency.value = freq;
        gains[ch].gain.value = vol * 0.15; // Scale down to avoid clipping
    }
}

// --- Palettes ---

const PALETTES = {
    green:  { name: 'green',  colors: ['#9bbc0f', '#8bac0f', '#306230', '#0f380f'] },
    gray:   { name: 'gray',   colors: ['#ffffff', '#aaaaaa', '#555555', '#000000'] },
    bw:     { name: 'bw',     colors: ['#ffffff', '#b0b0b0', '#404040', '#000000'] },
};

let currentPalette = localStorage.getItem('ob-palette') || 'gray';

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

async function startEmulator(bytes) {
    wasm = await init();
    romBytes = bytes;
    emu = new WasmEmulator(new Uint8Array(bytes));

    const title = emu.title();
    if (title) document.title = `RUGB — ${title}`;

    pauseBtn.disabled = false;
    resetBtn.disabled = false;
    muteBtn.disabled = false;
    paused = false;
    pauseBtn.textContent = 'Pause';

    initAudio();

    if (animationId) cancelAnimationFrame(animationId);
    requestAnimationFrame(frame);
}

function frame() {
    if (paused || !emu) return;

    for (let i = 0; i < speed; i++) {
        emu.run_frame();
    }

    const ptr = emu.framebuffer_ptr();
    const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, 160 * 144 * 4);
    let imageData = new ImageData(new Uint8ClampedArray(pixels), 160, 144);
    imageData = applyPalette(imageData);
    ctx.putImageData(imageData, 0, 0);

    updateAudio();

    animationId = requestAnimationFrame(frame);
}

// --- ROM loading ---

romInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (file) loadFile(file);
});

function loadFile(file) {
    const reader = new FileReader();
    reader.onload = () => startEmulator(reader.result);
    reader.readAsArrayBuffer(file);
}

// Drag & drop
canvas.addEventListener('dragover', (e) => {
    e.preventDefault();
    canvas.classList.add('drag-over');
});
canvas.addEventListener('dragleave', () => canvas.classList.remove('drag-over'));
canvas.addEventListener('drop', (e) => {
    e.preventDefault();
    canvas.classList.remove('drag-over');
    const file = e.dataTransfer.files[0];
    if (file) loadFile(file);
});

// --- Controls ---

pauseBtn.addEventListener('click', () => {
    paused = !paused;
    pauseBtn.textContent = paused ? 'Resume' : 'Pause';
    if (!paused) requestAnimationFrame(frame);
});

resetBtn.addEventListener('click', () => {
    if (romBytes) startEmulator(romBytes);
});

muteBtn.addEventListener('click', () => {
    muted = !muted;
    muteBtn.textContent = muted ? 'Unmute' : 'Mute';
    if (muted && gains[0]) {
        gains[0].gain.value = 0;
        gains[1].gain.value = 0;
    }
});

// Speed buttons
speedBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        speed = parseInt(btn.dataset.speed);
        speedBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
    });
});

// Palette buttons
paletteBtns.forEach(btn => {
    btn.addEventListener('click', () => {
        currentPalette = btn.dataset.palette;
        localStorage.setItem('ob-palette', currentPalette);
        paletteBtns.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
    });
});

// Set initial active palette button
document.querySelector(`.palette-btn[data-palette="${currentPalette}"]`)?.classList.add('active');

// --- Key remapping ---

const ACTIONS = ['right', 'left', 'up', 'down', 'a', 'b', 'start', 'select'];
const ACTION_TO_BTN = { right: 0, left: 1, up: 2, down: 3, a: 4, b: 5, start: 6, select: 7 };
const DEFAULT_KEYS = {
    right: 'ArrowRight', left: 'ArrowLeft', up: 'ArrowUp', down: 'ArrowDown',
    a: 'z', b: 'x', start: 'Enter', select: 'Shift',
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
            alert('Invalid keybinds file');
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

    if (e.key === 'p') { pauseBtn.click(); return; }
    if (e.key === 'm') { muteBtn.click(); return; }
    if (e.key === '?') { helpOverlay.classList.toggle('visible'); return; }
    if (e.key === '1') { document.querySelector('.speed-btn[data-speed="1"]')?.click(); return; }
    if (e.key === '2') { document.querySelector('.speed-btn[data-speed="2"]')?.click(); return; }
    if (e.key === '4') { document.querySelector('.speed-btn[data-speed="4"]')?.click(); return; }
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined && emu) {
        emu.set_button(btn, true);
        e.preventDefault();
    }
});

document.addEventListener('keyup', (e) => {
    if (remapListening) return;
    const btn = BUTTON_MAP[e.key];
    if (btn !== undefined && emu) {
        emu.set_button(btn, false);
        e.preventDefault();
    }
});

// --- Touch controls ---

document.querySelectorAll('.touch-btn').forEach(btn => {
    const gbBtn = parseInt(btn.dataset.btn);
    btn.addEventListener('touchstart', (e) => {
        e.preventDefault();
        if (emu) emu.set_button(gbBtn, true);
    });
    btn.addEventListener('touchend', (e) => {
        e.preventDefault();
        if (emu) emu.set_button(gbBtn, false);
    });
});

console.log('RUGB ready — load a ROM to start');
