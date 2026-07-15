# Architecture Overview

## The Big Picture

RUGB is split into two layers:

1. **Rust core** — the actual emulator, compiled to WebAssembly
2. **JS frontend** — renders video, plays audio, handles input

They communicate through a thin `wasm-bindgen` bridge defined in `src/lib.rs`.

```
Browser (JS)                          Rust WASM Core
─────────────                          ──────────────
Canvas    ← framebuffer_ptr() ←──────  PPU framebuffer
WebAudio  ← audio_buffer_ptr() ←────  APU sample buffer
Keyboard  ── set_button() ──────────→  Joypad
            run_frame() ───────────→  CPU step loop
```

## Execution Flow

Each animation frame, the JS calls `emu.run_frame()`. Inside Rust:

```
run_frame()
  └─ while cycles < 70224:
       step()
         ├─ handle_interrupts()     → 20 cycles if dispatched
         ├─ cpu.step()              → 4-20 cycles per instruction
         ├─ ppu.tick(cycles)        → advances mode state machine
         ├─ timer.tick(cycles)      → falling-edge TIMA increment
         └─ apu.tick(cycles)        → ticks channels, generates samples
```

All subsystems advance by the **same number of T-cycles** returned by the CPU step. This keeps everything synchronized without a shared clock.

## Module Dependency Graph

```
lib.rs (WASM entry point)
  └─ Emulator
       ├─ Cpu         (src/cpu/)
       │    └─ reads/writes through MMU
       └─ Mmu         (src/mmu.rs)
            ├─ Ppu     (src/ppu.rs)        — 0x8000-0x9FFF, 0xFE00-0xFE9F, 0xFF40-0xFF4B
            ├─ Apu     (src/apu.rs)        — 0xFF10-0xFF3F
            ├─ Timer   (src/timer.rs)      — 0xFF04-0xFF07
            ├─ Joypad  (src/joypad.rs)     — 0xFF00
            ├─ Cartridge (src/cartridge/)  — 0x0000-0x7FFF, 0xA000-0xBFFF
            ├─ WRAM    [u8; 0x2000]        — 0xC000-0xDFFF (echo at 0xE000-0xFDFF)
            └─ HRAM    [u8; 0x7F]          — 0xFF80-0xFFFE
```

## Memory Map

| Address Range | Size | Destination |
|---|---|---|
| 0x0000–0x3FFF | 16 KB | ROM Bank 0 (via cartridge) |
| 0x4000–0x7FFF | 16 KB | ROM Bank N (switchable, via cartridge) |
| 0x8000–0x9FFF | 8 KB | VRAM (PPU) |
| 0xA000–0xBFFF | 8 KB | External RAM (via cartridge, battery-backed) |
| 0xC000–0xDFFF | 8 KB | Work RAM |
| 0xE000–0xFDFF | 7,680 B | Echo of Work RAM |
| 0xFE00–0xFE9F | 160 B | OAM (PPU sprite attributes) |
| 0xFEA0–0xFEFF | 96 B | Unusable (returns 0xFF) |
| 0xFF00–0xFF7F | 128 B | I/O registers (routed to subsystems) |
| 0xFF80–0xFFFE | 127 B | High RAM |
| 0xFFFF | 1 B | Interrupt Enable register |

## Data Flow: Video

1. Game writes tiles to VRAM (0x8000–0x9FFF) and tilemaps (0x9800–0x9FFF)
2. Game writes sprite attributes to OAM (0xFE00–0xFE9F)
3. PPU renders one scanline when entering HBlank (mode 3 → mode 0)
4. Pixel data goes into `ppu.framebuffer` — a 160x144 RGBA array
5. JS reads the framebuffer via `framebuffer_ptr()` and draws it to a `<canvas>`

## Data Flow: Audio

1. Game writes to APU registers (0xFF10–0xFF3F) to configure channels
2. APU `tick()` advances channel frequency timers every T-cycle
3. Every ~87 T-cycles, a stereo sample pair (f32 left + f32 right) is pushed to `sample_buffer`
4. A high-pass filter removes DC offset before storage
5. JS `ScriptProcessorNode` callback reads samples via `audio_buffer_ptr()` and plays them

## Data Flow: Input

1. User presses a key → JS calls `emu.set_button(index, true)`
2. Joypad stores the button state and sets the joypad interrupt flag
3. Game reads 0xFF00 → Joypad returns active-low button bits based on the selection register
