# RUGB - Rust Gameboy

[![RUGB CI/CD](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/ci.yml)
[![Docker Build & Push](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml/badge.svg)](https://github.com/Baptiste-lg/RUGB/actions/workflows/Docker.yml)
[![Documentation](https://img.shields.io/badge/demo-GitHub%20Pages-blue)](https://baptiste-lg.github.io/RUGB/)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)
![JavaScript](https://img.shields.io/badge/JavaScript-F7DF1E?logo=javascript&logoColor=black)
![HTML5](https://img.shields.io/badge/HTML5-E34F26?logo=html5&logoColor=white)
![CSS3](https://img.shields.io/badge/CSS3-1572B6?logo=css3&logoColor=white)
![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)

A Game Boy and Game Boy Advance emulator written in Rust, compiled to WebAssembly, playable in the browser. Drop a ROM and play — no install, no backend.

**[Play it here](https://baptiste-lg.github.io/RUGB/)**

## Supported Systems

| System | CPU | Resolution | Status |
|--------|-----|-----------|--------|
| Game Boy (DMG) | SM83 @ 4.19 MHz | 160x144 | Full emulation |
| Game Boy Color | SM83 @ 4.19/8.39 MHz | 160x144 | Full color emulation |
| Game Boy Advance | ARM7TDMI @ 16.78 MHz | 240x160 | Full emulation |

ROM type is auto-detected — just drop any `.gb` or `.gba` file.

## Features

### Game Boy / Game Boy Color Emulation
- Full SM83 CPU — all 512 opcodes (256 base + 256 CB-prefixed)
- Game Boy Color support — auto-detected from ROM header (0x0143)
  - VRAM banking (2 x 8 KB, VBK register)
  - WRAM banking (8 x 4 KB, SVBK register)
  - CGB color palettes — 8 BG + 8 OBJ palettes (RGB555) via BCPS/BCPD/OCPS/OCPD
  - CGB tile attributes (palette, VRAM bank, flip, priority from bank 1)
  - Double-speed CPU mode (KEY1 register)
  - HDMA transfers (general + HBlank DMA)
- Scanline-accurate PPU — background, window, and sprite rendering (DMG + CGB color)
- Sample-accurate APU — all 4 channels with DC-blocking high-pass filter and AudioWorklet output
- Cartridge support — NoMBC, MBC1, MBC2, MBC3 (with RTC), MBC5 (with rumble), MBC7, HuC1, HuC3, MMM01
- Timer subsystem with falling-edge detection
- Interrupt controller (VBlank, STAT, Timer, Serial, Joypad)
- Serial link cable — master/slave 512 T-cycle timing
- Battery save — cartridge SRAM persisted to localStorage
- Boot ROM support — drop a `dmg_boot.bin` to see the Nintendo logo scroll

### GBA Emulation
- ARM7TDMI CPU — full ARM (32-bit) and THUMB (16-bit) instruction sets
- HLE BIOS — SoftReset, Halt, IntrWait, VBlankIntrWait, Div, Sqrt, CpuSet, CpuFastSet
- Memory bus — EWRAM, IWRAM, VRAM, palette, OAM, ROM, SRAM with proper mirroring
- PPU — all 6 display modes:
  - Mode 0: 4 text BG layers (8x8 tiles, 4bpp/8bpp, scroll, priority)
  - Mode 1: 2 text BG + 1 affine BG (rotation/scaling)
  - Mode 2: 2 affine BG layers
  - Mode 3: 240x160 direct 15-bit color
  - Mode 4: 240x160 indexed, double-buffered
  - Mode 5: 160x128 direct color, double-buffered
- Sprite rendering — 128 OBJ from OAM (4bpp/8bpp, hflip/vflip, 1D/2D mapping)
- Affine sprites — rotation/scaling via OAM affine parameter groups
- DMA controller — 4 channels with immediate transfer + sound FIFO DMA
- Timer controller — 4 cascadable 16-bit timers with prescaler
- Audio — DMA Sound FIFO A/B (8-bit PCM, timer-driven) + PSG channels (CH1-4)
- Cartridge backup — SRAM (32KB), Flash (64/128KB with command protocol), EEPROM (512B/8K), auto-detected
- Color effects — alpha blending (EVA/EVB), brightness increase/decrease (EVY)
- Scanline-accurate timing with H-blank, V-blank, and V-count match interrupts
- Save states — full CPU, memory, and I/O serialization
- 10-button keypad input with mobile L/R touch controls

### Interface
- Faithful DMG-01 Game Boy shell with interactive, animated buttons
- Game Boy Color shell with distinct styling
- Classic Indigo GBA shell with shoulder buttons
- Shell auto-switches based on ROM type (manual override available)
- Drag-and-drop ROM loading (`.gb`, `.gba`, `.zip`) — whole page
- IPS/BPS patch support — drop a patch file to apply to the current ROM
- Save states — 5 slots with export/import, quick save (F5) / quick load (F8)
- Rewind — hold R to step backwards through gameplay (~5 seconds buffer)
- Auto-save on exit — resume where you left off when reloading
- Cheat codes — Game Genie, GameShark, and libretro cheat database with toggle UI
- Video recording — capture gameplay as WebM (F9)
- Shareable state links — copy a URL encoding the current save state (F10)
- Keyboard and gamepad remapping with export/import as JSON
- Speed control (1/2x / 1x / 2x / 4x) + hold Space for uncapped fast forward
- Turbo buttons — toggle auto-repeat for A (Q) and B (W)
- Color palettes — classic green, gray, B&W, and fully customizable with color pickers
- Display filters — CRT scanlines, LCD grid, smooth scaling, frame blending
- Volume slider and per-channel mute with real-time audio visualizer
- Per-channel audio waveforms — 4 color-coded mini visualizers (CH1-4)
- RTC time override for MBC3 games (F6)
- Rumble feedback via Gamepad Vibration API (MBC5 rumble carts)
- Fullscreen, screenshot, FPS counter
- Console view / screen-only toggle with free resize
- Mobile touch controls with haptic feedback
- Installable PWA — works offline via service worker
- ROM library — previously loaded ROMs saved in IndexedDB with search and delete

### Multiplayer
- Link cable — local multiplayer between two tabs via BroadcastChannel
- Online multiplayer — peer-to-peer WebRTC with manual SDP exchange

### Advanced
- WebGL renderer — GPU-accelerated rendering with shader-based filters (CRT barrel distortion, LCD dot matrix, smooth bilinear, frame blending)
- Web Worker mode — emulation runs off the main thread via OffscreenCanvas for smoother frame pacing
- Cloud saves — backup/restore to Google Drive (appDataFolder, OAuth 2.0)
- Debug tools — CPU register viewer, memory hex editor, tile viewer, SM83 disassembler, single-step execution
- Per-game settings — palette, filter, and speed auto-saved per ROM title
- Input HUD — on-screen button state overlay for streaming/recording
- Light/dark theme toggle
- Dockable sidebar — drag to left, right, top, or bottom edge
- Keyboard shortcuts overlay (press ?)
- Content Security Policy headers, input sanitization, WASM memory safety

## Architecture

The project is a Cargo workspace with two crates:

```
RUGB/
+-- rugb/          Game Boy emulator (SM83 CPU)
+-- rugba/         GBA emulator (ARM7TDMI CPU)
+-- web/           Shared web frontend
```

Both crates compile to independent WASM modules. The JS frontend auto-detects the ROM type and loads the correct module.

```
+----------------------------------------------------------+
|                      Browser (JS)                        |
|  +----------+  +---------------+  +-------------------+  |
|  |  Canvas / |  | AudioWorklet  |  | Keyboard / Touch  |  |
|  |  WebGL    |  |   (sound)     |  | Gamepad (input)   |  |
|  +-----+-----+  +------+-------+  +--------+----------+  |
|        |               |                   |             |
|  +-----+---------------+-------------------+----------+  |
|  |              wasm-bindgen bridge                    |  |
|  +----------+-------------------+---------+-----------+  |
|             |                   |         |              |
|        +----+----+         +----+----+    |              |
|        | Worker  |         | Worker  |  (main thread     |
|        | (opt.)  |         | (opt.)  |   fallback)       |
+--------+---------+---------+---------+-------------------+
              |                   |
    +---------+---------+   +----+----------+
    |    rugb (WASM)     |   |    rugba (WASM)    |
    |                    |   |                    |
    |  SM83 CPU          |   |  ARM7TDMI CPU      |
    |  PPU (160x144)     |   |  PPU (240x160)     |
    |  APU (4 channels)  |   |  APU (FIFO + PSG)  |
    |  MMU + MBC1-7      |   |  DMA + Timers      |
    |  Timer + Serial    |   |  I/O + Keypad      |
    +--------------------+   +--------------------+
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

GBA audio uses DMA Sound FIFO channels (A/B) for 8-bit PCM playback driven by timers, plus 4 PSG channels identical to the Game Boy APU.

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

Supports all 6 display modes, sprites (regular + affine), DMA sound, Flash/SRAM/EEPROM saves, and alpha blending.

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
  serial.rs            Link cable serial transfer
  interrupt.rs         5-type interrupt controller
  cartridge/
    mod.rs             ROM header parser, MBC detection
    no_mbc.rs          ROM-only
    mbc1.rs            MBC1 (battery)
    mbc2.rs            MBC2 (built-in RAM)
    mbc3.rs            MBC3 (RTC, battery)
    mbc5.rs            MBC5 (rumble, battery)
    mbc7.rs            MBC7 (accelerometer, EEPROM)
    huc1.rs            HuC1 (IR stub)
    huc3.rs            HuC3 (RTC, IR stubs)
    mmm01.rs           MMM01 (multi-cart)

rugba/src/
  lib.rs              WASM entry point, GbaEmulator + WasmGbaEmulator
  arm7tdmi/
    mod.rs             ARM7TDMI core, mode switching, HLE BIOS
    arm.rs             ARM (32-bit) instruction decoder
    thumb.rs           THUMB (16-bit) instruction decoder
    registers.rs       Banked registers, CPSR/SPSR, mode enum
  bus.rs               Memory bus (EWRAM, IWRAM, VRAM, ROM, SRAM)
  ppu/
    mod.rs             Scanline state machine, timing, layer compositing
    modes.rs           Mode 3/4/5 bitmap rendering
    bg.rs              Text and affine BG tile rendering (Mode 0-2)
    obj.rs             Sprite renderer (128 OBJ, 4bpp/8bpp)
    blend.rs           Alpha blend + brightness effects
  dma.rs               4-channel DMA controller (immediate + sound FIFO)
  timer.rs             4 cascadable 16-bit timers
  apu.rs               DMA FIFO sound (A/B) + PSG channels (CH1-4)
  io.rs                I/O register file (PPU, BG, DMA, timer, interrupt)
  cartridge.rs         Backup detection (SRAM, Flash, EEPROM) + Flash state machine
  keypad.rs            10-button input (A, B, L, R, Start, Select, D-pad)

web/
  index.html           GB + GBC + GBA shells, overlays, sidebar
  style.css            DMG gray + GBC purple + GBA Indigo styling, CSS theme variables
  js/
    index.js           Main frontend (auto-detects system, frame loop, UI)
    webgl-renderer.js  GPU shader rendering (CRT, LCD, smooth, ghost filters)
    emu-worker.js      Web Worker for off-main-thread emulation
    link-cable.js      Multiplayer (BroadcastChannel + WebRTC P2P)
    debug-tools.js     CPU viewer, memory hex editor, tile viewer, disassembler
    dock.js            Dockable sidebar (drag to any edge)
    cloud-saves.js     Google Drive backup/sync
  audio-processor.js   AudioWorklet for low-latency sound
  sw.js                Service worker (PWA/offline)
  manifest.json        PWA manifest
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
