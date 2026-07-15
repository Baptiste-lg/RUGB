# RUGB Developer Documentation

This directory contains technical documentation for developers working on RUGB. It explains how each subsystem works, how they interact, and where the tricky bits are.

## Table of Contents

1. [Architecture Overview](architecture.md) — how the emulator fits together
2. [CPU (SM83)](cpu.md) — instruction decoding, ALU, flags, timing
3. [PPU (Video)](ppu.md) — scanline renderer, tile modes, sprites
4. [APU (Audio)](apu.md) — sample generation, channels, mixing, high-pass filter
5. [MMU (Memory Bus)](mmu.md) — address routing, I/O registers, DMA
6. [Timer](timer.md) — DIV, TIMA, falling-edge detection
7. [Cartridge Mappers](cartridge.md) — NoMBC, MBC1, MBC2, MBC3, MBC5, battery saves
8. [Frontend](frontend.md) — WASM bridge, audio pipeline, frame timing, UI

## Quick Start for Contributors

```sh
# Build
cargo install wasm-pack
wasm-pack build --target web --out-dir web/pkg --release

# Serve
python3 -m http.server -d web 8080

# Run tests
cargo test
```

The emulator has no boot ROM — registers are initialized to post-boot values (DMG-01).

## Key Design Decisions

- **No boot ROM**: CPU starts at PC=0x0100 with post-boot register state.
- **Scanline rendering**: The PPU renders whole scanlines at once (not per-dot). This is simpler than dot-accurate emulation and correct enough for the vast majority of games.
- **Per-cycle APU**: The APU ticks every T-cycle for sample-accurate audio generation at 48 kHz.
- **Frame-driven emulation**: The JS frontend calls `run_frame()` which runs exactly 70,224 T-cycles (one Game Boy frame). Frame pacing is handled by the browser via `requestAnimationFrame` with delta-time throttling.
- **Save states**: Binary format using little-endian push/pop helpers. Each module serializes its own state in a fixed order. No versioning — format changes break old saves.
