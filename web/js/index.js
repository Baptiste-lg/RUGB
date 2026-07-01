import init, { WasmEmulator } from '../pkg/oxide_boy.js';

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
    if (title) document.title = `OxideBoy — ${title}`;

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

// --- Keyboard ---

const BUTTON_MAP = {
    'ArrowRight': 0, 'ArrowLeft': 1, 'ArrowUp': 2, 'ArrowDown': 3,
    'z': 4, 'x': 5, 'Enter': 6, 'Shift': 7,
};

document.addEventListener('keydown', (e) => {
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

console.log('OxideBoy ready — load a ROM to start');
