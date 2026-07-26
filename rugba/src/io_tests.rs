#[cfg(test)]
mod tests {
    use crate::io::IoRegisters;

    const IO_BASE: u32 = 0x0400_0000;

    // ---- dispcnt_read_write ----

    #[test]
    fn dispcnt_read_write() {
        let mut io = IoRegisters::new();
        io.write16(IO_BASE + 0x000, 0x1234);
        assert_eq!(io.read16(IO_BASE + 0x000), 0x1234);
    }

    // ---- dispstat_preserves_read_only ----

    #[test]
    fn dispstat_preserves_read_only() {
        let mut io = IoRegisters::new();
        // Bits 0-2 are read-only (set by PPU hardware): pre-set them directly
        io.dispstat = 0x07; // bits 0, 1, 2 set
        // Write a value that tries to clear bits 0-2
        io.write16(IO_BASE + 0x004, 0xFFF8);
        // Bits 0-2 must be preserved from the original value
        assert_eq!(io.read16(IO_BASE + 0x004) & 0x07, 0x07);
        // Bits 3-15 come from the written value
        assert_eq!(io.read16(IO_BASE + 0x004) & 0xFFF8, 0xFFF8);
    }

    // ---- vcount_not_writable ----

    #[test]
    fn vcount_not_writable() {
        let mut io = IoRegisters::new();
        // VCOUNT has no write arm — set it programmatically
        io.vcount = 42;
        // write16 to 0x006 hits the catch-all `_ => {}` and does nothing
        io.write16(IO_BASE + 0x006, 0xFFFF);
        assert_eq!(io.read16(IO_BASE + 0x006), 42);
    }

    // ---- bgcnt_all_four ----

    #[test]
    fn bgcnt_all_four() {
        let mut io = IoRegisters::new();
        let addrs = [0x008u32, 0x00A, 0x00C, 0x00E];
        let values = [0x1111u16, 0x2222, 0x3333, 0x4444];

        for (i, (&addr, &val)) in addrs.iter().zip(values.iter()).enumerate() {
            io.write16(IO_BASE + addr, val);
            assert_eq!(
                io.read16(IO_BASE + addr),
                val,
                "BG{}CNT mismatch",
                i
            );
        }
    }

    // ---- scroll_write_only ----

    #[test]
    fn scroll_write_only() {
        let mut io = IoRegisters::new();
        // Write to BG0HOFS (0x010) and BG0VOFS (0x012)
        io.write16(IO_BASE + 0x010, 0x01FF);
        io.write16(IO_BASE + 0x012, 0x01FF);
        // Read back must return 0 (scroll registers are write-only)
        assert_eq!(io.read16(IO_BASE + 0x010), 0);
        assert_eq!(io.read16(IO_BASE + 0x012), 0);
    }

    // ---- if_write_clears_bits ----

    #[test]
    fn if_write_clears_bits() {
        let mut io = IoRegisters::new();
        // Set some IRQ flags directly
        io.irq_flags = 0x0007;
        // Writing 1s to IF (0x202) clears those bits
        io.write16(IO_BASE + 0x202, 0x0003);
        assert_eq!(io.irq_flags, 0x0004);
    }

    // ---- ime_register ----

    #[test]
    fn ime_register() {
        let mut io = IoRegisters::new();
        // Only bit 0 is stored
        io.write16(IO_BASE + 0x208, 0xFFFF);
        assert_eq!(io.read16(IO_BASE + 0x208), 0x0001);

        io.write16(IO_BASE + 0x208, 0x0000);
        assert_eq!(io.read16(IO_BASE + 0x208), 0x0000);
    }

    // ---- fifo_a_assembles_dword ----

    #[test]
    fn fifo_a_assembles_dword() {
        let mut io = IoRegisters::new();
        // Two 16-bit writes to 0x0A0 (low halfword) and 0x0A2 (high halfword)
        io.write16(IO_BASE + 0x0A0, 0x1234);
        io.write16(IO_BASE + 0x0A2, 0x5678);
        // Pending u32 should be 0x5678_1234 (low word first)
        assert_eq!(io.fifo_a_pending, Some(0x5678_1234));
    }

    // ---- dma0_enable_latches ----

    #[test]
    fn dma0_enable_latches() {
        let mut io = IoRegisters::new();
        // Set up count so latch gives a known internal_count
        io.write16(IO_BASE + 0x0B8, 10); // DMA0 count = 10
        // Write ctrl with enable bit (0x8000), timing=immediate (bits 12:13=0)
        io.write16(IO_BASE + 0x0BA, 0x8000);
        // Channel should now be enabled
        assert!(io.dma.channels[0].enabled());
    }

    // ---- soundcnt_h_dirty_flag ----

    #[test]
    fn soundcnt_h_dirty_flag() {
        let mut io = IoRegisters::new();
        assert!(!io.soundcnt_h_dirty);
        io.write16(IO_BASE + 0x082, 0x0300);
        assert!(io.soundcnt_h_dirty);
        assert_eq!(io.soundcnt_h, 0x0300);
    }
}
