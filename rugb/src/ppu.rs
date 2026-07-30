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

    // --- CGB color palette ---
    pub cgb_mode: bool,
    pub bg_palette_data: [u8; 64], // 8 palettes × 4 colors × 2 bytes (RGB555)
    pub obj_palette_data: [u8; 64], // 8 palettes × 4 colors × 2 bytes (RGB555)
    pub bg_palette_index: u8,      // BCPS: index + auto-increment (bit 7)
    pub obj_palette_index: u8,     // OCPS: index + auto-increment (bit 7)
    // --- CGB VRAM bank 1 (bank 0 is the existing vram) ---
    pub vram_bank1: [u8; 0x2000],
    pub vram_bank: u8, // VBK register (0 or 1)
    // --- HDMA (CGB high-speed DMA, TODO: implement transfer logic) ---
    #[allow(dead_code)]
    pub hdma_src: u16,
    #[allow(dead_code)]
    pub hdma_dst: u16,
    #[allow(dead_code)]
    pub hdma_len: u8,
    #[allow(dead_code)]
    pub hdma_active: bool,
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
            cgb_mode: false,
            bg_palette_data: [0; 64],
            obj_palette_data: [0; 64],
            bg_palette_index: 0,
            obj_palette_index: 0,
            vram_bank1: [0; 0x2000],
            vram_bank: 0,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_len: 0,
            hdma_active: false,
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

            if self.cgb_mode {
                let map_offset = (map_addr - 0x8000) as usize;
                let attr = self.vram_bank1[map_offset];
                let palette_num = (attr & 0x07) as usize;
                let tile_bank = (attr >> 3) & 1;
                let cgb_x_flip = attr & 0x20 != 0;
                let cgb_y_flip = attr & 0x40 != 0;

                let actual_pixel_row = if cgb_y_flip { 7 - pixel_row } else { pixel_row };
                let tile_addr = if signed_tile_ids {
                    let signed_id = tile_id as i8 as i16;
                    (0x9000u16 as i16 + signed_id * 16 + actual_pixel_row as i16 * 2) as u16
                } else {
                    tile_data_base + tile_id as u16 * 16 + actual_pixel_row * 2
                };

                let actual_pixel_col = if cgb_x_flip { 7 - pixel_col } else { pixel_col };
                let color_id = self.get_tile_pixel_banked(tile_addr, actual_pixel_col, tile_bank);
                self.bg_color_ids[x as usize] = color_id;
                let rgba =
                    Self::cgb_palette_color(&self.bg_palette_data, palette_num, color_id as usize);
                self.set_pixel_rgba(x as usize, self.ly as usize, rgba);
            } else {
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

        for x in 0..SCREEN_W as u8 {
            if x < wx {
                continue;
            }

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

            if self.cgb_mode {
                let map_offset = (map_addr - 0x8000) as usize;
                let attr = self.vram_bank1[map_offset];
                let palette_num = (attr & 0x07) as usize;
                let tile_bank = (attr >> 3) & 1;
                let cgb_x_flip = attr & 0x20 != 0;
                let cgb_y_flip = attr & 0x40 != 0;

                let actual_pixel_row = if cgb_y_flip { 7 - pixel_row } else { pixel_row };
                let actual_tile_addr = if signed_tile_ids {
                    let signed_id = tile_id as i8 as i16;
                    (0x9000u16 as i16 + signed_id * 16 + actual_pixel_row as i16 * 2) as u16
                } else {
                    tile_data_base + tile_id as u16 * 16 + actual_pixel_row * 2
                };

                let actual_pixel_col = if cgb_x_flip { 7 - pixel_col } else { pixel_col };
                let color_id =
                    self.get_tile_pixel_banked(actual_tile_addr, actual_pixel_col, tile_bank);
                self.bg_color_ids[x as usize] = color_id;
                let rgba =
                    Self::cgb_palette_color(&self.bg_palette_data, palette_num, color_id as usize);
                self.set_pixel_rgba(x as usize, self.ly as usize, rgba);
            } else {
                let color_id = self.get_tile_pixel(tile_addr, pixel_col);
                self.bg_color_ids[x as usize] = color_id;
                let shade = self.apply_palette(self.bgp, color_id);
                self.set_pixel(x as usize, self.ly as usize, shade);
            }
        }

        self.window_line += 1;
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

                if self.cgb_mode {
                    let cgb_palette_num = (attr & 0x07) as usize;
                    let cgb_tile_bank = (attr >> 3) & 1;
                    let bit = if x_flip { 7 - pixel_x } else { pixel_x };
                    let color_id = self.get_tile_pixel_banked(tile_addr, bit, cgb_tile_bank);

                    if color_id == 0 {
                        continue;
                    }
                    if bg_priority && self.bg_color_ids[screen_x] != 0 {
                        continue;
                    }

                    let rgba = Self::cgb_palette_color(
                        &self.obj_palette_data,
                        cgb_palette_num,
                        color_id as usize,
                    );
                    self.set_pixel_rgba(screen_x, self.ly as usize, rgba);
                } else {
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
    }

    // -- Helpers --

    /// Extract a 2-bit color ID from tile data at the given address and pixel column.
    /// GB tiles store 2 bytes per row: low bit in byte 0, high bit in byte 1.
    #[inline(always)]
    fn get_tile_pixel(&self, addr: u16, pixel_col: u8) -> u8 {
        let lo = self.vram[(addr - 0x8000) as usize];
        let hi = self.vram[(addr + 1 - 0x8000) as usize];
        let bit = 7 - pixel_col;
        ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1)
    }

    /// Map a 2-bit color ID through a palette register to get a shade index
    #[inline(always)]
    fn apply_palette(&self, palette: u8, color_id: u8) -> u8 {
        let shade_idx = (palette >> (color_id * 2)) & 0x03;
        SHADES[shade_idx as usize]
    }

    #[inline(always)]
    fn set_pixel(&mut self, x: usize, y: usize, shade: u8) {
        let idx = (y * SCREEN_W + x) * 4;
        if idx + 3 < self.framebuffer.len() {
            self.framebuffer[idx] = shade; // R
            self.framebuffer[idx + 1] = shade; // G
            self.framebuffer[idx + 2] = shade; // B
            self.framebuffer[idx + 3] = 0xFF; // A
        }
    }

    #[inline(always)]
    fn set_pixel_rgba(&mut self, x: usize, y: usize, rgba: [u8; 4]) {
        let idx = (y * SCREEN_W + x) * 4;
        self.framebuffer[idx] = rgba[0];
        self.framebuffer[idx + 1] = rgba[1];
        self.framebuffer[idx + 2] = rgba[2];
        self.framebuffer[idx + 3] = rgba[3];
    }

    fn cgb_palette_color(palette_data: &[u8], palette_num: usize, color_id: usize) -> [u8; 4] {
        let idx = (palette_num * 4 + color_id) * 2;
        let lo = palette_data[idx] as u16;
        let hi = palette_data[idx + 1] as u16;
        let rgb555 = lo | (hi << 8);
        let r = ((rgb555 & 0x1F) << 3) as u8;
        let g = (((rgb555 >> 5) & 0x1F) << 3) as u8;
        let b = (((rgb555 >> 10) & 0x1F) << 3) as u8;
        [r, g, b, 0xFF]
    }

    /// Extract a 2-bit color ID from tile data in a specific VRAM bank.
    #[inline(always)]
    fn get_tile_pixel_banked(&self, addr: u16, pixel_col: u8, bank: u8) -> u8 {
        let offset = (addr - 0x8000) as usize;
        let (lo, hi) = if bank == 1 {
            (self.vram_bank1[offset], self.vram_bank1[offset + 1])
        } else {
            (self.vram[offset], self.vram[offset + 1])
        };
        let bit = 7 - pixel_col;
        ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1)
    }

    // -- VRAM / OAM access --

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr - 0x8000) as usize]
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[(addr - 0x8000) as usize] = val;
    }

    pub fn read_vram_banked(&self, addr: u16) -> u8 {
        let offset = (addr - 0x8000) as usize;
        if self.vram_bank == 1 {
            self.vram_bank1[offset]
        } else {
            self.vram[offset]
        }
    }

    pub fn write_vram_banked(&mut self, addr: u16, val: u8) {
        let offset = (addr - 0x8000) as usize;
        if self.vram_bank == 1 {
            self.vram_bank1[offset] = val;
        } else {
            self.vram[(addr - 0x8000) as usize] = val;
        }
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
                // bit 7 always 1, bits 2-6 from stat, bits 0-1 from mode
                (self.stat & 0x7C) | mode_bits | 0x80
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
            0xFF4F => self.vram_bank | 0xFE,
            0xFF68 => self.bg_palette_index,
            0xFF69 => self.bg_palette_data[(self.bg_palette_index & 0x3F) as usize],
            0xFF6A => self.obj_palette_index,
            0xFF6B => self.obj_palette_data[(self.obj_palette_index & 0x3F) as usize],
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
            0xFF4F => self.vram_bank = val & 1,
            0xFF68 => self.bg_palette_index = val,
            0xFF69 => {
                let idx = (self.bg_palette_index & 0x3F) as usize;
                self.bg_palette_data[idx] = val;
                if self.bg_palette_index & 0x80 != 0 {
                    self.bg_palette_index = 0x80 | ((self.bg_palette_index.wrapping_add(1)) & 0x3F);
                }
            }
            0xFF6A => self.obj_palette_index = val,
            0xFF6B => {
                let idx = (self.obj_palette_index & 0x3F) as usize;
                self.obj_palette_data[idx] = val;
                if self.obj_palette_index & 0x80 != 0 {
                    self.obj_palette_index =
                        0x80 | ((self.obj_palette_index.wrapping_add(1)) & 0x3F);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Tick the PPU by `total` T-cycles in 4-cycle increments.
    fn tick_cycles(ppu: &mut Ppu, if_: &mut u8, total: u32) {
        let mut remaining = total;
        while remaining > 0 {
            let step = remaining.min(4);
            ppu.tick(step, if_);
            remaining -= step;
        }
    }

    /// Advance through exactly `n` complete scanlines (456 T-cycles each).
    fn tick_scanlines(ppu: &mut Ppu, if_: &mut u8, n: u32) {
        tick_cycles(ppu, if_, n * 456);
    }

    // -----------------------------------------------------------------------
    // Initial state
    // -----------------------------------------------------------------------

    #[test]
    fn ppu_initial_state() {
        let ppu = Ppu::new();
        // Mode bits in STAT: OamScan = 2
        let mode_bits = ppu.read_register(0xFF41) & 0x03;
        assert_eq!(mode_bits, 2, "initial mode should be OamScan (2)");
        assert_eq!(ppu.read_register(0xFF44), 0, "initial LY should be 0");
        // dots is private; verify indirectly — after 79 cycles still OamScan
        let mut ppu2 = Ppu::new();
        let mut if_ = 0u8;
        tick_cycles(&mut ppu2, &mut if_, 79);
        assert_eq!(
            ppu2.read_register(0xFF41) & 0x03,
            2,
            "still OamScan at 79 dots"
        );
    }

    // -----------------------------------------------------------------------
    // Mode transitions
    // -----------------------------------------------------------------------

    #[test]
    fn tick_oam_to_pixel_transfer() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        tick_cycles(&mut ppu, &mut if_, 80);
        let mode = ppu.read_register(0xFF41) & 0x03;
        assert_eq!(mode, 3, "after 80 dots should be PixelTransfer (3)");
    }

    #[test]
    fn tick_to_hblank() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // OamScan(80) + PixelTransfer(172) = 252 dots → HBlank
        tick_cycles(&mut ppu, &mut if_, 80 + 172);
        let mode = ppu.read_register(0xFF41) & 0x03;
        assert_eq!(mode, 0, "after 252 dots should be HBlank (0)");
    }

    #[test]
    fn tick_full_scanline_increments_ly() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        tick_scanlines(&mut ppu, &mut if_, 1);
        assert_eq!(
            ppu.read_register(0xFF44),
            1,
            "after 456 T-cycles LY should be 1"
        );
    }

    #[test]
    fn tick_to_vblank_at_144() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        tick_scanlines(&mut ppu, &mut if_, 144);
        let mode = ppu.read_register(0xFF41) & 0x03;
        assert_eq!(mode, 1, "after 144 scanlines should be VBlank (1)");
        assert_ne!(if_ & 0x01, 0, "VBlank interrupt (bit 0) should be set");
    }

    #[test]
    fn vblank_wraps_to_0() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // 154 scanlines = full frame; LY wraps back to 0 and mode returns to OamScan
        tick_scanlines(&mut ppu, &mut if_, 154);
        assert_eq!(
            ppu.read_register(0xFF44),
            0,
            "after 154 scanlines LY should wrap to 0"
        );
        let mode = ppu.read_register(0xFF41) & 0x03;
        assert_eq!(mode, 2, "after full frame should be back in OamScan (2)");
    }

    // -----------------------------------------------------------------------
    // STAT / LYC
    // -----------------------------------------------------------------------

    #[test]
    fn stat_lyc_match() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // Set LYC = 5
        ppu.write_register(0xFF45, 5);
        // Advance to scanline 5
        tick_scanlines(&mut ppu, &mut if_, 5);
        let stat = ppu.read_register(0xFF41);
        assert_ne!(
            stat & 0x04,
            0,
            "STAT bit 2 (LYC=LY) should be set on scanline 5"
        );
    }

    #[test]
    fn stat_lyc_irq_fires() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // Enable LYC=LY IRQ (STAT bit 6)
        ppu.write_register(0xFF41, 0x40);
        // Set LYC = 3
        ppu.write_register(0xFF45, 3);
        // Advance to scanline 3
        tick_scanlines(&mut ppu, &mut if_, 3);
        assert_ne!(
            if_ & 0x02,
            0,
            "STAT interrupt (bit 1 of IF) should fire on LYC=LY"
        );
    }

    #[test]
    fn stat_oam_irq() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // Enable OAM STAT IRQ (bit 5). We must advance to scanline 1 first so
        // the PPU enters OamScan again after being reset; then the IRQ fires on
        // the rising edge of the STAT line.
        ppu.write_register(0xFF41, 0x20);
        // Tick one full scanline so we enter OamScan for line 1
        tick_scanlines(&mut ppu, &mut if_, 1);
        assert_ne!(
            if_ & 0x02,
            0,
            "STAT OAM interrupt should fire when entering OamScan"
        );
    }

    #[test]
    fn stat_hblank_irq() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // Enable HBlank STAT IRQ (bit 3)
        ppu.write_register(0xFF41, 0x08);
        // Advance past OamScan + PixelTransfer to trigger HBlank
        tick_cycles(&mut ppu, &mut if_, 80 + 172);
        assert_ne!(
            if_ & 0x02,
            0,
            "STAT HBlank interrupt should fire on entering HBlank"
        );
    }

    // -----------------------------------------------------------------------
    // LCD disable
    // -----------------------------------------------------------------------

    #[test]
    fn lcd_disabled_noop() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;
        // Disable LCD (clear bit 7 of LCDC)
        ppu.write_register(0xFF40, 0x00);
        // Tick many cycles — nothing should change
        tick_cycles(&mut ppu, &mut if_, 1000);
        assert_eq!(
            ppu.read_register(0xFF44),
            0,
            "LY must stay 0 when LCD is off"
        );
        // Mode bits stay at OamScan (2) after LCD-off reset
        assert_eq!(
            ppu.read_register(0xFF41) & 0x03,
            2,
            "mode unchanged when LCD off"
        );
        assert_eq!(if_, 0, "no interrupts when LCD is off");
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    #[test]
    fn bg_tile_renders_pixel() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;

        // LCD on, BG on, BG tile data at 0x8000 (LCDC bit 4), BG map at 0x9800
        ppu.write_register(0xFF40, 0x91); // 0b10010001
                                          // Standard DMG palette: color 0→white(0xFF), 1→light, 2→dark, 3→black
        ppu.write_register(0xFF47, 0xE4); // 0b11100100

        // Write tile 0 data: all pixels = color ID 3 (both bits set → 0xFF, 0xFF per row)
        // Tile 0 lives at VRAM 0x0000 (bus addr 0x8000); 8 rows × 2 bytes = 16 bytes
        for row in 0..8u16 {
            ppu.write_vram(0x8000 + row * 2, 0xFF); // low bit plane
            ppu.write_vram(0x8000 + row * 2 + 1, 0xFF); // high bit plane
        }
        // Tile map entry 0 (top-left tile) already 0x00 (tile 0) — no write needed

        // Tick one full scanline to render line 0
        tick_scanlines(&mut ppu, &mut if_, 1);

        // Pixel (0,0): color ID 3 → palette 0xE4 → shade index (0xE4 >> 6) & 3 = 3 → SHADES[3] = 0x00 (black)
        let fb_r = ppu.framebuffer[0];
        assert_eq!(
            fb_r, 0x00,
            "pixel (0,0) R channel should be black (0x00) for color ID 3"
        );
        assert_eq!(ppu.framebuffer[3], 0xFF, "alpha channel should be 0xFF");
    }

    #[test]
    fn sprite_renders_pixel() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;

        // LCD on, BG on, sprites on (LCDC bits 7,1,0 — bit 1 enables sprites)
        // 0x93 = 0b10010011: LCD on, sprites on, BG on, tile data at 0x8000
        ppu.write_register(0xFF40, 0x93);
        // BGP: map everything to white so sprite contrast is clear
        ppu.write_register(0xFF47, 0x00); // color 0→white,1→white,2→white,3→white
                                          // OBP0: color 1→black (for sprite pixel)
                                          // 0b00000001 = palette: c0→white,c1→dark,c2→light,c3→white ... let's use 0xFC
                                          // OBP0 = 0xFC: c3=black,c2=dark,c1=light,c0=transparent
        ppu.write_register(0xFF48, 0xFC);

        // Tile 1 for sprite: all pixels = color ID 1
        // Tile 1 at VRAM 0x0010 (bus 0x8010)
        for row in 0..8u16 {
            ppu.write_vram(0x8010 + row * 2, 0xFF); // low bit: all 1
            ppu.write_vram(0x8010 + row * 2 + 1, 0x00); // high bit: all 0 → color ID = 1
        }

        // OAM entry 0: Y=16 (screen y=0), X=8 (screen x=0), tile=1, attr=0
        ppu.write_oam(0xFE00, 16); // Y (sprite shows at screen y = Y-16 = 0)
        ppu.write_oam(0xFE01, 8); // X (sprite shows at screen x = X-8 = 0)
        ppu.write_oam(0xFE02, 1); // tile index 1
        ppu.write_oam(0xFE03, 0); // attributes: no flip, OBP0, no bg priority

        // Tick one full scanline
        tick_scanlines(&mut ppu, &mut if_, 1);

        // Pixel (0,0): sprite color ID 1 through OBP0=0xFC
        // shade_idx = (0xFC >> (1*2)) & 3 = (0xFC >> 2) & 3 = 0x3F & 3 = 3 → SHADES[3] = 0x00 (black)
        let fb_r = ppu.framebuffer[0];
        assert_eq!(fb_r, 0x00, "sprite pixel (0,0) should be black");
    }

    // -----------------------------------------------------------------------
    // CGB palette
    // -----------------------------------------------------------------------

    #[test]
    fn cgb_bg_palette_write_read() {
        let mut ppu = Ppu::new();

        // Write individual bytes by setting BCPS index explicitly each time
        // (no auto-increment: bit 7 of BCPS is clear)
        ppu.write_register(0xFF68, 0x00); // index 0
        ppu.write_register(0xFF69, 0xAB);

        ppu.write_register(0xFF68, 0x01); // index 1
        ppu.write_register(0xFF69, 0xCD);

        // Read back
        ppu.write_register(0xFF68, 0x00);
        assert_eq!(
            ppu.read_register(0xFF69),
            0xAB,
            "BCPD byte 0 should read back 0xAB"
        );
        ppu.write_register(0xFF68, 0x01);
        assert_eq!(
            ppu.read_register(0xFF69),
            0xCD,
            "BCPD byte 1 should read back 0xCD"
        );
    }

    #[test]
    fn cgb_bg_palette_auto_increment() {
        let mut ppu = Ppu::new();

        // BCPS with auto-increment bit set (bit 7)
        ppu.write_register(0xFF68, 0x80); // index 0, auto-increment
        ppu.write_register(0xFF69, 0x11);
        ppu.write_register(0xFF69, 0x22);
        ppu.write_register(0xFF69, 0x33);

        // Index should have auto-incremented to 3
        ppu.write_register(0xFF68, 0x00);
        assert_eq!(ppu.read_register(0xFF69), 0x11);
        ppu.write_register(0xFF68, 0x01);
        assert_eq!(ppu.read_register(0xFF69), 0x22);
        ppu.write_register(0xFF68, 0x02);
        assert_eq!(ppu.read_register(0xFF69), 0x33);
    }

    // -----------------------------------------------------------------------
    // STAT register read-only bits
    // -----------------------------------------------------------------------

    #[test]
    fn stat_register_bits() {
        let mut ppu = Ppu::new();

        // Bit 7 is always 1 in reads
        let stat = ppu.read_register(0xFF41);
        assert_ne!(stat & 0x80, 0, "STAT bit 7 should always read as 1");

        // Mode bits (0-1) are read-only; writing 0 to them should have no effect
        // In OamScan the mode bits read as 2; try to clear them via STAT write
        ppu.write_register(0xFF41, 0x00); // attempt to write mode bits to 0
        let stat_after = ppu.read_register(0xFF41);
        assert_eq!(stat_after & 0x03, stat & 0x03, "mode bits are read-only");
        assert_ne!(stat_after & 0x80, 0, "bit 7 still 1 after write");
    }

    // -----------------------------------------------------------------------
    // Save / load round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip() {
        let mut ppu = Ppu::new();
        let mut if_ = 0u8;

        // Advance to scanline 7 and set a custom LYC
        tick_scanlines(&mut ppu, &mut if_, 7);
        ppu.write_register(0xFF45, 42);

        // Serialize
        let mut buf = Vec::new();
        ppu.save_state(&mut buf);

        // Deserialize into a fresh PPU
        let mut ppu2 = Ppu::new();
        let mut slice: &[u8] = &buf;
        ppu2.load_state(&mut slice);

        assert_eq!(ppu2.read_register(0xFF44), 7, "loaded LY should be 7");
        assert_eq!(ppu2.read_register(0xFF45), 42, "loaded LYC should be 42");
    }
}
