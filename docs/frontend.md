# Frontend (Web)

**Files:** `web/index.html`, `web/style.css`, `web/js/index.js`

The frontend is a single-page web app that communicates with the Rust emulator core through `wasm-bindgen`. No build step is needed for the JS — it's a plain ES module loaded directly by the browser.

## WASM Bridge

The JS imports from the wasm-pack output:

```javascript
import init, { WasmEmulator } from '../pkg/rugb.js';
```

`init()` loads and initializes the WASM module. `WasmEmulator` wraps the Rust `Emulator` struct.

### Exposed Methods

| Method | Purpose |
|--------|---------|
| `new(rom: Uint8Array)` | Create emulator from ROM data |
| `run_frame()` | Execute one frame (70,224 T-cycles) |
| `framebuffer_ptr()` → pointer | Raw pointer to the 160x144 RGBA framebuffer |
| `set_button(index, pressed)` | Update button state (0–7) |
| `audio_buffer_ptr()` → pointer | Pointer to f32 sample buffer |
| `audio_buffer_len()` → usize | Number of f32 values in buffer |
| `audio_buffer_consume(count)` | Remove first `count` f32 values |
| `has_battery()` → bool | Does the cartridge have battery backup? |
| `battery_ram_ptr()` → pointer | Pointer to cartridge RAM |
| `battery_ram_len()` → usize | Size of cartridge RAM in bytes |
| `load_battery_ram(data)` | Restore cartridge RAM |
| `set_channel_mute(ch, muted)` | Mute/unmute APU channel (0–3) |
| `save_state()` → Vec<u8> | Serialize full emulator state |
| `load_state(data)` | Restore emulator from serialized state |
| `title()` → String | ROM title from cartridge header |

## Frame Loop

The emulator uses `requestAnimationFrame` with delta-time throttling to run at the correct speed regardless of the display refresh rate:

```javascript
function frame(timestamp) {
    // Calculate elapsed time
    let delta = timestamp - lastFrameTs;
    frameDebt += delta * speed;

    // Run as many frames as elapsed time allows
    while (frameDebt >= GB_FRAME_MS) {
        emu.run_frame();
        frameDebt -= GB_FRAME_MS;
    }

    // Render the last frame
    const ptr = emu.framebuffer_ptr();
    const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, 160*144*4);
    ctx.putImageData(new ImageData(new Uint8ClampedArray(pixels), 160, 144), 0, 0);

    requestAnimationFrame(frame);
}
```

`GB_FRAME_MS = 70224 / 4194304 * 1000 ≈ 16.74ms`

At 60 Hz, each RAF callback runs ~1 frame. At 144 Hz, some callbacks run 0 frames and some run 1. Fast forward mode sets `effectiveSpeed = 16` and allows up to 32 frames per callback.

## Audio Pipeline

```
APU (Rust)                    JS
──────────                    ──
tick() generates f32 samples → sample_buffer (Vec<f32>)
                                    │
                      ScriptProcessorNode.onaudioprocess
                                    │
                      ┌─────────────┴──────────────┐
                      │ Read via audio_buffer_ptr() │
                      │ De-interleave L/R           │
                      │ consume(count * 2)          │
                      └─────────────┬──────────────┘
                                    │
                              GainNode (volume)
                                    │
                              AudioContext.destination
```

### Latency Management

If the sample buffer grows beyond 80ms (~3,840 pairs), excess samples at the front are skipped to prevent audio latency from accumulating:

```javascript
if (samplePairs > AUDIO_SAMPLE_RATE * 0.08) {
    emu.audio_buffer_consume(excessPairs * 2);
}
```

### Partial Consume

Unlike a full `drain()`, `audio_buffer_consume(count)` only removes the samples that were actually read. Excess samples carry over to the next callback, eliminating gaps caused by timing jitter between the display and audio clocks.

## Input

### Button Mapping

| Index | Button | Default Key |
|-------|--------|-------------|
| 0 | Right | Arrow Right |
| 1 | Left | Arrow Left |
| 2 | Up | Arrow Up |
| 3 | Down | Arrow Down |
| 4 | A | Z |
| 5 | B | X |
| 6 | Start | Enter |
| 7 | Select | Shift |

### Visual Feedback

Each button element has the CSS class `gb-input` and a `data-btn` attribute. On press (keyboard or mouse), the element gets the `pressed` class, which triggers a CSS transition (background change, translateY for 3D effect).

### Gamepad

Polled every frame via `navigator.getGamepads()`. Supports remappable button bindings stored in localStorage. Left stick acts as D-pad with a 0.5 deadzone threshold.

### Turbo

Toggled with Q (turbo A) and W (turbo B). When active, the button is toggled on/off every frame in the frame loop.

## Save States

5 persistent slots stored in localStorage as base64-encoded JSON:

```json
{
    "data": "base64...",
    "timestamp": "7/13/2026, 10:30:00 AM",
    "title": "KIRBY DREAM LA"
}
```

Quick save (F5) / quick load (F8) use a separate in-memory slot (not persisted).

## Battery Save

For cartridges with battery backup, the external RAM is persisted to localStorage under the key `rugb-sram-{title}`. Auto-saved every 5 seconds and on page unload.

## ROM Loading

ROMs can be loaded via:
1. **File picker** — `<input type="file">` in the side menu
2. **Drag & drop** — anywhere on the page

ZIP files are detected by magic bytes (PK\x03\x04) and extracted using the Web Compression API (`DecompressionStream('deflate-raw')`). The first `.gb`, `.gbc`, or `.bin` file in the archive is loaded.

## Palettes

Four built-in palettes (green, gray, B&W, custom) applied in JS after reading the framebuffer. The PPU outputs shade values (0xFF/0xAA/0x55/0x00), and `applyPalette()` remaps them to the selected palette colors.

The custom palette uses four `<input type="color">` pickers, persisted in localStorage.

## Display Filters

Four filter modes applied via CSS:

| Filter | Method |
|--------|--------|
| None | Default pixelated rendering |
| Scanlines | CSS `::after` overlay with horizontal line pattern on `.gb-screen-frame` |
| LCD | CSS `::after` overlay with grid pattern simulating LCD subpixels |
| Smooth | Sets `image-rendering: auto` on canvas for bilinear interpolation |

Scanlines and LCD use `mix-blend-mode: multiply` with `pointer-events: none` so they don't intercept clicks. The selected filter is persisted to localStorage.

## Mobile Touch Controls

On touch-capable devices (`@media (pointer: coarse)`), an overlay with on-screen buttons appears at the bottom of the viewport:

- D-pad (grid layout: up/down/left/right)
- A and B buttons (right side)
- Start and Select (center bottom)

The Game Boy shell is hidden on mobile — only the screen and touch controls are shown. Touch events use `touchstart`/`touchend`/`touchcancel` with `preventDefault()` to avoid scrolling.

## localStorage Keys

| Key | Content |
|-----|---------|
| `rugb-palette` | Current palette name |
| `rugb-custom-palette` | Custom palette colors (JSON array) |
| `rugb-keymap` | Keyboard remapping (JSON) |
| `rugb-gpmap` | Gamepad remapping (JSON) |
| `rugb-view` | Display mode ("gb" or "screen") |
| `rugb-slot-{N}` | Save state slot N (JSON with base64 data) |
| `rugb-sram-{title}` | Battery RAM (base64) |
| `rugb-recent` | Recent ROM titles (JSON array) |
| `rugb-filter` | Display filter (none/scanlines/lcd/smooth) |
