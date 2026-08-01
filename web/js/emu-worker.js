/**
 * Web Worker for RUGB — runs WASM emulation off the main thread.
 * Renders to an OffscreenCanvas via WebGL (or posts raw pixels as fallback).
 * Communicates with main thread via a message protocol.
 */

import { WebGLRenderer } from './webgl-renderer.js';

let emu = null;
let wasm = null;
let system = null; // 'gb', 'gbc', or 'gba'
let glRenderer = null;
let offscreen = null;

// Frame timing
let running = false;
let speed = 1;
let fastForward = false;
let muted = false;

const AUDIO_SAMPLE_RATE = 48000;
const GB_FRAME_MS = 70224 / 4194304 * 1000;
const GBA_FRAME_MS = 280896 / 16777216 * 1000;

// Rewind state
const REWIND_MAX_FRAMES = 300;
const REWIND_CAPTURE_INTERVAL = 4;
let rewindBuffer = [];
let rewindFrameCounter = 0;
let rewinding = false;

// Turbo
let turboA = false;
let turboB = false;
let turboFrame = 0;

// GameShark cheats
let activeCheats = [];

let lastTick = 0;
let frameDebt = 0;

/* ── WASM init ────────────────────────────────────────────────────── */

async function initWasm(romBytes, sys, bootRom) {
    system = sys;
    const isGba = sys === 'gba';
    const screenW = isGba ? 240 : 160;
    const screenH = isGba ? 160 : 144;

    if (isGba) {
        const { default: initGba, WasmGbaEmulator } = await import('../pkg/rugba/rugba.js');
        wasm = await initGba();
        emu = new WasmGbaEmulator(new Uint8Array(romBytes));
    } else {
        const { default: initGb, WasmEmulator } = await import('../pkg/rugb/rugb.js');
        wasm = await initGb();
        if (bootRom) {
            emu = WasmEmulator.new_with_boot(new Uint8Array(romBytes), new Uint8Array(bootRom));
        } else {
            emu = new WasmEmulator(new Uint8Array(romBytes));
        }
    }

    // Set up OffscreenCanvas renderer if available
    if (offscreen) {
        offscreen.width = screenW;
        offscreen.height = screenH;
        glRenderer = new WebGLRenderer(offscreen);
    }

    const title = emu.title ? emu.title() : '';
    self.postMessage({ cmd: 'ready', title, screenW, screenH });
}

/* ── Frame loop ───────────────────────────────────────────────────── */

function tick() {
    if (!running || !emu) return;

    const now = performance.now();
    if (lastTick === 0) { lastTick = now; setTimeout(tick, 1); return; }

    let delta = now - lastTick;
    lastTick = now;

    const FRAME_MS = system === 'gba' ? GBA_FRAME_MS : GB_FRAME_MS;
    if (delta > 100) delta = FRAME_MS;

    let framesRun = 0;
    const screenW = system === 'gba' ? 240 : 160;
    const screenH = system === 'gba' ? 160 : 144;
    const screenBytes = screenW * screenH * 4;

    if (rewinding) {
        if (rewindBuffer.length > 0) {
            const state = rewindBuffer.pop();
            emu.load_state(state);
            framesRun = 1;
        } else {
            rewinding = false;
        }
    } else {
        const effectiveSpeed = fastForward ? 16 : speed;
        frameDebt += delta * effectiveSpeed;
        const maxFrames = fastForward ? 32 : Math.max(4, 4 * speed);

        while (frameDebt >= FRAME_MS && framesRun < maxFrames) {
            turboFrame++;
            const turboOn = (turboFrame & 1) === 0;
            if (turboA) emu.set_button(4, turboOn);
            if (turboB) emu.set_button(5, turboOn);

            emu.run_frame();
            applyGameSharkCheats();
            frameDebt -= FRAME_MS;
            framesRun++;

            rewindFrameCounter++;
            if (rewindFrameCounter >= REWIND_CAPTURE_INTERVAL) {
                rewindFrameCounter = 0;
                if (emu.save_state) {
                    const state = emu.save_state();
                    rewindBuffer.push(state);
                    if (rewindBuffer.length > REWIND_MAX_FRAMES) rewindBuffer.shift();
                }
            }
        }
        if (fastForward && frameDebt > FRAME_MS * 4) frameDebt = 0;
    }

    if (framesRun > 0) {
        // Feed audio samples to main thread
        feedAudio();

        // Render
        const ptr = emu.framebuffer_ptr();
        const srcView = new Uint8ClampedArray(wasm.memory.buffer, ptr, screenBytes);

        if (glRenderer && glRenderer.ready) {
            glRenderer.render(srcView, screenW, screenH);
        } else {
            // Post raw pixels to main thread for Canvas 2D rendering
            const copy = new Uint8ClampedArray(srcView);
            self.postMessage(
                { cmd: 'frame', pixels: copy, w: screenW, h: screenH },
                [copy.buffer]
            );
        }

        // Rumble
        if (system !== 'gba' && emu.rumble && emu.rumble()) {
            self.postMessage({ cmd: 'rumble' });
        }
    }

    setTimeout(tick, 1);
}

/* ── Audio ────────────────────────────────────────────────────────── */

function feedAudio() {
    if (!emu || muted) return;
    if (!emu.audio_ring_available) return;

    const available = emu.audio_ring_available();
    if (available < 2) return;

    let samplePairs = Math.floor(available / 2);
    const maxWasmBuffer = Math.floor(AUDIO_SAMPLE_RATE * 0.04);
    if (samplePairs > maxWasmBuffer) {
        const skip = samplePairs - Math.floor(AUDIO_SAMPLE_RATE * 0.006);
        if (skip > 0) { emu.audio_ring_consume(skip * 2); samplePairs -= skip; }
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
        left[i] = wasmBuf[rp]; rp = (rp + 1) & mask;
        right[i] = wasmBuf[rp]; rp = (rp + 1) & mask;
    }
    emu.audio_ring_consume(count * 2);

    self.postMessage({ cmd: 'audio', left, right }, [left.buffer, right.buffer]);
}

/* ── Cheats ───────────────────────────────────────────────────────── */

function applyGameSharkCheats() {
    if (activeCheats.length === 0 || !emu.write_byte) return;
    for (const cheat of activeCheats) {
        if (cheat.addr !== undefined && cheat.val !== undefined) {
            emu.write_byte(cheat.addr, cheat.val);
        }
    }
}

/* ── Message handler ──────────────────────────────────────────────── */

self.onmessage = async (e) => {
    const msg = e.data;

    switch (msg.cmd) {
        case 'init':
            if (msg.offscreen) offscreen = msg.offscreen;
            await initWasm(msg.rom, msg.system, msg.bootRom);
            running = true;
            lastTick = 0;
            frameDebt = 0;
            tick();
            break;

        case 'input':
            if (emu) emu.set_button(msg.button, msg.pressed);
            break;

        case 'pause':
            running = false;
            break;

        case 'resume':
            running = true;
            lastTick = 0;
            frameDebt = 0;
            tick();
            break;

        case 'set_speed':
            speed = msg.speed;
            break;

        case 'fast_forward':
            fastForward = msg.enabled;
            if (!msg.enabled) frameDebt = 0;
            break;

        case 'mute':
            muted = msg.muted;
            break;

        case 'turbo':
            if (msg.button === 'a') turboA = msg.enabled;
            if (msg.button === 'b') turboB = msg.enabled;
            break;

        case 'save_state':
            if (emu && emu.save_state) {
                const data = emu.save_state();
                self.postMessage({ cmd: 'state', data }, [data.buffer]);
            }
            break;

        case 'load_state':
            if (emu && emu.load_state && msg.data) {
                emu.load_state(new Uint8Array(msg.data));
            }
            break;

        case 'battery_ram':
            if (emu && emu.has_battery && emu.has_battery()) {
                const len = emu.battery_ram_len();
                if (len > 0) {
                    const ptr = emu.battery_ram_ptr();
                    const ram = new Uint8Array(new Uint8Array(wasm.memory.buffer, ptr, len));
                    const title = emu.title ? emu.title() : '';
                    self.postMessage({ cmd: 'battery_ram', data: ram, title }, [ram.buffer]);
                }
            }
            break;

        case 'load_battery_ram':
            if (emu && emu.load_battery_ram && msg.data) {
                emu.load_battery_ram(new Uint8Array(msg.data));
            }
            break;

        case 'set_filter':
            if (glRenderer && glRenderer.ready) {
                glRenderer.setFilter(msg.filter);
                if (msg.blend !== undefined) glRenderer.setBlend(msg.blend);
            }
            break;

        case 'set_palette':
            if (glRenderer && glRenderer.ready) {
                glRenderer.setPalette(msg.lut);
            }
            break;

        case 'set_cheats':
            activeCheats = msg.cheats || [];
            break;

        case 'rewind_start':
            rewinding = true;
            break;

        case 'rewind_stop':
            rewinding = false;
            break;

        case 'debug_state':
            if (emu) {
                const result = {};
                if (emu.cpu_registers) result.cpu = emu.cpu_registers();
                if (emu.read_byte) {
                    // Sample memory around requested address
                    const addr = msg.addr || 0;
                    const len = msg.len || 256;
                    const mem = new Uint8Array(len);
                    for (let i = 0; i < len; i++) mem[i] = emu.read_byte(addr + i);
                    result.mem = mem;
                    result.memAddr = addr;
                }
                self.postMessage({ cmd: 'debug_state', ...result });
            }
            break;

        case 'serial_receive':
            if (emu && emu.serial_receive) emu.serial_receive(msg.byte);
            break;

        case 'serial_poll':
            if (emu && emu.serial_take_outgoing) {
                const byte = emu.serial_take_outgoing();
                if (byte !== undefined && byte !== null) {
                    self.postMessage({ cmd: 'serial_out', byte });
                }
            }
            break;

        case 'reset':
            // Re-init with same ROM
            if (msg.rom) {
                running = false;
                rewindBuffer = [];
                rewindFrameCounter = 0;
                await initWasm(msg.rom, msg.system, msg.bootRom);
                running = true;
                lastTick = 0;
                frameDebt = 0;
                tick();
            }
            break;
    }
};
