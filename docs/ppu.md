# PPU (Pixel Processing Unit)

**File:** `src/ppu.rs` (507 lines)

The PPU renders the Game Boy's 160x144 pixel display using a scanline-based approach. It cycles through four modes per scanline, rendering the complete line when transitioning to HBlank.

## Mode State Machine

Each scanline takes exactly 456 T-cycles:

```
 ┌──────────┐    ┌──────────────┐    ┌──────────┐
 │ OAM Scan │───→│Pixel Transfer│───→│  HBlank  │───→ next line
 │ (Mode 2) │    │  (Mode 3)    │    │ (Mode 0) │
 │  80 dots  │    │   172 dots   │    │ 204 dots │
 └──────────┘    └──────────────┘    └──────────┘
```

After 144 visible scanlines, the PPU enters VBlank (Mode 1) for 10 more scanlines:

| Phase | Lines | T-cycles |
|---|---|---|
| Visible (modes 2→3→0) | 0–143 | 144 × 456 = 65,664 |
| VBlank (mode 1) | 144–153 | 10 × 456 = 4,560 |
| **Total frame** | | **70,224** |

## Rendering Pipeline

Rendering happens at the end of Mode 3 (pixel transfer), right before entering HBlank. For the current scanline (LY), three layers are composited:

### 1. Background

- 256x256 pixel virtual map, scrolled by SCX/SCY registers
- 32x32 tile grid, each tile is 8x8 pixels (16 bytes in VRAM)
- Tile map at 0x9800 or 0x9C00 (selected by LCDC bit 3)
- Tile data at 0x8000 (unsigned) or 0x8800 (signed, base 0x9000) — LCDC bit 4

```
Screen viewport (160x144) scrolls within the 256x256 BG map:

┌─────────────────────────────────┐
│          256x256 BG map         │
│     ┌──────────────────┐        │
│     │  Screen (160x144)│        │
│     │  offset by SCX,  │        │
│     │  SCY             │        │
│     └──────────────────┘        │
└─────────────────────────────────┘
```

### 2. Window

- Overlays the background starting at position (WX-7, WY)
- Has its own tile map (LCDC bit 6) but shares tile data mode with BG
- Enabled by LCDC bit 5
- Uses an internal line counter (`window_line`) that only increments on lines where the window was actually rendered — this is reset at VBlank

### 3. Sprites (OAM)

- Up to 40 sprites defined in OAM (160 bytes at 0xFE00–0xFE9F)
- Max 10 sprites per scanline
- Each OAM entry: 4 bytes

```
Byte 0: Y position + 16  (Y=0 → sprite is 16px above screen)
Byte 1: X position + 8   (X=0 → sprite is 8px left of screen)
Byte 2: Tile index
Byte 3: Attributes
  Bit 7: BG priority (1 = behind BG colors 1-3)
  Bit 6: Y-flip
  Bit 5: X-flip
  Bit 4: Palette (OBP0 or OBP1)
```

**8x16 mode** (LCDC bit 2): tile index bit 0 is forced to 0 for the top half, 1 for the bottom half.

**Priority**: lower X coordinate wins. Equal X → lower OAM index wins. The implementation sorts sprites and renders in reverse order (painter's algorithm) so higher-priority sprites overwrite.

**Transparency**: sprite color 0 is always transparent.

## LCDC Register (0xFF40)

| Bit | Function |
|-----|----------|
| 7 | LCD enable (0 = off, screen blank) |
| 6 | Window tile map (0 = 0x9800, 1 = 0x9C00) |
| 5 | Window enable |
| 4 | BG/Window tile data (0 = 0x8800 signed, 1 = 0x8000 unsigned) |
| 3 | BG tile map (0 = 0x9800, 1 = 0x9C00) |
| 2 | Sprite size (0 = 8x8, 1 = 8x16) |
| 1 | Sprite enable |
| 0 | BG/Window enable |

## STAT Register (0xFF41)

| Bit | Function | R/W |
|-----|----------|-----|
| 7 | Always 1 | R |
| 6 | LYC=LY interrupt enable | R/W |
| 5 | Mode 2 (OAM) interrupt enable | R/W |
| 4 | Mode 1 (VBlank) interrupt enable | R/W |
| 3 | Mode 0 (HBlank) interrupt enable | R/W |
| 2 | LYC=LY coincidence flag | R |
| 1–0 | Current mode | R |

Writes only modify bits 3–6. The mode and coincidence bits are set by the PPU.

## Palette Mapping

The DMG has three palette registers:
- **BGP** (0xFF47): background palette
- **OBP0** (0xFF48): sprite palette 0
- **OBP1** (0xFF49): sprite palette 1

Each palette maps 2-bit color IDs (0–3) to shades:

```
Bits 1-0: color 0 shade
Bits 3-2: color 1 shade
Bits 5-4: color 2 shade
Bits 7-6: color 3 shade

Shade values: 0=white (0xFF), 1=light (0xAA), 2=dark (0x55), 3=black (0x00)
```

The framebuffer stores these shade values as RGBA pixels (shade, shade, shade, 0xFF). The JS frontend applies additional palette coloring (green, custom, etc.) after reading the framebuffer.

## LCD Off Behavior

When LCDC bit 7 is cleared:
- LY resets to 0
- Mode resets to OAM scan
- Dot counter resets to 0
- All `tick()` calls become no-ops
