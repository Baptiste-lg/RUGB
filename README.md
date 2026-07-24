# RUGB

[![RUGB CI/CD](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml)
[![Docker Build & Push](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml)
[![Documentation](https://img.shields.io/badge/demo-GitHub%20Pages-blue)](https://baptiste-lg.github.io/RUGB/)

A Game Boy and Game Boy Advance emulator written in Rust, compiled to WebAssembly, playable in the browser. Drop a ROM and play — no install, no backend.

**[Play it here](https://baptiste-lg.github.io/RUGB/)**

## Supported Systems

| System | CPU | Resolution | Status |
|--------|-----|-----------|--------|
| Game Boy (DMG) | SM83 @ 4.19 MHz | 160x144 | Full emulation |
| Game Boy Advance | ARM7TDMI @ 16.78 MHz | 240x160 | Bitmap modes (Mode 3/4/5) |

ROM type is auto-detected — just drop any `.gb` or `.gba` file.

## Features

### Game Boy Emulation
- Full SM83 CPU — all 512 opcodes (256 base + 256 CB-prefixed)
- Scanline-accurate PPU — background, window, and sprite rendering
- Sample-accurate APU — all 4 channels with DC-blocking high-pass filter and AudioWorklet output
- Cartridge support — NoMBC, MBC1, MBC2, MBC3 (with RTC), MBC5 (with rumble)
- Timer subsystem with falling-edge detection
- Interrupt controller (VBlank, STAT, Timer, Serial, Joypad)
- Battery save — cartridge SRAM persisted to localStorage
- Boot ROM support — drop a `dmg_boot.bin` to see the Nintendo logo scroll

### GBA Emulation (Phase 1+2)
- ARM7TDMI CPU — full ARM (32-bit) and THUMB (16-bit) instruction sets
- HLE BIOS — software interrupt stubs (Div, Sqrt, CpuSet, Halt, VBlankIntrWait)
- Memory bus — EWRAM, IWRAM, VRAM, palette, OAM, ROM, SRAM with proper mirroring
- PPU — Mode 3 (240x160 direct color), Mode 4 (indexed, double-buffered), Mode 5 (160x128 direct)
- Scanline-accurate timing with H-blank and V-blank interrupts
- 10-button keypad input (A, B, L, R, Start, Select, D-pad)

### Interface
- Faithful DMG-01 Game Boy shell with interactive, animated buttons
- Classic Indigo GBA shell with shoulder buttons (auto-switches based on ROM type)
- Drag-and-drop ROM loading (`.gb`, `.gba`, `.zip`)
- IPS/BPS patch support — drop a patch file to apply to the current ROM
- Save states — 5 slots with export/import, quick save (F5) / quick load (F8)
- Rewind — hold R to step backwards through gameplay (~5 seconds buffer)
- Auto-save on exit — resume where you left off when reloading
- Cheat codes — Game Genie and GameShark support (F7)
- Video recording — capture gameplay as WebM (F9)
- Shareable state links — copy a URL encoding the current save state (F10)
- Keyboard and gamepad remapping with export/import as JSON
- Speed control (1/2x / 1x / 2x / 4x) + hold Space for uncapped fast forward
- Turbo buttons — toggle auto-repeat for A (Q) and B (W)
- Color palettes — classic green, gray, B&W, and fully customizable
- Display filters — CRT scanlines, LCD grid, smooth scaling, frame blending
- Volume slider and per-channel mute with real-time audio visualizer
- RTC time override for MBC3 games (F6)
- Rumble feedback via Gamepad Vibration API (MBC5 rumble carts)
- Fullscreen, screenshot, FPS counter
- Console view / screen-only toggle with free resize
- Mobile touch controls with haptic feedback
- Installable PWA — works offline
- ROM library — previously loaded ROMs saved in IndexedDB for quick re-launch

## Architecture

The project is a Cargo workspace with two crates:

```
RUGB/
├── rugb/          Game Boy emulator (SM83 CPU)
├── rugba/         GBA emulator (ARM7TDMI CPU)
└── web/           Shared web frontend
```

Both crates compile to independent WASM modules. The JS frontend auto-detects the ROM type and loads the correct module.

```
┌──────────────────────────────────────────────────────────┐
│                      Browser (JS)                        │
│  ┌──────────┐  ┌───────────────┐  ┌───────────────────┐  │
│  │  Canvas   │  │ AudioWorklet  │  │ Keyboard / Touch  │  │
│  │ (display) │  │   (sound)     │  │ Gamepad (input)   │  │
│  └─────┬─────┘  └──────┬───────┘  └────────┬──────────┘  │
│        │               │                   │             │
│  ┌─────┴───────────────┴───────────────────┴──────────┐  │
│  │              wasm-bindgen bridge                   │  │
│  └──────────┬───────────────────────┬─────────────────┘  │
└─────────────┼───────────────────────┼────────────────────┘
              │                       │
    ┌─────────┴─────────┐   ┌────────┴──────────┐
    │    rugb (WASM)     │   │    rugba (WASM)    │
    │                    │   │                    │
    │  SM83 CPU          │   │  ARM7TDMI CPU      │
    │  PPU (160x144)     │   │  PPU (240x160)     │
    │  APU (4 channels)  │   │  Memory Bus        │
    │  MMU + MBC1-5      │   │  I/O + Keypad      │
    │  Timer + Joypad    │   │  HLE BIOS          │
    └────────────────────┘   └────────────────────┘
```

## Build

Requires Rust and `wasm-pack`.

```sh
# Install wasm-pack
cargo install wasm-pack

# Build both emulators
wasm-pack build rugb --target web --out-dir ../web/pkg/rugb --release
wasm-pack build rugba --target web --out-dir ../web/pkg/rugba --release

# Serve
python3 -m http.server -d web 8080
```

Then open `http://localhost:8080` and drop a `.gb` or `.gba` ROM file.

## Tests

```sh
# Run all workspace tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p rugb
cargo test -p rugba
```

## Controls

| Key | Action |
|-----|--------|
| Arrow keys | D-pad |
| Z | A button |
| X | B button |
| Enter | Start |
| Shift | Select |
| P | Pause / Resume |
| M | Mute / Unmute |
| R (hold) | Rewind |
| Space (hold) | Fast forward |
| Q / W | Toggle turbo A / B |
| F3 | Toggle FPS counter |
| F5 | Quick save |
| F6 | RTC time override |
| F7 | Add cheat code |
| F8 | Quick load |
| F9 | Toggle video recording |
| F10 | Copy share link |
| F11 | Fullscreen |
| ? | Keyboard shortcuts |
| Escape | Toggle side menu |
| 1 / 2 / 4 | Speed 1x / 2x / 4x |

All keys are remappable. Gamepad bindings are fully configurable.

## Audio

The Game Boy APU generates sample-accurate audio at 48 kHz via AudioWorklet (2.67ms latency):

| Channel | Type | Used for |
|---------|------|----------|
| CH1 | Square wave + sweep | Melody, effects |
| CH2 | Square wave | Harmony |
| CH3 | Programmable wave | Bass, custom waveforms |
| CH4 | Noise (LFSR) | Drums, percussion |

Each channel can be individually muted. A hardware-accurate high-pass filter removes DC offset.

GBA audio is not yet implemented (Phase 4).

## Game Compatibility

### Game Boy

| Game | MBC | Status |
|------|-----|--------|
| Tetris | None | Fully playable |
| Dr. Mario | None | Fully playable |
| Super Mario Land | MBC1 | Fully playable |
| Kirby's Dream Land | MBC1 | Fully playable |
| Mega Man | MBC2 | Fully playable |
| Pokemon Red/Blue | MBC3 | Playable with battery save |
| Pokemon Gold/Silver | MBC3 | Playable with RTC |
| Pokemon Crystal | MBC5 | Playable with battery save |
| Zelda: Link's Awakening DX | MBC5 | Playable with battery save |

### GBA

Bitmap-mode homebrew and demos (Mode 3/4/5). Tile-based commercial games require Phase 3 (tile rendering).

## Project Structure

```
rugb/src/
  lib.rs              WASM entry point, Emulator + WasmEmulator
  savestate.rs         Binary serialization helpers
  cpu/
    mod.rs             SM83 fetch-decode-execute loop
    registers.rs       Register file (AF, BC, DE, HL, SP, PC)
    opcodes.rs         256 base opcodes
    cb_opcodes.rs      256 CB-prefixed opcodes
  mmu.rs               Memory bus, boot ROM, Game Genie cheats
  ppu.rs               Scanline PPU renderer
  apu.rs               4-channel APU with ring buffer
  timer.rs             DIV / TIMA / TMA / TAC
  joypad.rs            8-button input
  interrupt.rs         5-type interrupt controller
  cartridge/
    mod.rs             ROM header parser, MBC detection
    no_mbc.rs          ROM-only
    mbc1.rs            MBC1 (battery)
    mbc2.rs            MBC2 (built-in RAM)
    mbc3.rs            MBC3 (RTC, battery)
    mbc5.rs            MBC5 (rumble, battery)

rugba/src/
  lib.rs              WASM entry point, GbaEmulator + WasmGbaEmulator
  arm7tdmi/
    mod.rs             ARM7TDMI core, mode switching, HLE BIOS
    arm.rs             ARM (32-bit) instruction decoder
    thumb.rs           THUMB (16-bit) instruction decoder
    registers.rs       Banked registers, CPSR/SPSR, mode enum
  bus.rs               Memory bus (EWRAM, IWRAM, VRAM, ROM, SRAM)
  ppu/
    mod.rs             Scanline state machine, timing
    modes.rs           Mode 3/4/5 bitmap rendering
  io.rs                I/O register file (DISPCNT, DISPSTAT, interrupts)
  keypad.rs            10-button input (A, B, L, R, Start, Select, D-pad)

web/
  index.html           GB + GBA shells
  style.css            DMG gray + GBA Indigo styling
  js/index.js          Shared frontend (auto-detects system)
  audio-processor.js   AudioWorklet for low-latency sound
  sw.js                Service worker (PWA/offline)
```

## References

### Game Boy
- [Pan Docs](https://gbdev.io/pandocs/) — GB hardware reference
- [SM83 Opcode Table](https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html)
- [GB Complete Technical Reference](https://gekkio.fi/files/gb-docs/gbctr.pdf)

### GBA
- [GBATEK](https://problemkaputt.de/gbatek.htm) — GBA hardware reference
- [Tonc](https://www.coranac.com/tonc/text/) — GBA programming tutorial
- [ARM7TDMI Technical Reference](https://developer.arm.com/documentation/ddi0210/c)

## Credits

- Cheat database provided by [libretro-database](https://github.com/libretro/libretro-database) (MIT license), maintained by the libretro/RetroArch community

## License

MIT
