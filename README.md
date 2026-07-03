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
- Cartridge support — NoMBC, MBC1, MBC3 (covers Tetris through Pokemon)
- Timer, interrupts, and joypad input
- APU with frame sequencer and Web Audio output (channels 1 & 2)

### Interface
- Faithful DMG-01 Game Boy shell with interactive buttons
- Drag-and-drop ROM loading (`.gb`, `.zip`)
- Save states — 5 slots with export/import, quick save (F5) / quick load (F8)
- Keyboard remapping with export/import as JSON
- Gamepad support with remappable bindings and controller auto-detection
- Speed control (1x / 2x / 4x)
- Color palette selector (classic green, gray, B&W)
- Console view / screen-only display toggle
- Mobile touch controls
- Pause, reset, mute
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
| F5 | Quick save |
| F8 | Quick load |

All keys are remappable from the side menu. Gamepad bindings are also fully configurable.

Touch controls appear automatically on mobile devices.

## Game Compatibility

| Game | MBC | Status |
|------|-----|--------|
| Tetris | None | Target: first playable game |
| Dr. Mario | None | Validates sprites and input |
| Super Mario Land | MBC1 | Tests scrolling and banking |
| Kirby's Dream Land | MBC1 | Tests window layer |
| Pokemon Red/Blue | MBC3 | The holy grail |

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
  apu.rs               Audio Processing Unit — 4 channels
  timer.rs             DIV, TIMA, TMA, TAC
  joypad.rs            Button input
  interrupt.rs         Interrupt controller
  cartridge/
    mod.rs             ROM header parsing, MBC detection
    no_mbc.rs          ROM-only cartridges
    mbc1.rs            MBC1 mapper
    mbc3.rs            MBC3 mapper
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
