/// I/O register file for GBA (0x04000000 - 0x040003FF).
/// Only the minimal set needed for Phase 1+2 (bitmap PPU + interrupts).
pub struct IoRegisters {
    /// 0x04000000 DISPCNT — Display control
    pub dispcnt: u16,
    /// 0x04000004 DISPSTAT — Display status (V-blank, H-blank, V-count match + IRQ enables)
    pub dispstat: u16,
    /// 0x04000006 VCOUNT — Current scanline (read-only, set by PPU)
    pub vcount: u16,
    /// 0x04000200 IE — Interrupt Enable
    pub ie: u16,
    /// 0x04000202 IF — Interrupt Request Flags (write 1 to acknowledge)
    pub irq_flags: u16,
    /// 0x04000208 IME — Interrupt Master Enable
    pub ime: u16,
    /// 0x04000300 POSTFLG
    pub postflg: u8,
    /// Is the CPU halted (waiting for interrupt)?
    pub halted: bool,
}

impl IoRegisters {
    pub fn new() -> Self {
        IoRegisters {
            dispcnt: 0,
            dispstat: 0,
            vcount: 0,
            ie: 0,
            irq_flags: 0,
            ime: 0,
            postflg: 0,
            halted: false,
        }
    }

    pub fn read16(&self, addr: u32) -> u16 {
        match addr & 0x3FF {
            0x000 => self.dispcnt,
            0x004 => self.dispstat | ((self.vcount & 0xFF) << 8),
            0x006 => self.vcount,
            0x130 => 0x03FF, // KEYINPUT placeholder — overridden by bus
            0x200 => self.ie,
            0x202 => self.irq_flags,
            0x208 => self.ime,
            _ => 0,
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        match addr & 0x3FF {
            0x000 => self.dispcnt = val,
            0x004 => {
                // Only bits 3-5 (IRQ enables) and bits 8-15 (V-count target) are writable
                self.dispstat = (self.dispstat & 0x07) | (val & 0xFFF8);
            }
            0x200 => self.ie = val,
            0x202 => {
                // Writing 1 acknowledges (clears) the flag
                self.irq_flags &= !val;
            }
            0x208 => self.ime = val & 1,
            0x300 => self.postflg = val as u8,
            0x301 => {
                // HALTCNT — halt CPU until interrupt
                self.halted = true;
            }
            _ => {}
        }
    }
}
