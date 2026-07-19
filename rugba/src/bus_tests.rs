#[cfg(test)]
mod tests {
    use crate::bus::Bus;

    fn make_bus() -> Bus {
        let rom = vec![0xAA; 0x200]; // Small test ROM
        Bus::new(rom)
    }

    #[test]
    fn test_ewram_read_write() {
        let mut bus = make_bus();
        bus.write8(0x0200_0000, 0x42);
        assert_eq!(bus.read8(0x0200_0000), 0x42);

        bus.write16(0x0200_0010, 0xBEEF);
        assert_eq!(bus.read16(0x0200_0010), 0xBEEF);

        bus.write32(0x0200_0020, 0xDEAD_BEEF);
        assert_eq!(bus.read32(0x0200_0020), 0xDEAD_BEEF);
    }

    #[test]
    fn test_ewram_mirroring() {
        let mut bus = make_bus();
        bus.write8(0x0200_0000, 0x55);
        // EWRAM mirrors every 256KB
        assert_eq!(bus.read8(0x0204_0000), 0x55);
    }

    #[test]
    fn test_iwram_read_write() {
        let mut bus = make_bus();
        bus.write32(0x0300_0000, 0x1234_5678);
        assert_eq!(bus.read32(0x0300_0000), 0x1234_5678);
    }

    #[test]
    fn test_iwram_mirroring() {
        let mut bus = make_bus();
        bus.write8(0x0300_0000, 0xAA);
        // IWRAM mirrors every 32KB
        assert_eq!(bus.read8(0x0300_8000), 0xAA);
    }

    #[test]
    fn test_palette_read_write() {
        let mut bus = make_bus();
        bus.write16(0x0500_0000, 0x7FFF); // White in RGB555
        assert_eq!(bus.read16(0x0500_0000), 0x7FFF);
    }

    #[test]
    fn test_palette_8bit_write_duplicates() {
        let mut bus = make_bus();
        bus.write8(0x0500_0000, 0x42);
        // 8-bit palette writes duplicate to both bytes
        assert_eq!(bus.read16(0x0500_0000), 0x4242);
    }

    #[test]
    fn test_vram_read_write() {
        let mut bus = make_bus();
        bus.write16(0x0600_0000, 0x1234);
        assert_eq!(bus.read16(0x0600_0000), 0x1234);
    }

    #[test]
    fn test_vram_mirroring() {
        let mut bus = make_bus();
        bus.write16(0x0600_0000, 0xAAAA);
        // VRAM is 96KB, mirrors at 0x18000 (upper 32KB mirrors lower 32KB)
        assert_eq!(bus.read16(0x0601_8000), 0xAAAA);
    }

    #[test]
    fn test_rom_read() {
        let mut rom = vec![0; 0x200];
        rom[0] = 0x12;
        rom[1] = 0x34;
        rom[2] = 0x56;
        rom[3] = 0x78;
        let bus = Bus::new(rom);

        assert_eq!(bus.read8(0x0800_0000), 0x12);
        assert_eq!(bus.read16(0x0800_0000), 0x3412);
        assert_eq!(bus.read32(0x0800_0000), 0x78563412);
    }

    #[test]
    fn test_rom_out_of_bounds() {
        let rom = vec![0xAA; 0x100];
        let bus = Bus::new(rom);
        // Reading past ROM returns 0
        assert_eq!(bus.read8(0x0800_0200), 0);
    }

    #[test]
    fn test_sram_read_write() {
        let mut bus = make_bus();
        bus.write8(0x0E00_0000, 0x99);
        assert_eq!(bus.read8(0x0E00_0000), 0x99);
    }

    #[test]
    fn test_oam_read_write() {
        let mut bus = make_bus();
        bus.write16(0x0700_0000, 0xABCD);
        assert_eq!(bus.read16(0x0700_0000), 0xABCD);
    }

    #[test]
    fn test_io_keyinput() {
        let mut bus = make_bus();
        // All buttons released = 0x03FF
        assert_eq!(bus.read16(0x0400_0130), 0x03FF);

        // Press A (bit 0)
        bus.keypad.set_button(0, true);
        assert_eq!(bus.read16(0x0400_0130), 0x03FE); // Bit 0 cleared (active-low)
    }

    #[test]
    fn test_alignment_enforcement() {
        let mut bus = make_bus();
        bus.write32(0x0200_0000, 0xDEAD_BEEF);
        // Misaligned read32 should force-align to word boundary
        assert_eq!(bus.read32(0x0200_0001), 0xDEAD_BEEF); // Aligned to 0x0200_0000
        assert_eq!(bus.read32(0x0200_0002), 0xDEAD_BEEF);
        assert_eq!(bus.read32(0x0200_0003), 0xDEAD_BEEF);
    }
}
