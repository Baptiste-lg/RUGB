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
            if ch == 3 {
                0x10000
            } else {
                0x4000
            }
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

            let (c, irq) = self.execute_channel(ch, ewram, iwram, vram, palette, oam, rom);
            cycles += c;
            if irq {
                irqs |= 1 << (8 + ch); // DMA IRQ flags are bits 8-11
            }
        }

        (cycles, irqs)
    }

    /// Execute a specific DMA channel transfer.
    #[allow(clippy::too_many_arguments)]
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
        let count = channel.internal_count.min(0x10000); // Cap to prevent runaway transfers

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
    write16_dma(
        addr + 2,
        (val >> 16) as u16,
        ewram,
        iwram,
        vram,
        palette,
        oam,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- memory helpers ----

    fn make_ewram() -> Vec<u8> {
        vec![0u8; 0x40000]
    }
    fn make_iwram() -> Vec<u8> {
        vec![0u8; 0x8000]
    }
    fn make_vram() -> Vec<u8> {
        vec![0u8; 0x18000]
    }
    fn make_palette() -> Vec<u8> {
        vec![0u8; 0x400]
    }
    fn make_oam() -> Vec<u8> {
        vec![0u8; 0x400]
    }
    fn make_rom() -> Vec<u8> {
        vec![0u8; 0x200_0000]
    }

    /// Create a channel, set public fields, latch internals, then run.
    fn setup_channel(
        dma: &mut DmaController,
        ch: usize,
        src: u32,
        dst: u32,
        count: u16,
        ctrl: u16,
    ) {
        dma.channels[ch].src = src;
        dma.channels[ch].dst = dst;
        dma.channels[ch].count = count;
        dma.channels[ch].ctrl = ctrl;
        dma.channels[ch].latch(ch);
    }

    // ---- channel_disabled_noop ----

    #[test]
    fn channel_disabled_noop() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // Source in EWRAM with data; dest in IWRAM; channel disabled (bit 15 = 0)
        ewram[0] = 0xAB;
        ewram[1] = 0xCD;
        setup_channel(
            &mut dma,
            0,
            0x0200_0000,
            0x0300_0000,
            1,
            0x0000, // enabled bit NOT set
        );
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);
        // IWRAM should be untouched
        assert_eq!(iwram[0], 0);
        assert_eq!(iwram[1], 0);
    }

    // ---- immediate_16bit_copy ----

    #[test]
    fn immediate_16bit_copy() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // Write pattern into EWRAM at address 0x02000000
        ewram[0] = 0x34;
        ewram[1] = 0x12;

        // ctrl: enabled(0x8000), timing=immediate(bits 12:13=0), 16-bit, no repeat, src/dst increment
        setup_channel(
            &mut dma,
            0,
            0x0200_0000,
            0x0300_0000,
            1,
            0x8000,
        );
        let (_, irqs) = dma.run_immediate(
            &mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom,
        );
        // Value should appear in IWRAM at offset 0x0000 (addr & 0x7FFE)
        assert_eq!(iwram[0], 0x34);
        assert_eq!(iwram[1], 0x12);
        // No IRQ requested (irq_enabled bit not set)
        assert_eq!(irqs & (1 << 8), 0);
    }

    // ---- immediate_32bit_copy ----

    #[test]
    fn immediate_32bit_copy() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        ewram[0] = 0x78;
        ewram[1] = 0x56;
        ewram[2] = 0x34;
        ewram[3] = 0x12;

        // ctrl: enabled | 32-bit (bit 10)
        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 1, 0x8400);
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        assert_eq!(iwram[0], 0x78);
        assert_eq!(iwram[1], 0x56);
        assert_eq!(iwram[2], 0x34);
        assert_eq!(iwram[3], 0x12);
    }

    // ---- src_increment ----

    #[test]
    fn src_increment() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // Source: 2 words at 0x02000000 and 0x02000002
        ewram[0] = 0x01;
        ewram[1] = 0x00;
        ewram[2] = 0x02;
        ewram[3] = 0x00;

        // ctrl: enabled, src increment (bits 7:8 = 0b00), dst increment (bits 5:6 = 0b00), 2 words
        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 2, 0x8000);
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        assert_eq!(u16::from_le_bytes([iwram[0], iwram[1]]), 0x0001);
        assert_eq!(u16::from_le_bytes([iwram[2], iwram[3]]), 0x0002);
    }

    // ---- src_decrement ----

    #[test]
    fn src_decrement() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // Place data at addresses 0x02000002 and 0x02000000
        ewram[2] = 0xAA;
        ewram[3] = 0x00;
        ewram[0] = 0xBB;
        ewram[1] = 0x00;

        // src starts at 0x02000002; src decrement = bits 7:8 = 0b01 → ctrl |= (1<<7)
        // Two transfers: reads 0x02000002 then 0x02000000
        setup_channel(&mut dma, 0, 0x0200_0002, 0x0300_0000, 2, 0x8000 | (1 << 7));
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        assert_eq!(iwram[0], 0xAA, "first read from 0x02000002");
        assert_eq!(iwram[2], 0xBB, "second read from 0x02000000");
    }

    // ---- src_fixed ----

    #[test]
    fn src_fixed() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        ewram[0] = 0x55;
        ewram[1] = 0x00;

        // src fixed = bits 7:8 = 0b10 → ctrl |= (2<<7)
        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 3, 0x8000 | (2 << 7));
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        // All 3 destinations should have 0x0055
        assert_eq!(iwram[0], 0x55);
        assert_eq!(iwram[2], 0x55);
        assert_eq!(iwram[4], 0x55);
    }

    // ---- dst_increment_reload ----

    #[test]
    fn dst_increment_reload() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        ewram[0] = 0x77;
        ewram[1] = 0x00;

        // dst mode 3 (reload) = bits 5:6 = 0b11 → ctrl |= (3<<5); repeat enabled (bit 9)
        // First run: dst increments during transfer, then reloads to original dst for next repeat.
        setup_channel(
            &mut dma,
            0,
            0x0200_0000,
            0x0300_0000,
            1,
            0x8000 | (3 << 5) | (1 << 9), // enabled | dst mode 3 | repeat
        );
        // Run once — repeat keeps channel enabled, dst reloads
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        // The transfer wrote to 0x03000000 (internal_dst=0)
        assert_eq!(iwram[0], 0x77);
        // Channel still enabled (repeat)
        assert!(dma.channels[0].enabled());
        // internal_dst should be reset to original dst (0x03000000)
        // We verify by running again and checking iwram[0] again (dst reloaded).
        // Note: internal_src was NOT reloaded, so the second run reads from ewram[2].
        ewram[2] = 0x88;
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);
        assert_eq!(iwram[0], 0x88, "dst should reload to original address on repeat");
    }

    // ---- zero_count_defaults_max ----

    #[test]
    fn zero_count_defaults_max() {
        // channel 0–2: count=0 → latch gives 0x4000
        let mut ch = DmaChannel::new();
        ch.count = 0;
        ch.latch(0);
        // internal_count is private, but we can infer via a transfer + check cycles
        // Instead, do a real run and verify 0x4000 words transferred
        // For speed, just verify the latch effect via a tiny DmaController run
        // We can observe the side-effect: if count=0x4000, running will touch
        // iwram[0..0x4000*2]. We just check latch via 1 word and count behavior.
        // The simplest check: construct the channel, latch it, then confirm
        // internal_count (exposed indirectly through how many bytes were written).
        // We'll do it through DmaController:
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        // Fill ewram from 0x02000000 with known bytes
        for i in 0..0x4000usize {
            ewram[i * 2] = (i & 0xFF) as u8;
            ewram[i * 2 + 1] = ((i >> 8) & 0xFF) as u8;
        }
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 0, 0x8000);
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        // Check last word (index 0x3FFF) was transferred
        let last = 0x3FFFusize;
        assert_eq!(iwram[last * 2], (last & 0xFF) as u8);
        assert_eq!(iwram[last * 2 + 1], ((last >> 8) & 0xFF) as u8);
    }

    // ---- irq_flag_raised ----

    #[test]
    fn irq_flag_raised() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // channel 2, irq_enabled = bit 14
        setup_channel(&mut dma, 2, 0x0200_0000, 0x0300_0000, 1, 0x8000 | 0x4000);
        let (_, irqs) = dma.run_immediate(
            &mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom,
        );
        // DMA channel 2 IRQ is bit 10 (8+2)
        assert!(irqs & (1 << 10) != 0, "IRQ bit 10 should be set for channel 2");
    }

    // ---- no_repeat_disables ----

    #[test]
    fn no_repeat_disables() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // No repeat bit → channel should be disabled after transfer
        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 1, 0x8000);
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);
        assert!(!dma.channels[0].enabled(), "channel should be disabled after single-shot");
    }

    // ---- latch_copies_internals ----

    #[test]
    fn latch_copies_internals() {
        let mut ch = DmaChannel::new();
        ch.src = 0x0200_1234;
        ch.dst = 0x0300_5678;
        ch.count = 42;
        ch.latch(0);

        // We can only observe internals indirectly, but we can verify by running a
        // minimal transfer that reads from the latched src and writes to latched dst.
        // Set up a real DmaController to prove it:
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        // Place data at EWRAM offset for 0x02001234
        let ewram_off = 0x1234usize; // addr & 0x3FFFE
        ewram[ewram_off] = 0xDE;
        ewram[ewram_off + 1] = 0xAD;

        dma.channels[0].src = 0x0200_1234;
        dma.channels[0].dst = 0x0300_5678;
        dma.channels[0].count = 1;
        dma.channels[0].ctrl = 0x8000;
        dma.channels[0].latch(0);

        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        // IWRAM at 0x03005678 → offset = 0x5678 & 0x7FFE = 0x5678
        let iwram_off = 0x5678usize;
        assert_eq!(iwram[iwram_off], 0xDE, "latch captured src correctly");
        assert_eq!(iwram[iwram_off + 1], 0xAD);
    }

    // ---- non_immediate_timing_skipped ----

    #[test]
    fn non_immediate_timing_skipped() {
        let mut dma = DmaController::new();
        let mut ewram = make_ewram();
        let mut iwram = make_iwram();
        let mut vram = make_vram();
        let mut palette = make_palette();
        let mut oam = make_oam();
        let rom = make_rom();

        ewram[0] = 0xFF;
        ewram[1] = 0xFF;

        // timing = 1 (bits 12:13 = 0b01) → run_immediate should skip
        setup_channel(&mut dma, 0, 0x0200_0000, 0x0300_0000, 1, 0x8000 | (1 << 12));
        dma.run_immediate(&mut ewram, &mut iwram, &mut vram, &mut palette, &mut oam, &rom);

        assert_eq!(iwram[0], 0, "non-immediate DMA should not transfer");
        assert_eq!(iwram[1], 0);
    }
}
