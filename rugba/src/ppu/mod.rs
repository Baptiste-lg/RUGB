pub mod bg;
pub mod blend;
pub mod modes;
pub mod obj;
#[cfg(test)]
mod tests;

use crate::io::IoRegisters;

const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const CYCLES_PER_DOT: u32 = 4;
const HDRAW_DOTS: u32 = 240;
const HBLANK_DOTS: u32 = 68;
const SCANLINE_DOTS: u32 = HDRAW_DOTS + HBLANK_DOTS; // 308
const VDRAW_LINES: u32 = 160;
const VBLANK_LINES: u32 = 68;
const TOTAL_LINES: u32 = VDRAW_LINES + VBLANK_LINES; // 228

/// Cycles per scanline (308 dots × 4 cycles/dot = 1232)
pub const CYCLES_PER_SCANLINE: u32 = SCANLINE_DOTS * CYCLES_PER_DOT;
/// Total cycles per frame (228 lines × 1232 cycles = 280896)
pub const CYCLES_PER_FRAME: u32 = TOTAL_LINES * CYCLES_PER_SCANLINE;

pub struct Ppu {
    pub framebuffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4]>,
    /// Cycle counter within current scanline (0..1232)
    dot_cycles: u32,
    /// Current scanline (0..227)
    line: u32,
    /// IRQ flags to raise (accumulated during tick, flushed to IO)
    pending_irqs: u16,
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            framebuffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT * 4]),
            dot_cycles: 0,
            line: 0,
            pending_irqs: 0,
        }
    }

    /// Advance the PPU by `cycles` T-cycles. Returns IRQ flags to raise.
    pub fn tick(
        &mut self,
        cycles: u32,
        io: &mut IoRegisters,
        vram: &[u8],
        palette: &[u8],
        oam: &[u8],
    ) -> u16 {
        self.pending_irqs = 0;
        let mut remaining = cycles;

        while remaining > 0 {
            let old_dot = self.dot_cycles;
            let to_end_of_phase = if old_dot < HDRAW_DOTS * CYCLES_PER_DOT {
                HDRAW_DOTS * CYCLES_PER_DOT - old_dot
            } else {
                SCANLINE_DOTS * CYCLES_PER_DOT - old_dot
            };

            let advance = remaining.min(to_end_of_phase);
            self.dot_cycles += advance;
            remaining -= advance;

            // Crossed into H-blank?
            if old_dot < HDRAW_DOTS * CYCLES_PER_DOT
                && self.dot_cycles >= HDRAW_DOTS * CYCLES_PER_DOT
            {
                // Render this scanline if visible
                if self.line < VDRAW_LINES {
                    self.render_scanline(io, vram, palette, oam);
                }
                // Set H-blank flag
                io.dispstat |= 0x02;
                if io.dispstat & 0x10 != 0 {
                    self.pending_irqs |= 0x02; // H-blank IRQ
                }
            }

            // End of scanline?
            if self.dot_cycles >= SCANLINE_DOTS * CYCLES_PER_DOT {
                self.dot_cycles -= SCANLINE_DOTS * CYCLES_PER_DOT;
                self.line += 1;

                if self.line >= TOTAL_LINES {
                    self.line = 0;
                }

                // Update VCOUNT
                io.vcount = self.line as u16;

                // Clear H-blank flag at start of new line
                io.dispstat &= !0x02;

                // V-blank start?
                if self.line == VDRAW_LINES {
                    io.dispstat |= 0x01; // V-blank flag
                    if io.dispstat & 0x08 != 0 {
                        self.pending_irqs |= 0x01; // V-blank IRQ
                    }
                }
                // V-blank end?
                if self.line == 0 {
                    io.dispstat &= !0x01;
                }

                // V-count match?
                let target = (io.dispstat >> 8) as u32;
                if self.line == target {
                    io.dispstat |= 0x04;
                    if io.dispstat & 0x20 != 0 {
                        self.pending_irqs |= 0x04; // V-count IRQ
                    }
                } else {
                    io.dispstat &= !0x04;
                }
            }
        }

        self.pending_irqs
    }

    fn render_scanline(&mut self, io: &IoRegisters, vram: &[u8], palette: &[u8], oam: &[u8]) {
        let mode = io.dispcnt & 0x07;
        let line = self.line as usize;

        match mode {
            0 | 1 | 2 => {
                // Clear scanline to backdrop color (palette[0])
                let backdrop = if palette.len() >= 2 {
                    let c = (palette[0] as u16) | ((palette[1] as u16) << 8);
                    modes::rgb555_to_rgba(c)
                } else {
                    [0, 0, 0, 255]
                };
                let start = line * SCREEN_WIDTH * 4;
                for x in 0..SCREEN_WIDTH {
                    let dst = start + x * 4;
                    self.framebuffer[dst] = backdrop[0];
                    self.framebuffer[dst + 1] = backdrop[1];
                    self.framebuffer[dst + 2] = backdrop[2];
                    self.framebuffer[dst + 3] = backdrop[3];
                }

                // Render BG layers by priority (3=lowest first, 0=highest last)
                for priority in (0..4).rev() {
                    for bg_idx in (0..4).rev() {
                        // Check if this BG is enabled in DISPCNT
                        if io.dispcnt & (1 << (8 + bg_idx)) == 0 {
                            continue;
                        }
                        let bgcnt = io.bgcnt[bg_idx];
                        if (bgcnt & 3) as usize != priority {
                            continue;
                        }
                        // Mode constraints: Mode 0 has BG0-3 text, Mode 1 has BG0-1 text + BG2 affine, Mode 2 has BG2-3 affine
                        let is_affine = match mode {
                            1 => bg_idx == 2,
                            2 => bg_idx >= 2,
                            _ => false,
                        };
                        let bgctrl =
                            bg::BgControl::from_raw(bgcnt, io.bghofs[bg_idx], io.bgvofs[bg_idx]);
                        if is_affine {
                            let affine = if bg_idx == 2 {
                                bg::AffineParams {
                                    pa: io.bg2pa,
                                    pb: io.bg2pb,
                                    pc: io.bg2pc,
                                    pd: io.bg2pd,
                                    ref_x: io.bg2x,
                                    ref_y: io.bg2y,
                                }
                            } else {
                                bg::AffineParams {
                                    pa: io.bg3pa,
                                    pb: io.bg3pb,
                                    pc: io.bg3pc,
                                    pd: io.bg3pd,
                                    ref_x: io.bg3x,
                                    ref_y: io.bg3y,
                                }
                            };
                            bg::render_affine_bg(
                                &mut self.framebuffer,
                                line,
                                &bgctrl,
                                vram,
                                palette,
                                &affine,
                            );
                        } else {
                            bg::render_text_bg(&mut self.framebuffer, line, &bgctrl, vram, palette);
                        }
                    }
                }
            }
            3 => modes::render_mode3_scanline(&mut self.framebuffer, line, vram),
            4 => {
                modes::render_mode4_scanline(&mut self.framebuffer, line, io.dispcnt, vram, palette)
            }
            5 => modes::render_mode5_scanline(&mut self.framebuffer, line, io.dispcnt, vram),
            _ => {
                let start = line * SCREEN_WIDTH * 4;
                self.framebuffer[start..start + SCREEN_WIDTH * 4].fill(0);
            }
        }

        // Render sprites on top (if OBJ enabled in DISPCNT bit 12)
        if io.dispcnt & (1 << 12) != 0 {
            obj::render_sprites(&mut self.framebuffer, line, io.dispcnt, oam, vram, palette);
        }

        // Apply brightness fade (BLDCNT mode 2 or 3)
        let blend_mode = (io.bldcnt >> 6) & 3;
        if blend_mode == 2 || blend_mode == 3 {
            let evy = (io.bldy & 0x1F) as u8;
            blend::apply_brightness(&mut self.framebuffer, line, blend_mode as u8, evy);
        }
    }
}
