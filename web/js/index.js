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
const menuToggle = document.getElementById('menu-toggle');
const sideMenu = document.getElementById('side-menu');
const viewGbBtn = document.getElementById('view-gb');
const viewScreenBtn = document.getElementById('view-screen');
const gameboy = document.querySelector('.gameboy');

// --- Side menu toggle ---
menuToggle.addEventListener('click', () => sideMenu.classList.toggle('open'));
document.addEventListener('click', (e) => {
    if (sideMenu.classList.contains('open') && !sideMenu.contains(e.target) && e.target !== menuToggle) {
        sideMenu.classList.remove('open');
    }
});

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
        const w = entry.contentRect.width;
        gameboy.style.setProperty('--gb-w', w + 'px');
    }
});
resizeObs.observe(gameboy);

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
const ALL_ACTIONS = [...ACTIONS, 'pause', 'mute', 'quicksave', 'quickload'];
const ACTION_TO_BTN = { right: 0, left: 1, up: 2, down: 3, a: 4, b: 5, start: 6, select: 7 };
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
        if (emu) emu.set_button(btn, true);
        if (btnIndexToEl[btn]) btnIndexToEl[btn].classList.add('pressed');
        e.preventDefault();
    }
});

document.addEventListener('keyup', (e) => {
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

function saveToSlot(slot) {
    if (!emu) return;
    const data = emu.save_state();
    const timestamp = new Date().toLocaleString();
    const title = emu.title() || '';
    saveSlots[slot] = { data, timestamp, title };
    // Store as base64 in localStorage
    const b64 = btoa(String.fromCharCode(...data));
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
        const b64 = btoa(String.fromCharCode(...s.data));
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

console.log('RUGB ready — load a ROM to start');
