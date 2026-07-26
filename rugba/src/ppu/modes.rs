/// Bitmap PPU rendering modes (Mode 3, 4, 5).
const SCREEN_WIDTH: usize = 240;

/// Convert a GBA 15-bit RGB555 color to RGBA bytes.
#[inline(always)]
pub fn rgb555_to_rgba(color: u16) -> [u8; 4] {
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- rgb555_to_rgba ---

    #[test]
    fn rgb555_black() {
        assert_eq!(rgb555_to_rgba(0x0000), [0, 0, 0, 255]);
    }

    #[test]
    fn rgb555_white() {
        // All 5 bits set in each channel → 0x1F << 3 = 0xF8
        assert_eq!(rgb555_to_rgba(0x7FFF), [0xF8, 0xF8, 0xF8, 255]);
    }

    #[test]
    fn rgb555_red() {
        // Red = bits 4:0, value 0x001F
        assert_eq!(rgb555_to_rgba(0x001F), [0xF8, 0, 0, 255]);
    }

    #[test]
    fn rgb555_green() {
        // Green = bits 9:5, value 0x03E0
        assert_eq!(rgb555_to_rgba(0x03E0), [0, 0xF8, 0, 255]);
    }

    #[test]
    fn rgb555_blue() {
        // Blue = bits 14:10, value 0x7C00
        assert_eq!(rgb555_to_rgba(0x7C00), [0, 0, 0xF8, 255]);
    }

    // --- Mode 3 ---

    #[test]
    fn mode3_renders_pixel() {
        // FB: 240*160*4 bytes; VRAM: at least 2 bytes for pixel (0,0)
        let mut fb = vec![0u8; SCREEN_WIDTH * 160 * 4];
        // Red in RGB555: bits 4:0 = 0x1F → 0x001F little-endian
        let mut vram = vec![0u8; SCREEN_WIDTH * 160 * 2];
        vram[0] = 0x1F;
        vram[1] = 0x00;

        render_mode3_scanline(&mut fb, 0, &vram);

        assert_eq!(fb[0], 0xF8, "R component of pixel (0,0)");
        assert_eq!(fb[1], 0, "G component");
        assert_eq!(fb[2], 0, "B component");
        assert_eq!(fb[3], 255, "A component");
    }

    // --- Mode 4 ---

    #[test]
    fn mode4_indexed_color() {
        let mut fb = vec![0u8; SCREEN_WIDTH * 160 * 4];
        // VRAM: pixel at (0,0) has palette index 2
        let mut vram = vec![0u8; 0xA000 + SCREEN_WIDTH * 160];
        vram[0] = 2; // index 2 for page 0, pixel (0,0)

        // Palette: entry 2 = green (0x03E0 little-endian)
        let mut palette = vec![0u8; 512];
        palette[4] = 0xE0; // low byte of 0x03E0
        palette[5] = 0x03; // high byte

        render_mode4_scanline(&mut fb, 0, 0, &vram, &palette);

        assert_eq!(fb[0], 0, "R of green pixel");
        assert_eq!(fb[1], 0xF8, "G of green pixel");
        assert_eq!(fb[2], 0, "B of green pixel");
        assert_eq!(fb[3], 255, "A of green pixel");
    }

    #[test]
    fn mode4_page1_offset() {
        let mut fb = vec![0u8; SCREEN_WIDTH * 160 * 4];
        // Page 1 starts at VRAM offset 0xA000
        let mut vram = vec![0u8; 0xA000 + SCREEN_WIDTH * 160];
        // Set index at page1 offset for pixel (0,0)
        vram[0xA000] = 1;

        // Palette: entry 1 = red (0x001F)
        let mut palette = vec![0u8; 512];
        palette[2] = 0x1F;
        palette[3] = 0x00;

        // DISPCNT bit 4 set → page 1
        render_mode4_scanline(&mut fb, 0, 0x0010, &vram, &palette);

        assert_eq!(fb[0], 0xF8, "R of page1 pixel");
        assert_eq!(fb[1], 0, "G");
        assert_eq!(fb[2], 0, "B");
        assert_eq!(fb[3], 255, "A");
    }

    // --- Mode 5 ---

    #[test]
    fn mode5_160_wide_rest_black() {
        let mut fb = vec![0u8; SCREEN_WIDTH * 160 * 4];
        // VRAM page 0, line 0: set pixel 0 to blue
        let mut vram = vec![0u8; 0xA000 + 160 * 128 * 2];
        vram[0] = 0x00;
        vram[1] = 0x7C; // blue = 0x7C00

        render_mode5_scanline(&mut fb, 0, 0, &vram);

        // Pixels 160-239 (visible line 0) should be black
        for x in 160..SCREEN_WIDTH {
            let dst = x * 4;
            assert_eq!(fb[dst], 0, "pixel {} R should be 0", x);
            assert_eq!(fb[dst + 1], 0, "pixel {} G should be 0", x);
            assert_eq!(fb[dst + 2], 0, "pixel {} B should be 0", x);
            assert_eq!(fb[dst + 3], 0, "pixel {} A should be 0", x);
        }
        // Pixel 0 should be blue
        assert_eq!(fb[2], 0xF8, "blue pixel B channel");
    }

    #[test]
    fn mode5_lines_above_128_black() {
        let mut fb = vec![0xFF_u8; SCREEN_WIDTH * 160 * 4]; // pre-fill with non-zero
        let vram = vec![0xFFu8; 0xA000 + 160 * 128 * 2];

        // Line 128 should be entirely zeroed out
        render_mode5_scanline(&mut fb, 128, 0, &vram);

        let start = 128 * SCREEN_WIDTH * 4;
        for i in start..start + SCREEN_WIDTH * 4 {
            assert_eq!(fb[i], 0, "byte {} of line 128 should be 0", i - start);
        }
    }
}
