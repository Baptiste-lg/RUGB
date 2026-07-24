/// GBA background tile rendering for Modes 0, 1 (text) and 2 (affine).
use super::modes::rgb555_to_rgba;

const SCREEN_WIDTH: usize = 240;

/// Decoded BG control register + scroll values, ready for the renderer.
pub struct BgControl {
    /// Priority (0 = highest, 3 = lowest)
    pub priority: u8,
    /// Character (tile data) base address in VRAM (bits 2-3 of BGCNT * 0x4000)
    pub char_base: u32,
    /// Mosaic enable
    pub mosaic: bool,
    /// True = 256-color / 1-palette (8bpp), false = 16-color / 16-palette (4bpp)
    pub palette_256: bool,
    /// Screen (map) base address in VRAM (bits 8-12 of BGCNT * 0x800)
    pub screen_base: u32,
    /// Affine overflow wrap (bit 13, affine BGs only)
    pub overflow_wrap: bool,
    /// Screen size selector (bits 14-15)
    pub size: u8,
    /// Horizontal scroll offset (9 bits, text BGs)
    pub scroll_x: u16,
    /// Vertical scroll offset (9 bits, text BGs)
    pub scroll_y: u16,
}

impl BgControl {
    /// Decode a raw 16-bit BGCNT register value plus scroll offsets.
    pub fn from_raw(bgcnt: u16, hofs: u16, vofs: u16) -> Self {
        BgControl {
            priority: (bgcnt & 0x03) as u8,
            char_base: ((bgcnt >> 2) & 0x03) as u32 * 0x4000,
            mosaic: bgcnt & 0x40 != 0,
            palette_256: bgcnt & 0x80 != 0,
            screen_base: ((bgcnt >> 8) & 0x1F) as u32 * 0x800,
            overflow_wrap: bgcnt & 0x2000 != 0,
            size: ((bgcnt >> 14) & 0x03) as u8,
            scroll_x: hofs & 0x1FF,
            scroll_y: vofs & 0x1FF,
        }
    }
}

/// Text BG screen dimensions in pixels, by size field.
fn text_bg_dimensions(size: u8) -> (usize, usize) {
    match size {
        0 => (256, 256),
        1 => (512, 256),
        2 => (256, 512),
        3 => (512, 512),
        _ => (256, 256),
    }
}

/// Return the screen-block offset for a given tile coordinate in a text BG.
///
/// Text BGs larger than 32x32 tiles are arranged in 32x32-tile "screen blocks":
///   size 0 (32x32): block 0
///   size 1 (64x32): blocks 0,1 left-to-right
///   size 2 (32x64): blocks 0,1 top-to-bottom
///   size 3 (64x64): blocks 0,1 (top row), 2,3 (bottom row)
///
/// Each screen block is 0x800 bytes (32 * 32 * 2).
#[inline]
fn screen_entry_offset(tile_col: usize, tile_row: usize, size: u8) -> usize {
    let tiles_wide = if size & 1 != 0 { 64 } else { 32 };
    // Which screen block column/row
    let block_col = tile_col / 32;
    let block_row = tile_row / 32;
    // Index within the 32x32 block
    let local_col = tile_col % 32;
    let local_row = tile_row % 32;
    // Screen block index
    let block_idx = match size {
        0 => 0,
        1 => block_col,
        2 => block_row,
        3 => block_row * 2 + block_col,
        _ => 0,
    };
    let _ = tiles_wide; // suppress unused warning
    block_idx * 0x800 + (local_row * 32 + local_col) * 2
}

/// Read a little-endian u16 from a byte slice (with bounds check).
#[inline]
fn read_u16(data: &[u8], offset: usize) -> u16 {
    if offset + 1 < data.len() {
        (data[offset] as u16) | ((data[offset + 1] as u16) << 8)
    } else {
        0
    }
}

/// Look up a palette color and convert to RGBA.
#[inline]
fn palette_color(palette: &[u8], index: usize) -> [u8; 4] {
    let addr = index * 2;
    let color = read_u16(palette, addr);
    rgb555_to_rgba(color)
}

/// Render a single scanline for a text-mode background (Modes 0 and 1).
///
/// Writes into `fb` at the correct scanline offset. Pixels with color index 0
/// (transparent) are skipped, allowing lower-priority layers to show through.
pub fn render_text_bg(fb: &mut [u8], line: usize, bg: &BgControl, vram: &[u8], palette: &[u8]) {
    let (screen_w, screen_h) = text_bg_dimensions(bg.size);
    let eff_y = (line + bg.scroll_y as usize) % screen_h;
    let tile_row = eff_y / 8;
    let py = eff_y % 8; // pixel row within tile

    let fb_offset = line * SCREEN_WIDTH * 4;

    for x in 0..SCREEN_WIDTH {
        let eff_x = (x + bg.scroll_x as usize) % screen_w;
        let tile_col = eff_x / 8;
        let px = eff_x % 8; // pixel column within tile

        // Read screen entry
        let se_off = bg.screen_base as usize + screen_entry_offset(tile_col, tile_row, bg.size);
        let entry = read_u16(vram, se_off);

        let tile_idx = (entry & 0x03FF) as usize;
        let hflip = entry & 0x0400 != 0;
        let vflip = entry & 0x0800 != 0;
        let pal_num = ((entry >> 12) & 0x0F) as usize;

        // Apply flip to pixel coordinates within tile
        let fx = if hflip { 7 - px } else { px };
        let fy = if vflip { 7 - py } else { py };

        let color_idx = if bg.palette_256 {
            // 8bpp: 64 bytes per tile, 8 bytes per row
            let tile_addr = bg.char_base as usize + tile_idx * 64 + fy * 8 + fx;
            if tile_addr < vram.len() {
                vram[tile_addr] as usize
            } else {
                0
            }
        } else {
            // 4bpp: 32 bytes per tile, 4 bytes per row (2 pixels per byte)
            let tile_addr = bg.char_base as usize + tile_idx * 32 + fy * 4 + fx / 2;
            if tile_addr < vram.len() {
                let byte = vram[tile_addr];
                if fx & 1 == 0 {
                    (byte & 0x0F) as usize
                } else {
                    (byte >> 4) as usize
                }
            } else {
                0
            }
        };

        // Color index 0 is transparent
        if color_idx == 0 {
            continue;
        }

        let pal_index = if bg.palette_256 {
            color_idx
        } else {
            pal_num * 16 + color_idx
        };

        let rgba = palette_color(palette, pal_index);
        let dst = fb_offset + x * 4;
        fb[dst] = rgba[0];
        fb[dst + 1] = rgba[1];
        fb[dst + 2] = rgba[2];
        fb[dst + 3] = rgba[3];
    }
}

/// Render a single scanline for an affine (rotation/scaling) background (Mode 1/2).
///
/// Affine parameters should be passed via a dedicated struct in the future.
/// For now this is a stub that fills the scanline with the backdrop color (palette[0]).
pub fn render_affine_bg(fb: &mut [u8], line: usize, _bg: &BgControl, _vram: &[u8], palette: &[u8]) {
    // TODO: implement affine BG rendering with PA/PB/PC/PD + reference point
    // For now, fill with backdrop (palette entry 0) so the screen isn't garbage.
    let backdrop = palette_color(palette, 0);
    let fb_offset = line * SCREEN_WIDTH * 4;
    for x in 0..SCREEN_WIDTH {
        let dst = fb_offset + x * 4;
        fb[dst] = backdrop[0];
        fb[dst + 1] = backdrop[1];
        fb[dst + 2] = backdrop[2];
        fb[dst + 3] = backdrop[3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgcontrol_from_raw_parses_correctly() {
        // BGCNT = 0x4881: priority=1, char_base=0*0x4000=0, mosaic=false,
        //   palette_256=true, screen_base=9*0x800=0x4800, size=1
        // Actually let's build it manually:
        // bits 0-1: priority = 2 => 0x02
        // bits 2-3: char_base = 1 => 0x04
        // bit 6: mosaic = 0
        // bit 7: palette_256 = 1 => 0x80
        // bits 8-12: screen_base = 5 => 0x0500
        // bits 14-15: size = 2 => 0x8000
        let bgcnt: u16 = 0x02 | 0x04 | 0x80 | 0x0500 | 0x8000;
        let bg = BgControl::from_raw(bgcnt, 0x123, 0x1FF);
        assert_eq!(bg.priority, 2);
        assert_eq!(bg.char_base, 0x4000);
        assert!(bg.palette_256);
        assert_eq!(bg.screen_base, 5 * 0x800);
        assert_eq!(bg.size, 2);
        assert_eq!(bg.scroll_x, 0x123);
        assert_eq!(bg.scroll_y, 0x1FF);
    }

    #[test]
    fn screen_entry_offset_size0() {
        // 32x32 single screen, tile (5, 3) => offset (3*32+5)*2
        let off = screen_entry_offset(5, 3, 0);
        assert_eq!(off, (3 * 32 + 5) * 2);
    }

    #[test]
    fn screen_entry_offset_size1_second_block() {
        // 64x32: tile col 35 is in block 1
        let off = screen_entry_offset(35, 2, 1);
        // block 1 at 0x800, local col=3, local row=2
        assert_eq!(off, 0x800 + (2 * 32 + 3) * 2);
    }

    #[test]
    fn transparent_pixel_not_written() {
        // Create a minimal VRAM with all-zero tile data (color index 0 = transparent)
        let vram = vec![0u8; 0x10000];
        let palette = vec![0u8; 512];
        let mut fb = vec![0xFFu8; SCREEN_WIDTH * 160 * 4]; // fill with white

        let bg = BgControl::from_raw(0, 0, 0);
        render_text_bg(&mut fb, 0, &bg, &vram, &palette);

        // Framebuffer should remain 0xFF (unchanged) since all pixels are transparent
        assert_eq!(fb[0], 0xFF);
        assert_eq!(fb[3], 0xFF);
    }
}
