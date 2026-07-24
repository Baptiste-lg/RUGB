/// Bitmap PPU rendering modes (Mode 3, 4, 5).

const SCREEN_WIDTH: usize = 240;

/// Convert a GBA 15-bit RGB555 color to RGBA bytes.
#[inline(always)]
fn rgb555_to_rgba(color: u16) -> [u8; 4] {
    let r = ((color & 0x1F) << 3) as u8;
    let g = (((color >> 5) & 0x1F) << 3) as u8;
    let b = (((color >> 10) & 0x1F) << 3) as u8;
    [r, g, b, 255]
}

/// Mode 3: 240×160, direct 15-bit color (no palette). Single frame in VRAM.
pub fn render_mode3_scanline(fb: &mut [u8], line: usize, vram: &[u8]) {
    let vram_offset = line * SCREEN_WIDTH * 2;
    let fb_offset = line * SCREEN_WIDTH * 4;

    for x in 0..SCREEN_WIDTH {
        let addr = vram_offset + x * 2;
        let color = if addr + 1 < vram.len() {
            (vram[addr] as u16) | ((vram[addr + 1] as u16) << 8)
        } else {
            0
        };
        let rgba = rgb555_to_rgba(color);
        let dst = fb_offset + x * 4;
        fb[dst] = rgba[0];
        fb[dst + 1] = rgba[1];
        fb[dst + 2] = rgba[2];
        fb[dst + 3] = rgba[3];
    }
}

/// Mode 4: 240×160, 8-bit indexed (palette lookup). Double-buffered (page flip via DISPCNT bit 4).
pub fn render_mode4_scanline(
    fb: &mut [u8],
    line: usize,
    dispcnt: u16,
    vram: &[u8],
    palette: &[u8],
) {
    let page = if dispcnt & 0x10 != 0 { 0xA000 } else { 0 };
    let vram_offset = page + line * SCREEN_WIDTH;
    let fb_offset = line * SCREEN_WIDTH * 4;

    for x in 0..SCREEN_WIDTH {
        let addr = vram_offset + x;
        let index = if addr < vram.len() { vram[addr] } else { 0 };
        // Each palette entry is 2 bytes (RGB555)
        let pal_addr = (index as usize) * 2;
        let color = if pal_addr + 1 < palette.len() {
            (palette[pal_addr] as u16) | ((palette[pal_addr + 1] as u16) << 8)
        } else {
            0
        };
        let rgba = rgb555_to_rgba(color);
        let dst = fb_offset + x * 4;
        fb[dst] = rgba[0];
        fb[dst + 1] = rgba[1];
        fb[dst + 2] = rgba[2];
        fb[dst + 3] = rgba[3];
    }
}

/// Mode 5: 160×128, direct 15-bit color. Double-buffered, smaller resolution.
pub fn render_mode5_scanline(fb: &mut [u8], line: usize, dispcnt: u16, vram: &[u8]) {
    let fb_offset = line * SCREEN_WIDTH * 4;

    if line >= 128 {
        fb[fb_offset..fb_offset + SCREEN_WIDTH * 4].fill(0);
        return;
    }

    let page = if dispcnt & 0x10 != 0 { 0xA000 } else { 0 };
    let vram_offset = page + line * 160 * 2;

    // Render visible 160 pixels
    for x in 0..160 {
        let addr = vram_offset + x * 2;
        let color = if addr + 1 < vram.len() {
            (vram[addr] as u16) | ((vram[addr + 1] as u16) << 8)
        } else {
            0
        };
        let rgba = rgb555_to_rgba(color);
        let dst = fb_offset + x * 4;
        fb[dst] = rgba[0];
        fb[dst + 1] = rgba[1];
        fb[dst + 2] = rgba[2];
        fb[dst + 3] = rgba[3];
    }
    // Fill remaining 80 pixels with black
    let black_start = fb_offset + 160 * 4;
    fb[black_start..black_start + 80 * 4].fill(0);
}
