/// GBA DMA controller — 4 channels with priority DMA0 > DMA1 > DMA2 > DMA3.
///
/// Each channel has: source addr, dest addr, word count, control register.
/// DMA1/DMA2 can be triggered by sound FIFO requests.

#[derive(Clone, Copy)]
pub struct DmaChannel {
    /// Source address (27/28 bits)
    pub src: u32,
    /// Destination address (27/28 bits)
    pub dst: u32,
    /// Word count (14 bits for DMA0-2, 16 bits for DMA3)
    pub count: u16,
    /// Control register
    pub ctrl: u16,
    /// Internal latched source
    internal_src: u32,
    /// Internal latched destination
    internal_dst: u32,
    /// Internal latched count
    internal_count: u32,
}

impl DmaChannel {
    pub fn new() -> Self {
        DmaChannel {
            src: 0,
            dst: 0,
            count: 0,
            ctrl: 0,
            internal_src: 0,
            internal_dst: 0,
            internal_count: 0,
        }
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.ctrl & 0x8000 != 0
    }

    #[inline]
    pub fn timing(&self) -> u8 {
        ((self.ctrl >> 12) & 3) as u8
    }

    #[inline]
    pub fn irq_enabled(&self) -> bool {
        self.ctrl & 0x4000 != 0
    }

    #[inline]
    pub fn is_32bit(&self) -> bool {
        self.ctrl & 0x0400 != 0
    }

    #[inline]
    pub fn repeat(&self) -> bool {
        self.ctrl & 0x0200 != 0
    }

    #[inline]
    fn src_adjust(&self) -> i32 {
        match (self.ctrl >> 7) & 3 {
            0 => 1,  // Increment
            1 => -1, // Decrement
            2 => 0,  // Fixed
            _ => 1,  // Prohibited (treat as increment)
        }
    }

    #[inline]
    fn dst_adjust(&self) -> i32 {
        match (self.ctrl >> 5) & 3 {
            0 => 1,  // Increment
            1 => -1, // Decrement
            2 => 0,  // Fixed
            3 => 1,  // Increment/Reload (reloads dst on repeat)
            _ => 1,
        }
    }

    /// Latch source/dest/count when DMA is first enabled.
    pub fn latch(&mut self, ch: usize) {
        self.internal_src = self.src;
        self.internal_dst = self.dst;
        self.internal_count = if self.count == 0 {
            if ch == 3 { 0x10000 } else { 0x4000 }
        } else {
            self.count as u32
        };
    }
}

pub struct DmaController {
    pub channels: [DmaChannel; 4],
}

impl DmaController {
    pub fn new() -> Self {
        DmaController {
            channels: [DmaChannel::new(); 4],
        }
    }

    /// Execute any pending immediate DMA transfers.
    /// Returns cycles consumed and IRQ flags to raise.
    pub fn run_immediate(
        &mut self,
        ewram: &mut [u8],
        iwram: &mut [u8],
        vram: &mut [u8],
        palette: &mut [u8],
        oam: &mut [u8],
        rom: &[u8],
    ) -> (u32, u16) {
        let mut cycles = 0u32;
        let mut irqs = 0u16;

        for ch in 0..4 {
            if !self.channels[ch].enabled() || self.channels[ch].timing() != 0 {
                continue; // Not immediate or not enabled
            }

            let (c, irq) =
                self.execute_channel(ch, ewram, iwram, vram, palette, oam, rom);
            cycles += c;
            if irq {
                irqs |= 1 << (8 + ch); // DMA IRQ flags are bits 8-11
            }
        }

        (cycles, irqs)
    }

    /// Execute a specific DMA channel transfer.
    fn execute_channel(
        &mut self,
        ch: usize,
        ewram: &mut [u8],
        iwram: &mut [u8],
        vram: &mut [u8],
        palette: &mut [u8],
        oam: &mut [u8],
        rom: &[u8],
    ) -> (u32, bool) {
        let channel = &mut self.channels[ch];
        let word32 = channel.is_32bit();
        let step = if word32 { 4u32 } else { 2u32 };
        let src_adj = channel.src_adjust() * step as i32;
        let dst_adj = channel.dst_adjust() * step as i32;
        let count = channel.internal_count;

        let mut src = channel.internal_src;
        let mut dst = channel.internal_dst;

        for _ in 0..count {
            if word32 {
                let val = read32_dma(src, ewram, iwram, vram, rom);
                write32_dma(dst, val, ewram, iwram, vram, palette, oam);
            } else {
                let val = read16_dma(src, ewram, iwram, vram, rom);
                write16_dma(dst, val, ewram, iwram, vram, palette, oam);
            }
            src = (src as i32).wrapping_add(src_adj) as u32;
            dst = (dst as i32).wrapping_add(dst_adj) as u32;
        }

        channel.internal_src = src;
        channel.internal_dst = dst;

        let irq = channel.irq_enabled();

        if !channel.repeat() {
            channel.ctrl &= !0x8000; // Disable after transfer
        } else if (channel.ctrl >> 5) & 3 == 3 {
            // Dst reload on repeat
            channel.internal_dst = channel.dst;
        }

        (count * if word32 { 2 } else { 1 }, irq)
    }
}

// Simplified DMA memory access (bypasses full bus routing)
#[inline]
fn read16_dma(addr: u32, ewram: &[u8], iwram: &[u8], vram: &[u8], rom: &[u8]) -> u16 {
    match addr >> 24 {
        0x02 => {
            let a = (addr & 0x3FFFE) as usize;
            u16::from_le_bytes([ewram[a], ewram[a + 1]])
        }
        0x03 => {
            let a = (addr & 0x7FFE) as usize;
            u16::from_le_bytes([iwram[a], iwram[a + 1]])
        }
        0x06 => {
            let a = (addr & 0x1FFFE) as usize;
            let a = if a >= 0x18000 { a - 0x8000 } else { a };
            u16::from_le_bytes([vram[a], vram[a + 1]])
        }
        0x08..=0x0D => {
            let a = (addr & 0x01FF_FFFE) as usize;
            let lo = *rom.get(a).unwrap_or(&0);
            let hi = *rom.get(a + 1).unwrap_or(&0);
            u16::from_le_bytes([lo, hi])
        }
        _ => 0,
    }
}

#[inline]
fn read32_dma(addr: u32, ewram: &[u8], iwram: &[u8], vram: &[u8], rom: &[u8]) -> u32 {
    let lo = read16_dma(addr, ewram, iwram, vram, rom) as u32;
    let hi = read16_dma(addr + 2, ewram, iwram, vram, rom) as u32;
    lo | (hi << 16)
}

#[inline]
fn write16_dma(
    addr: u32,
    val: u16,
    ewram: &mut [u8],
    iwram: &mut [u8],
    vram: &mut [u8],
    palette: &mut [u8],
    oam: &mut [u8],
) {
    let bytes = val.to_le_bytes();
    match addr >> 24 {
        0x02 => {
            let a = (addr & 0x3FFFE) as usize;
            ewram[a] = bytes[0];
            ewram[a + 1] = bytes[1];
        }
        0x03 => {
            let a = (addr & 0x7FFE) as usize;
            iwram[a] = bytes[0];
            iwram[a + 1] = bytes[1];
        }
        0x05 => {
            let a = (addr & 0x3FE) as usize;
            palette[a] = bytes[0];
            palette[a + 1] = bytes[1];
        }
        0x06 => {
            let mut a = (addr & 0x1FFFE) as usize;
            if a >= 0x18000 {
                a -= 0x8000;
            }
            vram[a] = bytes[0];
            vram[a + 1] = bytes[1];
        }
        0x07 => {
            let a = (addr & 0x3FE) as usize;
            oam[a] = bytes[0];
            oam[a + 1] = bytes[1];
        }
        _ => {}
    }
}

#[inline]
fn write32_dma(
    addr: u32,
    val: u32,
    ewram: &mut [u8],
    iwram: &mut [u8],
    vram: &mut [u8],
    palette: &mut [u8],
    oam: &mut [u8],
) {
    write16_dma(addr, val as u16, ewram, iwram, vram, palette, oam);
    write16_dma(addr + 2, (val >> 16) as u16, ewram, iwram, vram, palette, oam);
}
