# RUGB — Bug & Security Audit Plan

## Objective

Exhaustive audit of the full RUGB codebase for correctness bugs, security vulnerabilities, and robustness issues. Covers both Rust emulator cores (rugb + rugba) and the web frontend (HTML/CSS/JS).

**Rules:**
- Read every file before changing it.
- One fix per commit using `[FIX]`/`[SEC]`/`[OPT]` norm.
- Fetch Pan Docs / GBATEK references for any hardware behavior claims.
- Do not add features. Fix what exists.

---

## PHASE 1: WASM Memory Safety

### Files: `rugb/src/lib.rs`, `rugba/src/lib.rs`

1. **Framebuffer pointer aliasing** — Verify `framebuffer_ptr()` returns a stable pointer that isn't invalidated by `run_frame()`. Check if WASM memory can grow between calls (if Vec/Box reallocates).
2. **Audio ring buffer bounds** — Verify `audio_ring_consume(count)` cannot advance past write pointer. Check for integer overflow on `read_pos + count`.
3. **Save state deserialization** — `load_state()` deserializes arbitrary bytes into CPU/PPU/APU state. Verify all `pop_*` calls check remaining data length. A malformed save state should not panic or corrupt memory.
4. **Battery RAM load** — `load_battery_ram()` copies external data into cartridge RAM. Verify length is clamped to actual RAM size.
5. **ROM size validation** — Verify `from_rom()` doesn't panic on truncated ROMs (< 0x150 bytes). Test with empty/tiny files.

---

## PHASE 2: Web Frontend Security

### Files: `web/js/index.js`, `web/js/cloud-saves.js`, `web/js/emu-worker.js`, `web/js/link-cable.js`

1. **ROM title injection** — `sanitizeTitle()` strips non-printable chars, but verify it's used consistently everywhere titles appear in DOM (innerHTML, textContent, data attributes, localStorage keys).
2. **localStorage key injection** — Verify all localStorage keys built from user input (ROM title) are properly sanitized. Could a crafted ROM title overwrite other keys?
3. **IndexedDB input validation** — Verify `saveRomToLibrary()` and `loadRomFromLibrary()` don't accept excessively large data. Check if there's a size cap on stored ROMs.
4. **Worker message validation** — `emu-worker.js` accepts messages with `cmd` field. Verify the `onmessage` handler validates message shape and doesn't blindly access properties that could be undefined.
5. **Cloud saves token handling** — `cloud-saves.js` stores OAuth token in `sessionStorage`. Verify:
   - Token is not logged to console
   - Token is not included in error messages
   - Token is cleared on sign-out
   - `fetch()` calls only go to Google APIs (no SSRF via crafted URLs)
6. **WebRTC SDP validation** — `link-cable.js` processes SDP from peers. Verify:
   - SDP size cap is enforced
   - Type field is validated
   - DataChannel messages are validated (should be exactly 1 byte)
7. **Share link decompression** — `loadShareLink()` decompresses URL hash data. Verify:
   - Size cap before decompression (already 2MB — verify)
   - Size cap after decompression
   - No zip bomb via deflate ratio
8. **Cheat code injection** — `applyGameSharkCheats()` writes to emulator memory via `write_byte()`. Verify cheat parsing validates address ranges (should be within valid Game Boy memory map, not arbitrary).
9. **File input validation** — `loadFile()` processes uploaded files. Verify:
   - ZIP extraction has bomb cap (64MB)
   - IPS/BPS patch size limits
   - Double extension attacks (e.g., `rom.gb.exe`) don't cause issues
10. **CSP headers** — Check `Dockerfile` and any nginx config for Content-Security-Policy. Verify:
    - `script-src` doesn't allow `unsafe-eval` or `unsafe-inline`
    - `connect-src` is restricted appropriately
    - `worker-src` allows the emu-worker

---

## PHASE 3: Rust Emulator Correctness (rugb)

### CPU (`rugb/src/cpu/`)

1. **Opcode timing audit** — Spot-check 20 random opcodes against the SM83 opcode table for correct cycle counts. Focus on:
   - Memory-accessing opcodes (LD (HL),r should be 8, not 4)
   - 16-bit arithmetic (ADD HL,rr should be 8)
   - RST instructions (should be 16, not 32)
2. **Flag edge cases** — Test DAA with edge values: A=0x9A after ADD, A=0x00 after SUB with C set. Verify Z and C flags.
3. **HALT power-down** — Verify HALT with IME=1 and no pending interrupts correctly pauses until interrupt. Verify cycle counting during halt.
4. **DI/EI sequence** — Verify `DI; EI; <interrupt>` correctly delays IME enable by one instruction.

### PPU (`rugb/src/ppu.rs`)

5. **Sprite 10-per-line limit** — Verify only 10 sprites are selected per scanline (OAM scan collects max 10).
6. **Sprite X=0 clipping** — Sprites with X=1..7 should be partially visible (leftmost pixels clipped). Verify the rendering loop handles this.
7. **Window WX=0..6** — When WX < 7, the window should still render (shifted). Verify `wx.saturating_sub(7)` doesn't cause wrong positioning.
8. **STAT mode transitions** — Verify STAT register bits 0-1 correctly reflect current mode at each dot count.
9. **VBlank STAT interrupt** — Verify STAT mode 1 (VBlank) interrupt fires if bit 4 of STAT is set, in addition to the VBlank interrupt (IF bit 0).

### APU (`rugb/src/apu.rs`)

10. **Sweep overflow** — CH1 sweep: when new frequency > 2047, channel should disable. Verify the overflow check happens both on trigger (if shift > 0) and on each sweep tick.
11. **Length counter on trigger** — When triggering with length=0, length should reload to max (64 for CH1/2/4, 256 for CH3). Verify.
12. **Wave RAM access during playback** — On DMG, reading wave RAM while CH3 is playing returns the byte currently being read by the wave channel. Verify or note as known limitation.
13. **NR52 power-off** — Writing 0 to NR52 bit 7 should zero all NR registers except wave RAM. Verify.

### Timer (`rugb/src/timer.rs`)

14. **TAC write edge** — Changing TAC clock select while timer is enabled can produce a spurious falling edge. Verify this is handled.
15. **DIV write during enabled timer** — Already implemented (verified). Double-check the edge detection fires correctly.

### MMU (`rugb/src/mmu.rs`)

16. **OAM DMA bus conflicts** — During OAM DMA, only HRAM should be accessible. Verify DMA doesn't block HRAM reads.
17. **VRAM access during mode 3** — Reading VRAM during pixel transfer should return 0xFF. Verify or note as known limitation.

---

## PHASE 4: Rust Emulator Correctness (rugba)

### ARM7TDMI (`rugba/src/arm7tdmi/`)

1. **ARM multiply timing** — MUL/MLA timing depends on the value of Rs. Verify cycle count is correct (1S + mI where m depends on Rs magnitude).
2. **Thumb BL** — Two-instruction sequence (high/low half). Verify the first instruction stores the offset and the second completes the branch + sets LR.
3. **CPSR/SPSR restore** — When restoring CPSR from SPSR (e.g., on exception return), verify mode bits are validated and banked registers are swapped correctly.
4. **Undefined instruction exception** — Verify undefined ARM/THUMB instructions trigger the UND exception vector (0x04), not a panic.

### Bus (`rugba/src/bus.rs`)

5. **Unaligned access** — GBA supports unaligned reads with rotation. Verify `read32()` handles misaligned addresses (bits 0-1 cause byte rotation).
6. **Open bus behavior** — Reads from unmapped regions should return the last prefetched value, not 0. Verify or note as approximation.
7. **BIOS protection** — After boot, reads from BIOS region (0x00000000-0x00003FFF) should return the last BIOS read value, not actual BIOS data. Verify.

### PPU (`rugba/src/ppu/`)

8. **Priority between BG layers** — Verify BG priority bits are respected (lower number = higher priority when same priority value).
9. **Windowing** — Verify WIN0/WIN1/WINOBJ correctly mask pixels in/out of regions.
10. **Mosaic** — Verify mosaic effect is applied to BG and OBJ when enabled.

### DMA (`rugba/src/dma.rs`)

11. **DMA priority** — DMA channel 0 has highest priority. Verify channels are serviced in order.
12. **Sound DMA timing** — DMA channels 1/2 with FIFO timing should trigger on audio FIFO request. Verify.

### Timers (`rugba/src/timer.rs`)

13. **Timer cascade** — When a timer overflows and the next timer has cascade enabled, it should increment. Verify the cascade chain works for all 4 timers.

---

## PHASE 5: Frontend Robustness

### Files: `web/js/index.js`, `web/js/dock.js`, `web/js/debug-tools.js`

1. **Double ROM load** — Loading a new ROM while one is running. Verify:
   - Old animation frame is cancelled
   - Old audio context is not duplicated
   - Worker (if active) is terminated
   - Battery RAM is saved before switch
2. **Tab backgrounding** — Verify `requestAnimationFrame` stops firing when tab is hidden, and frame debt doesn't accumulate excessively (capped at 100ms delta).
3. **Resize edge cases** — Verify the gameboy shell doesn't break at minimum (280px) or very large (2000px+) sizes.
4. **Service worker update** — Verify `sw.js` correctly invalidates old caches on version bump. No stale assets served after deploy.
5. **Gamepad disconnect** — Verify `navigator.getGamepads()` handles null entries gracefully.
6. **Audio context resume** — On mobile, audio context must be resumed after user gesture. Verify `audioCtx.resume()` is called on first interaction.
7. **Memory leaks** — Check for event listeners that aren't cleaned up on ROM switch (canvas listeners, interval timers, etc.).

---

## PHASE 6: Docker & CI Security

### Files: `Dockerfile`, `.github/workflows/ci.yml`, `.github/workflows/Docker.yml`

1. **Base image pinning** — Verify Docker images use specific tags/digests, not `latest`.
2. **Multi-stage build** — Verify build artifacts don't leak into the final image (no Rust toolchain, no source code).
3. **nginx security headers** — Verify the final image serves with:
   - `X-Content-Type-Options: nosniff`
   - `X-Frame-Options: DENY`
   - `Referrer-Policy: no-referrer`
   - `Permissions-Policy` restricting sensors
4. **CI secrets** — Verify no secrets are logged or exposed in CI output.
5. **Dependency audit** — Run `cargo audit` and check for known vulnerabilities in dependencies.

---

## Execution Order

1. Phase 2 (Web Security) — highest impact, user-facing
2. Phase 1 (WASM Memory Safety) — crash/corruption risk
3. Phase 5 (Frontend Robustness) — UX stability
4. Phase 3 (rugb Correctness) — emulation accuracy
5. Phase 4 (rugba Correctness) — emulation accuracy
6. Phase 6 (Docker & CI) — deployment security

**Take your time. Read every line. Verify against documentation. One fix per commit.**
