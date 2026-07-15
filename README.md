# RUGB

[![RUGB CI/CD](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml)
[![Docker Build & Push](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml)
[![Documentation](https://img.shields.io/badge/demo-GitHub%20Pages-blue)](https://baptiste-lg.github.io/RUGB/)

A Game Boy (DMG) emulator written in Rust, compiled to WebAssembly, playable in the browser. Load a ROM and play — no install, no backend.

**[Play it here](https://baptiste-lg.github.io/RUGB/)**

## Features

### Emulation
- Full SM83 CPU — all 512 opcodes (256 base + 256 CB-prefixed)
- Scanline-accurate PPU — background, window, and sprite rendering
- Sample-accurate APU — all 4 channels (2 square, 1 wave, 1 noise) with DC-blocking high-pass filter
- Cartridge support — NoMBC, MBC1, MBC3 (covers Tetris through Pokemon)
- Timer subsystem with falling-edge detection
- Interrupt controller (VBlank, STAT, Timer, Serial, Joypad)
- Battery save — cartridge SRAM automatically persisted to localStorage

### Interface
- Faithful DMG-01 Game Boy shell with interactive, animated buttons
- Drag-and-drop ROM loading anywhere on the page (`.gb`, `.zip`)
- Save states — 5 slots with export/import, quick save (F5) / quick load (F8)
- Keyboard remapping with export/import as JSON
- Gamepad support with remappable bindings and controller auto-detection
- Speed control (½x / 1x / 2x / 4x) + hold Space for uncapped fast forward
- Turbo buttons — toggle auto-repeat for A (Q key) and B (W key)
- Color palettes — classic green, gray, B&W, and fully customizable user palette
- Volume slider and per-channel mute (CH1–CH4)
- Fullscreen mode (F11 or menu button)
- Screenshot download as PNG
- Console view / screen-only display toggle with free resize
- Recent ROMs history
- All preferences persisted in localStorage

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Browser (JS)                     │
│  ┌───────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │  Canvas   │  │ WebAudio │  │ Keyboard / Touch │  │
│  │ (display) │  │  (sound) │  │ Gamepad (joypad) │  │
│  └─────┬─────┘  └─────┬────┘  └────────┬─────────┘  │
│        │              │                │            │
│  ┌─────┴──────────────┴────────────────┴─────────┐  │
│  │              wasm-bindgen bridge              │  │
│  └─────┬──────────────┬────────────────┬─────────┘  │
└────────┼──────────────┼────────────────┼────────────┘
         │              │                │
┌────────┼──────────────┼────────────────┼────────────┐
│        │        Rust WASM Core         │            │
│                                                     │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐ │
│  │   CPU   │  │   PPU   │  │   APU   │  │  Timer  │ │
│  │ (SM83)  │  │ (video) │  │ (audio) │  │         │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘ │
│       │            │            │            │      │
│  ┌────┴────────────┴────────────┴────────────┴───┐  │
│  │               Memory Bus (MMU)                │  │
│  └────┬──────┬───────┬───────┬───────┬───────┬───┘  │
│       │      │       │       │       │       │      │
│    ┌──┴─┐ ┌──┴──┐ ┌──┴──┐ ┌──┴──┐ ┌──┴─┐ ┌───┴─┐    │
│    │ROM │ │VRAM │ │WRAM │ │OAM  │ │ IO │ │HRAM │    │
│    └────┘ └─────┘ └─────┘ └─────┘ └────┘ └─────┘    │
└─────────────────────────────────────────────────────┘
```

## Build

Requires Rust and `wasm-pack`.

```sh
# Install wasm-pack (if not already)
cargo install wasm-pack

# Build the WASM module
wasm-pack build --target web --out-dir web/pkg --release

# Serve the web/ directory (any static server works)
python3 -m http.server -d web 8080
```

Then open `http://localhost:8080` and load a `.gb` ROM file.

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
| Space (hold) | Fast forward |
| Q | Toggle turbo A |
| W | Toggle turbo B |
| F5 | Quick save |
| F8 | Quick load |
| F11 | Fullscreen |
| Escape | Toggle side menu |
| 1 / 2 / 4 | Set speed 1x / 2x / 4x |

All keys are remappable from the side menu. Gamepad bindings are also fully configurable.

## Audio

The APU generates sample-accurate audio at 48 kHz with all four Game Boy channels:

| Channel | Type | Used for |
|---------|------|----------|
| CH1 | Square wave + sweep | Melody, sound effects |
| CH2 | Square wave | Harmony, secondary melody |
| CH3 | Programmable wave | Bass, custom waveforms |
| CH4 | Noise (LFSR) | Drums, percussion |

Each channel can be individually muted from the side menu for music isolation or debugging. A hardware-accurate high-pass filter removes DC offset, matching the Game Boy's coupling capacitor behavior.

## Game Compatibility

| Game | MBC | Status |
|------|-----|--------|
| Tetris | None | Fully playable |
| Dr. Mario | None | Fully playable |
| Super Mario Land | MBC1 | Fully playable |
| Kirby's Dream Land | MBC1 | Fully playable |
| Pokemon Red/Blue | MBC3 | Playable with battery save |

## Project Structure

```
src/
  lib.rs              WASM entry point, Emulator struct
  savestate.rs         Save state serialization helpers
  cpu/
    mod.rs             Fetch-decode-execute loop, ALU helpers
    registers.rs       Register file (AF, BC, DE, HL, SP, PC)
    opcodes.rs         All 256 base opcodes
    cb_opcodes.rs      All 256 CB-prefixed opcodes
  mmu.rs               Memory bus — address routing
  ppu.rs               Pixel Processing Unit — scanline renderer
  apu.rs               Audio Processing Unit — 4 channels, sample-accurate
  timer.rs             DIV, TIMA, TMA, TAC
  joypad.rs            Button input
  interrupt.rs         Interrupt controller
  cartridge/
    mod.rs             ROM header parsing, MBC detection
    no_mbc.rs          ROM-only cartridges
    mbc1.rs            MBC1 mapper (with battery save)
    mbc3.rs            MBC3 mapper (with battery save)
web/
  index.html           Emulator UI (faithful DMG-01 shell)
  style.css            Game Boy shell styling, responsive layout
  js/index.js          Render loop, input, audio, save states, gamepad
```

## References

- [Pan Docs](https://gbdev.io/pandocs/) — the definitive GB hardware reference
- [SM83 Opcode Table](https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html)
- [GB Complete Technical Reference](https://gekkio.fi/files/gb-docs/gbctr.pdf)
- [DMG-01 Book](https://rylev.github.io/DMG-01/public/book/) — Rust-specific GB emulator guide

## License

MIT
