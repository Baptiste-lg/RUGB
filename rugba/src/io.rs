use crate::dma::DmaController;
use crate::timer::TimerController;

/// I/O register file for GBA (0x04000000 - 0x040003FF).
pub struct IoRegisters {
    /// 0x04000000 DISPCNT — Display control
    pub dispcnt: u16,
    /// 0x04000002 — Green swap (undocumented, unused)
    pub green_swap: u16,
    /// 0x04000004 DISPSTAT — Display status
    pub dispstat: u16,
    /// 0x04000006 VCOUNT — Current scanline (read-only, set by PPU)
    pub vcount: u16,
    /// BG control registers (BG0CNT..BG3CNT at 0x08, 0x0A, 0x0C, 0x0E)
    pub bgcnt: [u16; 4],
    /// BG horizontal scroll (BG0HOFS..BG3HOFS at 0x10, 0x14, 0x18, 0x1C)
    pub bghofs: [u16; 4],
    /// BG vertical scroll (BG0VOFS..BG3VOFS at 0x12, 0x16, 0x1A, 0x1E)
    pub bgvofs: [u16; 4],
    /// BG2 affine parameters (PA, PB, PC, PD at 0x20-0x26)
    pub bg2pa: i16,
    pub bg2pb: i16,
    pub bg2pc: i16,
    pub bg2pd: i16,
    /// BG2 reference point (0x28-0x2F, 28-bit signed)
    pub bg2x: i32,
    pub bg2y: i32,
    /// BG3 affine parameters
    pub bg3pa: i16,
    pub bg3pb: i16,
    pub bg3pc: i16,
    pub bg3pd: i16,
    /// BG3 reference point
    pub bg3x: i32,
    pub bg3y: i32,
    /// Window registers (0x40-0x4B)
    pub win0h: u16,
    pub win1h: u16,
    pub win0v: u16,
    pub win1v: u16,
    pub winin: u16,
    pub winout: u16,
    /// Blend control (0x50-0x54)
    pub bldcnt: u16,
    pub bldalpha: u16,
    pub bldy: u16,
    /// DMA controller
    pub dma: DmaController,
    /// Timer controller
    pub timers: TimerController,
    /// 0x04000200 IE — Interrupt Enable
    pub ie: u16,
    /// 0x04000202 IF — Interrupt Request Flags
    pub irq_flags: u16,
    /// 0x04000208 IME — Interrupt Master Enable
    pub ime: u16,
    /// 0x04000300 POSTFLG
    pub postflg: u8,
    /// Is the CPU halted?
    pub halted: bool,
}

impl IoRegisters {
    pub fn new() -> Self {
        IoRegisters {
            dispcnt: 0,
            green_swap: 0,
            dispstat: 0,
            vcount: 0,
            bgcnt: [0; 4],
            bghofs: [0; 4],
            bgvofs: [0; 4],
            bg2pa: 0x100, // 1.0 in 8.8 fixed point
            bg2pb: 0,
            bg2pc: 0,
            bg2pd: 0x100,
            bg2x: 0,
            bg2y: 0,
            bg3pa: 0x100,
            bg3pb: 0,
            bg3pc: 0,
            bg3pd: 0x100,
            bg3x: 0,
            bg3y: 0,
            win0h: 0,
            win1h: 0,
            win0v: 0,
            win1v: 0,
            winin: 0,
            winout: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            dma: DmaController::new(),
            timers: TimerController::new(),
            ie: 0,
            irq_flags: 0,
            ime: 0,
            postflg: 0,
            halted: false,
        }
    }

    pub fn read16(&self, addr: u32) -> u16 {
        match addr & 0x3FE {
            0x000 => self.dispcnt,
            0x002 => self.green_swap,
            0x004 => self.dispstat,
            0x006 => self.vcount,
            0x008 => self.bgcnt[0],
            0x00A => self.bgcnt[1],
            0x00C => self.bgcnt[2],
            0x00E => self.bgcnt[3],
            // Scroll registers are write-only
            0x010..=0x01E => 0,
            // Affine parameters are write-only
            0x020..=0x03E => 0,
            // Window registers are write-only
            0x040..=0x04A => 0,
            0x050 => self.bldcnt,
            0x052 => self.bldalpha,
            // DMA registers (0x0B0-0x0DE) — most are write-only
            0x0BA => self.dma.channels[0].ctrl,
            0x0C6 => self.dma.channels[1].ctrl,
            0x0D2 => self.dma.channels[2].ctrl,
            0x0DE => self.dma.channels[3].ctrl,
            // Timer counters (readable)
            0x100 => self.timers.timers[0].counter,
            0x102 => self.timers.timers[0].ctrl,
            0x104 => self.timers.timers[1].counter,
            0x106 => self.timers.timers[1].ctrl,
            0x108 => self.timers.timers[2].counter,
            0x10A => self.timers.timers[2].ctrl,
            0x10C => self.timers.timers[3].counter,
            0x10E => self.timers.timers[3].ctrl,
            0x130 => 0x03FF, // KEYINPUT — overridden by bus
            0x200 => self.ie,
            0x202 => self.irq_flags,
            0x208 => self.ime,
            _ => 0,
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        match addr & 0x3FE {
            0x000 => self.dispcnt = val,
            0x002 => self.green_swap = val,
            0x004 => {
                self.dispstat = (self.dispstat & 0x07) | (val & 0xFFF8);
            }
            0x008 => self.bgcnt[0] = val,
            0x00A => self.bgcnt[1] = val,
            0x00C => self.bgcnt[2] = val,
            0x00E => self.bgcnt[3] = val,
            0x010 => self.bghofs[0] = val & 0x1FF,
            0x012 => self.bgvofs[0] = val & 0x1FF,
            0x014 => self.bghofs[1] = val & 0x1FF,
            0x016 => self.bgvofs[1] = val & 0x1FF,
            0x018 => self.bghofs[2] = val & 0x1FF,
            0x01A => self.bgvofs[2] = val & 0x1FF,
            0x01C => self.bghofs[3] = val & 0x1FF,
            0x01E => self.bgvofs[3] = val & 0x1FF,
            0x020 => self.bg2pa = val as i16,
            0x022 => self.bg2pb = val as i16,
            0x024 => self.bg2pc = val as i16,
            0x026 => self.bg2pd = val as i16,
            0x028 => {
                self.bg2x = (self.bg2x & !0xFFFF) | val as i32;
                self.bg2x = (self.bg2x << 4) >> 4; // Sign extend 28-bit
            }
            0x02A => {
                self.bg2x = (self.bg2x & 0xFFFF) | ((val as i32) << 16);
                self.bg2x = (self.bg2x << 4) >> 4;
            }
            0x02C => {
                self.bg2y = (self.bg2y & !0xFFFF) | val as i32;
                self.bg2y = (self.bg2y << 4) >> 4;
            }
            0x02E => {
                self.bg2y = (self.bg2y & 0xFFFF) | ((val as i32) << 16);
                self.bg2y = (self.bg2y << 4) >> 4;
            }
            0x030 => self.bg3pa = val as i16,
            0x032 => self.bg3pb = val as i16,
            0x034 => self.bg3pc = val as i16,
            0x036 => self.bg3pd = val as i16,
            0x038 => {
                self.bg3x = (self.bg3x & !0xFFFF) | val as i32;
                self.bg3x = (self.bg3x << 4) >> 4;
            }
            0x03A => {
                self.bg3x = (self.bg3x & 0xFFFF) | ((val as i32) << 16);
                self.bg3x = (self.bg3x << 4) >> 4;
            }
            0x03C => {
                self.bg3y = (self.bg3y & !0xFFFF) | val as i32;
                self.bg3y = (self.bg3y << 4) >> 4;
            }
            0x03E => {
                self.bg3y = (self.bg3y & 0xFFFF) | ((val as i32) << 16);
                self.bg3y = (self.bg3y << 4) >> 4;
            }
            0x040 => self.win0h = val,
            0x042 => self.win1h = val,
            0x044 => self.win0v = val,
            0x046 => self.win1v = val,
            0x048 => self.winin = val,
            0x04A => self.winout = val,
            0x050 => self.bldcnt = val,
            0x052 => self.bldalpha = val,
            0x054 => self.bldy = val,
            // DMA source/dest/count/ctrl
            0x0B0 => self.dma.channels[0].src = (self.dma.channels[0].src & !0xFFFF) | val as u32,
            0x0B2 => {
                self.dma.channels[0].src =
                    (self.dma.channels[0].src & 0xFFFF) | ((val as u32 & 0x07FF) << 16);
            }
            0x0B4 => self.dma.channels[0].dst = (self.dma.channels[0].dst & !0xFFFF) | val as u32,
            0x0B6 => {
                self.dma.channels[0].dst =
                    (self.dma.channels[0].dst & 0xFFFF) | ((val as u32 & 0x07FF) << 16);
            }
            0x0B8 => self.dma.channels[0].count = val,
            0x0BA => {
                let was_off = !self.dma.channels[0].enabled();
                self.dma.channels[0].ctrl = val;
                if was_off && self.dma.channels[0].enabled() {
                    self.dma.channels[0].latch(0);
                }
            }
            // DMA1
            0x0BC => self.dma.channels[1].src = (self.dma.channels[1].src & !0xFFFF) | val as u32,
            0x0BE => {
                self.dma.channels[1].src =
                    (self.dma.channels[1].src & 0xFFFF) | ((val as u32 & 0x0FFF) << 16);
            }
            0x0C0 => self.dma.channels[1].dst = (self.dma.channels[1].dst & !0xFFFF) | val as u32,
            0x0C2 => {
                self.dma.channels[1].dst =
                    (self.dma.channels[1].dst & 0xFFFF) | ((val as u32 & 0x07FF) << 16);
            }
            0x0C4 => self.dma.channels[1].count = val,
            0x0C6 => {
                let was_off = !self.dma.channels[1].enabled();
                self.dma.channels[1].ctrl = val;
                if was_off && self.dma.channels[1].enabled() {
                    self.dma.channels[1].latch(1);
                }
            }
            // DMA2
            0x0C8 => self.dma.channels[2].src = (self.dma.channels[2].src & !0xFFFF) | val as u32,
            0x0CA => {
                self.dma.channels[2].src =
                    (self.dma.channels[2].src & 0xFFFF) | ((val as u32 & 0x0FFF) << 16);
            }
            0x0CC => self.dma.channels[2].dst = (self.dma.channels[2].dst & !0xFFFF) | val as u32,
            0x0CE => {
                self.dma.channels[2].dst =
                    (self.dma.channels[2].dst & 0xFFFF) | ((val as u32 & 0x07FF) << 16);
            }
            0x0D0 => self.dma.channels[2].count = val,
            0x0D2 => {
                let was_off = !self.dma.channels[2].enabled();
                self.dma.channels[2].ctrl = val;
                if was_off && self.dma.channels[2].enabled() {
                    self.dma.channels[2].latch(2);
                }
            }
            // DMA3
            0x0D4 => self.dma.channels[3].src = (self.dma.channels[3].src & !0xFFFF) | val as u32,
            0x0D6 => {
                self.dma.channels[3].src =
                    (self.dma.channels[3].src & 0xFFFF) | ((val as u32 & 0x0FFF) << 16);
            }
            0x0D8 => self.dma.channels[3].dst = (self.dma.channels[3].dst & !0xFFFF) | val as u32,
            0x0DA => {
                self.dma.channels[3].dst =
                    (self.dma.channels[3].dst & 0xFFFF) | ((val as u32 & 0x0FFF) << 16);
            }
            0x0DC => self.dma.channels[3].count = val,
            0x0DE => {
                let was_off = !self.dma.channels[3].enabled();
                self.dma.channels[3].ctrl = val;
                if was_off && self.dma.channels[3].enabled() {
                    self.dma.channels[3].latch(3);
                }
            }
            // Timers
            0x100 => self.timers.timers[0].reload = val,
            0x102 => {
                let was_off = !self.timers.timers[0].enabled();
                self.timers.timers[0].ctrl = val;
                if was_off && self.timers.timers[0].enabled() {
                    self.timers.timers[0].counter = self.timers.timers[0].reload;
                    self.timers.timers[0].cycles = 0;
                }
            }
            0x104 => self.timers.timers[1].reload = val,
            0x106 => {
                let was_off = !self.timers.timers[1].enabled();
                self.timers.timers[1].ctrl = val;
                if was_off && self.timers.timers[1].enabled() {
                    self.timers.timers[1].counter = self.timers.timers[1].reload;
                    self.timers.timers[1].cycles = 0;
                }
            }
            0x108 => self.timers.timers[2].reload = val,
            0x10A => {
                let was_off = !self.timers.timers[2].enabled();
                self.timers.timers[2].ctrl = val;
                if was_off && self.timers.timers[2].enabled() {
                    self.timers.timers[2].counter = self.timers.timers[2].reload;
                    self.timers.timers[2].cycles = 0;
                }
            }
            0x10C => self.timers.timers[3].reload = val,
            0x10E => {
                let was_off = !self.timers.timers[3].enabled();
                self.timers.timers[3].ctrl = val;
                if was_off && self.timers.timers[3].enabled() {
                    self.timers.timers[3].counter = self.timers.timers[3].reload;
                    self.timers.timers[3].cycles = 0;
                }
            }
            0x200 => self.ie = val,
            0x202 => self.irq_flags &= !val,
            0x208 => self.ime = val & 1,
            0x300 => self.postflg = val as u8,
            0x301 => self.halted = true,
            _ => {}
        }
    }
}
