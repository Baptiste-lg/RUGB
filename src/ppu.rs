//! PPU — Pixel Processing Unit
//!
//! Renders 160x144 pixels per frame via a scanline state machine.
//! Each frame is 154 scanlines (144 visible + 10 VBlank), each scanline is 456 T-cycles.

use crate::savestate::*;

const SCREEN_W: usize = 160;
const SCREEN_H: usize = 144;

// DMG palettes map 2-bit color IDs to shades
const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00]; // white, light, dark, black

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    OamScan,       // Mode 2 — 80 dots, scanning OAM for sprites on this line
    PixelTransfer, // Mode 3 — ~172 dots, pushing pixels to the LCD
    HBlank,        // Mode 0 — rest of the 456 dots
    VBlank,        // Mode 1 — scanlines 144-153
}

pub struct Ppu {
    mode: Mode,
    /// Dot counter within the current scanline (0-455)
    dots: u32,
    /// Internal window line counter — only incremented on lines where window was drawn
    window_line: u8,
    // LCD registers
    lcdc: u8, // 0xFF40 — LCD Control
    stat: u8, // 0xFF41 — LCD Status (mode bits are read-only)
    scy: u8,  // 0xFF42 — Scroll Y
    scx: u8,  // 0xFF43 — Scroll X
    ly: u8,   // 0xFF44 — Current scanline
    lyc: u8,  // 0xFF45 — LY Compare
    bgp: u8,  // 0xFF47 — BG Palette
    obp0: u8, // 0xFF48 — Sprite Palette 0
    obp1: u8, // 0xFF49 — Sprite Palette 1
    wy: u8,   // 0xFF4A — Window Y
    wx: u8,   // 0xFF4B — Window X

    vram: [u8; 0x2000],
    oam: [u8; 0xA0],

    /// Per-scanline raw BG/window color IDs (0-3) for sprite BG priority checks
    bg_color_ids: [u8; SCREEN_W],
    /// Previous STAT interrupt line state for edge detection
    stat_irq_line: bool,

    pub framebuffer: [u8; SCREEN_W * SCREEN_H * 4],
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            mode: Mode::OamScan,
            dots: 0,
            window_line: 0,
            lcdc: 0x91, // LCD on, BG on after boot
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            bg_color_ids: [0; SCREEN_W],
            stat_irq_line: false,
            framebuffer: [0; SCREEN_W * SCREEN_H * 4],
        }
    }

    /// Advance PPU by the given number of T-cycles.
    pub fn tick(&mut self, cycles: u32, interrupt_flag: &mut u8) {
        if self.lcdc & 0x80 == 0 {
            // LCD disabled — do nothing
            return;
        }

        self.dots += cycles;

        match self.mode {
            Mode::OamScan => {
                if self.dots >= 80 {
                    self.dots -= 80;
                    self.mode = Mode::PixelTransfer;
                }
            }
            Mode::PixelTransfer => {
                if self.dots >= 172 {
                    self.dots -= 172;
                    self.mode = Mode::HBlank;

                    // Render this scanline right as we enter HBlank
                    if (self.ly as usize) < SCREEN_H {
                        self.render_scanline();
                    }
                }
            }
            Mode::HBlank => {
                if self.dots >= 204 {
                    self.dots -= 204;
                    self.ly += 1;

                    if self.ly >= 144 {
                        self.mode = Mode::VBlank;
                        // VBlank interrupt (always fires, independent of STAT)
                        *interrupt_flag |= 0x01;
                        // Reset window line counter for next frame
                        self.window_line = 0;
                    } else {
                        self.mode = Mode::OamScan;
                    }

                    self.update_lyc_flag();
                }
            }
            Mode::VBlank => {
                if self.dots >= 456 {
                    self.dots -= 456;
                    self.ly += 1;

                    if self.ly > 153 {
                        self.ly = 0;
                        self.mode = Mode::OamScan;
                    }

                    self.update_lyc_flag();
                }
            }
        }

        // Edge-detected STAT interrupt: fire only on rising edge of the combined signal
        self.update_stat_irq(interrupt_flag);
    }

    /// Update the LYC=LY coincidence flag (does NOT fire interrupt directly)
    fn update_lyc_flag(&mut self) {
        if self.ly == self.lyc {
            self.stat |= 0x04;
        } else {
            self.stat &= !0x04;
        }
    }

    // -- Rendering --

    /// Compute the combined STAT interrupt line and fire on rising edge only.
    fn update_stat_irq(&mut self, interrupt_flag: &mut u8) {
        let line = (self.stat & 0x20 != 0 && self.mode == Mode::OamScan)
            || (self.stat & 0x10 != 0 && self.mode == Mode::VBlank)
            || (self.stat & 0x08 != 0 && self.mode == Mode::HBlank)
            || (self.stat & 0x40 != 0 && self.stat & 0x04 != 0);

        if line && !self.stat_irq_line {
            *interrupt_flag |= 0x02;
        }
        self.stat_irq_line = line;
    }

    fn render_scanline(&mut self) {
        self.bg_color_ids = [0; SCREEN_W];
        if self.lcdc & 0x01 != 0 {
            self.render_bg();
        }
        if self.lcdc & 0x20 != 0 {
            self.render_window();
        }
        if self.lcdc & 0x02 != 0 {
            self.render_sprites();
        }
    }

    fn render_bg(&mut self) {
        let tile_data_base: u16 = if self.lcdc & 0x10 != 0 {
            0x8000
        } else {
            0x8800
        };
        let tile_map_base: u16 = if self.lcdc & 0x08 != 0 {
            0x9C00
        } else {
            0x9800
        };
        let signed_tile_ids = self.lcdc & 0x10 == 0;

        let y = self.ly.wrapping_add(self.scy);
        let tile_row = (y / 8) as u16;
        let pixel_row = (y % 8) as u16;

        for x in 0..SCREEN_W as u8 {
            let scrolled_x = x.wrapping_add(self.scx);
            let tile_col = (scrolled_x / 8) as u16;
            let pixel_col = scrolled_x % 8;

            let map_addr = tile_map_base + tile_row * 32 + tile_col;
            let tile_id = self.vram[(map_addr - 0x8000) as usize];

            let tile_addr = if signed_tile_ids {
                // 0x8800 mode: tile_id is signed, 0 maps to 0x9000
                let signed_id = tile_id as i8 as i16;
                (0x9000u16 as i16 + signed_id * 16 + pixel_row as i16 * 2) as u16
            } else {
                tile_data_base + tile_id as u16 * 16 + pixel_row * 2
            };

            let color_id = self.get_tile_pixel(tile_addr, pixel_col);
            self.bg_color_ids[x as usize] = color_id;
            let shade = self.apply_palette(self.bgp, color_id);
            self.set_pixel(x as usize, self.ly as usize, shade);
        }
    }

    fn render_window(&mut self) {
        // Window is only drawn if current line is at or below WY
        if self.ly < self.wy {
            return;
        }
        let wx = self.wx.saturating_sub(7);

        let tile_data_base: u16 = if self.lcdc & 0x10 != 0 {
            0x8000
        } else {
            0x8800
        };
        let tile_map_base: u16 = if self.lcdc & 0x40 != 0 {
            0x9C00
        } else {
            0x9800
        };
        let signed_tile_ids = self.lcdc & 0x10 == 0;

        let win_y = self.window_line;
        let tile_row = (win_y / 8) as u16;
        let pixel_row = (win_y % 8) as u16;

        let mut drew_anything = false;

        for x in 0..SCREEN_W as u8 {
            if x < wx {
                continue;
            }
            drew_anything = true;

            let win_x = x - wx;
            let tile_col = (win_x / 8) as u16;
            let pixel_col = win_x % 8;

            let map_addr = tile_map_base + tile_row * 32 + tile_col;
            let tile_id = self.vram[(map_addr - 0x8000) as usize];

            let tile_addr = if signed_tile_ids {
                let signed_id = tile_id as i8 as i16;
                (0x9000u16 as i16 + signed_id * 16 + pixel_row as i16 * 2) as u16
            } else {
                tile_data_base + tile_id as u16 * 16 + pixel_row * 2
            };

            let color_id = self.get_tile_pixel(tile_addr, pixel_col);
            self.bg_color_ids[x as usize] = color_id;
            let shade = self.apply_palette(self.bgp, color_id);
            self.set_pixel(x as usize, self.ly as usize, shade);
        }

        if self.ly >= self.wy {
            self.window_line += 1;
        }
    }

    fn render_sprites(&mut self) {
        let tall_sprites = self.lcdc & 0x04 != 0;
        let sprite_height: u8 = if tall_sprites { 16 } else { 8 };

        // Collect visible sprites on stack (max 10 per scanline, no heap allocation)
        let mut sprites: [(u8, u8, u8, u8, usize); 10] = [(0, 0, 0, 0, 0); 10];
        let mut sprite_count = 0usize;
        for i in 0..40usize {
            let base = i * 4;
            let sy = self.oam[base].wrapping_sub(16);
            let sx = self.oam[base + 1].wrapping_sub(8);
            let tile = self.oam[base + 2];
            let attr = self.oam[base + 3];

            if self.ly.wrapping_sub(sy) < sprite_height {
                sprites[sprite_count] = (sy, sx, tile, attr, i);
                sprite_count += 1;
                if sprite_count >= 10 {
                    break;
                }
            }
        }

        let sprites = &mut sprites[..sprite_count];
        // DMG priority: lower X first, ties broken by lower OAM index
        sprites.sort_by(|a, b| a.1.cmp(&b.1).then(a.4.cmp(&b.4)));

        // Render in reverse order so higher-priority sprites overwrite lower
        for &(sy, sx, mut tile, attr, _) in sprites.iter().rev() {
            let palette = if attr & 0x10 != 0 {
                self.obp1
            } else {
                self.obp0
            };
            let x_flip = attr & 0x20 != 0;
            let y_flip = attr & 0x40 != 0;
            let bg_priority = attr & 0x80 != 0;

            let mut line = self.ly.wrapping_sub(sy) as u16;

            if tall_sprites {
                // 8x16 mode: top tile has bit 0 cleared, bottom has it set
                if y_flip {
                    line = 15 - line;
                }
                if line >= 8 {
                    tile |= 0x01;
                    line -= 8;
                } else {
                    tile &= 0xFE;
                }
            } else if y_flip {
                line = 7 - line;
            }

            let tile_addr = 0x8000u16 + tile as u16 * 16 + line * 2;

            for pixel_x in 0..8u8 {
                let screen_x = sx.wrapping_add(pixel_x) as usize;
                if screen_x >= SCREEN_W {
                    continue;
                }

                let bit = if x_flip { 7 - pixel_x } else { pixel_x };
                let color_id = self.get_tile_pixel(tile_addr, bit);

                // Color 0 is transparent for sprites
                if color_id == 0 {
                    continue;
                }

                // BG priority: sprite only visible over BG color 0
                if bg_priority && self.bg_color_ids[screen_x] != 0 {
                    continue;
                }

                let shade = self.apply_palette(palette, color_id);
                self.set_pixel(screen_x, self.ly as usize, shade);
            }
        }
    }

    // -- Helpers --

    /// Extract a 2-bit color ID from tile data at the given address and pixel column.
    /// GB tiles store 2 bytes per row: low bit in byte 0, high bit in byte 1.
    fn get_tile_pixel(&self, addr: u16, pixel_col: u8) -> u8 {
        let lo = self.vram[(addr - 0x8000) as usize];
        let hi = self.vram[(addr + 1 - 0x8000) as usize];
        let bit = 7 - pixel_col;
        ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1)
    }

    /// Map a 2-bit color ID through a palette register to get a shade index
    fn apply_palette(&self, palette: u8, color_id: u8) -> u8 {
        let shade_idx = (palette >> (color_id * 2)) & 0x03;
        SHADES[shade_idx as usize]
    }

    fn set_pixel(&mut self, x: usize, y: usize, shade: u8) {
        let idx = (y * SCREEN_W + x) * 4;
        if idx + 3 < self.framebuffer.len() {
            self.framebuffer[idx] = shade; // R
            self.framebuffer[idx + 1] = shade; // G
            self.framebuffer[idx + 2] = shade; // B
            self.framebuffer[idx + 3] = 0xFF; // A
        }
    }

    // -- VRAM / OAM access --

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr - 0x8000) as usize]
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[(addr - 0x8000) as usize] = val;
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr - 0xFE00) as usize]
    }

    pub fn write_oam(&mut self, addr: u16, val: u8) {
        self.oam[(addr - 0xFE00) as usize] = val;
    }

    // -- Register I/O --

    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => {
                let mode_bits = match self.mode {
                    Mode::HBlank => 0,
                    Mode::VBlank => 1,
                    Mode::OamScan => 2,
                    Mode::PixelTransfer => 3,
                };
                (self.stat & 0x7C) | mode_bits | 0x80 // bit 7 always 1, bits 2-6 from stat, bits 0-1 from mode
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF46 => 0xFF, // DMA register is write-only
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        let mode_byte = match self.mode {
            Mode::OamScan => 0u8,
            Mode::PixelTransfer => 1,
            Mode::HBlank => 2,
            Mode::VBlank => 3,
        };
        push_u8(d, mode_byte);
        push_u32(d, self.dots);
        push_u8(d, self.window_line);
        push_u8(d, self.lcdc);
        push_u8(d, self.stat);
        push_u8(d, self.scy);
        push_u8(d, self.scx);
        push_u8(d, self.ly);
        push_u8(d, self.lyc);
        push_u8(d, self.bgp);
        push_u8(d, self.obp0);
        push_u8(d, self.obp1);
        push_u8(d, self.wy);
        push_u8(d, self.wx);
        d.extend_from_slice(&self.vram);
        d.extend_from_slice(&self.oam);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.mode = match pop_u8(d) {
            0 => Mode::OamScan,
            1 => Mode::PixelTransfer,
            2 => Mode::HBlank,
            _ => Mode::VBlank,
        };
        self.dots = pop_u32(d);
        self.window_line = pop_u8(d);
        self.lcdc = pop_u8(d);
        self.stat = pop_u8(d);
        self.scy = pop_u8(d);
        self.scx = pop_u8(d);
        self.ly = pop_u8(d);
        self.lyc = pop_u8(d);
        self.bgp = pop_u8(d);
        self.obp0 = pop_u8(d);
        self.obp1 = pop_u8(d);
        self.wy = pop_u8(d);
        self.wx = pop_u8(d);
        self.vram.copy_from_slice(&d[..0x2000]);
        *d = &d[0x2000..];
        self.oam.copy_from_slice(&d[..0xA0]);
        *d = &d[0xA0..];
    }

    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => {
                let was_on = self.lcdc & 0x80 != 0;
                self.lcdc = val;
                // Turning LCD off resets scanline state
                if was_on && val & 0x80 == 0 {
                    self.ly = 0;
                    self.dots = 0;
                    self.mode = Mode::OamScan;
                    self.window_line = 0;
                }
            }
            0xFF41 => {
                // Only bits 3-6 are writable, bits 0-2 and 7 are read-only
                self.stat = (self.stat & 0x87) | (val & 0x78);
            }
            0xFF42 => self.scy = val,
            0xFF43 => self.scx = val,
            0xFF44 => {} // LY is read-only
            0xFF45 => self.lyc = val,
            0xFF46 => {} // DMA handled in MMU before this is called
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy = val,
            0xFF4B => self.wx = val,
            _ => {}
        }
    }
}
