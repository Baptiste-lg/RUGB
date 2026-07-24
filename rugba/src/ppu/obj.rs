/// GBA sprite (OBJ) renderer — reads OAM and draws hardware sprites.

use super::modes::rgb555_to_rgba;

const SCREEN_WIDTH: usize = 240;

/// Sprite dimensions indexed by (shape, size).
/// Shape: 0=Square, 1=Horizontal, 2=Vertical.  Size: 0..3.
const OBJ_DIMS: [[(u16, u16); 4]; 3] = [
    // Square
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    // Horizontal
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    // Vertical
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

/// Render all visible OBJ sprites for the given scanline.
///
/// `dispcnt` — DISPCNT register value (needed for mapping mode and BG mode).
/// OBJ palette lives at palette RAM offset 0x200.
pub fn render_sprites(
    fb: &mut [u8],
    line: usize,
    dispcnt: u16,
    oam: &[u8],
    vram: &[u8],
    palette: &[u8],
) {
    let one_d_mapping = dispcnt & (1 << 6) != 0;
    let bg_mode = dispcnt & 0x07;
    // In bitmap modes 3-5 OBJ tile base is 0x10000, otherwise 0x00000.
    let tile_base: usize = if bg_mode >= 3 { 0x10000 } else { 0x00000 };
    let fb_row = line * SCREEN_WIDTH * 4;

    // Iterate all 128 OAM entries (lower index = higher priority when overlapping).
    // We draw back-to-front so that lower-index sprites overwrite higher-index ones.
    for i in (0..128).rev() {
        let base = i * 8;
        if base + 7 >= oam.len() {
            continue;
        }

        let attr0 = u16::from_le_bytes([oam[base], oam[base + 1]]);
        let attr1 = u16::from_le_bytes([oam[base + 2], oam[base + 3]]);
        let attr2 = u16::from_le_bytes([oam[base + 4], oam[base + 5]]);

        // GFX mode (bits 10-11 of attr0): 2 = disabled, 1/3 = affine (skip for now).
        let gfx_mode = (attr0 >> 8) & 0x03;
        if gfx_mode == 2 {
            continue; // OBJ disabled
        }

        // OBJ mode check — skip affine sprites.
        let affine = (attr0 >> 8) & 0x01 != 0;
        if affine {
            continue;
        }

        // Skip semi-transparent / OBJ-window for now (obj_mode bits 10-11).
        // We still render them as normal.

        let is_8bpp = attr0 & (1 << 13) != 0;
        let shape = ((attr0 >> 14) & 0x03) as usize;
        let size = ((attr1 >> 14) & 0x03) as usize;
        if shape > 2 {
            continue;
        }

        let (w, h) = OBJ_DIMS[shape][size];
        let (w, h) = (w as usize, h as usize);

        // Y coordinate — 8-bit, wraps at 256.
        let y = (attr0 & 0xFF) as usize;
        // Check if this sprite intersects the current scanline (with wrapping).
        let sprite_y = if y + h > 256 {
            // Wraps past bottom of OAM Y space.
            if line < (y + h) - 256 {
                // Visible via wrap: effective row is line + (256 - y).
                line + 256 - y
            } else if line >= y {
                line - y
            } else {
                continue;
            }
        } else if line >= y && line < y + h {
            line - y
        } else {
            continue;
        };

        // X coordinate — 9-bit signed.
        let x_raw = (attr1 & 0x1FF) as i32;
        let x_start = if x_raw >= 256 { x_raw - 512 } else { x_raw };

        let hflip = attr1 & (1 << 12) != 0;
        let vflip = attr1 & (1 << 13) != 0;

        let tile_num = (attr2 & 0x03FF) as usize;
        let pal_bank = ((attr2 >> 12) & 0x0F) as usize;

        // Apply vertical flip.
        let row = if vflip { h - 1 - sprite_y } else { sprite_y };
        let tile_row = row / 8;
        let pixel_y = row % 8;

        let tiles_wide = w / 8;

        for tx in 0..tiles_wide {
            let col_tile = if hflip { tiles_wide - 1 - tx } else { tx };

            // Calculate tile index in VRAM.
            let tile_idx = if one_d_mapping {
                if is_8bpp {
                    // 8bpp tiles are twice as large; tile_num counts in 32-byte units.
                    tile_num + tile_row * (tiles_wide * 2) + col_tile * 2
                } else {
                    tile_num + tile_row * tiles_wide + col_tile
                }
            } else {
                // 2D mapping: 32 tiles per row.
                if is_8bpp {
                    tile_num + tile_row * 32 + col_tile * 2
                } else {
                    tile_num + tile_row * 32 + col_tile
                }
            };

            let tile_addr = if is_8bpp {
                tile_base + tile_idx * 32 // each 8bpp tile = 64 bytes, but tile_idx already doubled
            } else {
                tile_base + tile_idx * 32
            };

            for px in 0..8u16 {
                let pixel_x = if hflip { 7 - px } else { px } as usize;
                let screen_x = x_start + (tx * 8 + px as usize) as i32;

                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i32 {
                    continue;
                }
                let sx = screen_x as usize;

                let color_idx = if is_8bpp {
                    let byte_off = tile_addr + pixel_y * 8 + pixel_x;
                    if byte_off < vram.len() {
                        vram[byte_off] as usize
                    } else {
                        0
                    }
                } else {
                    let byte_off = tile_addr + pixel_y * 4 + pixel_x / 2;
                    if byte_off < vram.len() {
                        let byte = vram[byte_off];
                        let nibble = if pixel_x & 1 == 0 {
                            byte & 0x0F
                        } else {
                            byte >> 4
                        };
                        nibble as usize
                    } else {
                        0
                    }
                };

                if color_idx == 0 {
                    continue; // Transparent
                }

                // OBJ palette starts at palette RAM offset 0x200.
                let pal_addr = if is_8bpp {
                    0x200 + color_idx * 2
                } else {
                    0x200 + pal_bank * 32 + color_idx * 2
                };

                if pal_addr + 1 >= palette.len() {
                    continue;
                }

                let color =
                    u16::from_le_bytes([palette[pal_addr], palette[pal_addr + 1]]);
                let rgba = rgb555_to_rgba(color);

                let dst = fb_row + sx * 4;
                fb[dst] = rgba[0];
                fb[dst + 1] = rgba[1];
                fb[dst + 2] = rgba[2];
                fb[dst + 3] = rgba[3];
            }
        }
    }
}
