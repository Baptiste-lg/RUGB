import init, { WasmEmulator } from '../pkg/oxide_boy.js';

let emu = null;
let animationId = null;
let paused = false;
let romBytes = null;
let wasm = null;

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const pauseBtn = document.getElementById('pause-btn');
const resetBtn = document.getElementById('reset-btn');
const romInput = document.getElementById('rom-input');

async function startEmulator(bytes) {
    wasm = await init();
    romBytes = bytes;
    emu = new WasmEmulator(new Uint8Array(bytes));

    const title = emu.title();
    if (title) document.title = `OxideBoy — ${title}`;

    pauseBtn.disabled = false;
    resetBtn.disabled = false;
    paused = false;
    pauseBtn.textContent = 'Pause';

    if (animationId) cancelAnimationFrame(animationId);
    requestAnimationFrame(frame);
}

function frame() {
    if (paused || !emu) return;

    emu.run_frame();

    const ptr = emu.framebuffer_ptr();
    const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, 160 * 144 * 4);
    const imageData = new ImageData(pixels, 160, 144);
    ctx.putImageData(imageData, 0, 0);

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

// --- Keyboard ---

const BUTTON_MAP = {
    'ArrowRight': 0, 'ArrowLeft': 1, 'ArrowUp': 2, 'ArrowDown': 3,
    'z': 4, 'x': 5, 'Enter': 6, 'Shift': 7,
};

document.addEventListener('keydown', (e) => {
    if (e.key === 'p') { pauseBtn.click(); return; }
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

console.log('OxideBoy ready — load a ROM to start');
