#[cfg(test)]
mod tests {
    use crate::mmu::Mmu;

    // ------------------------------------------------------------------
    // WRAM
    // ------------------------------------------------------------------

    #[test]
    fn wram_bank0_read_write() {
        let mut mmu = Mmu::new();
        mmu.write(0xC000, 0xAB);
        assert_eq!(mmu.read(0xC000), 0xAB);
    }

    #[test]
    fn wram_bank1_default() {
        // Default wram_bank is 1, mapped at 0xD000
        let mut mmu = Mmu::new();
        mmu.write(0xD000, 0xCD);
        assert_eq!(mmu.read(0xD000), 0xCD);
    }

    // ------------------------------------------------------------------
    // Echo RAM
    // ------------------------------------------------------------------

    #[test]
    fn echo_ram_mirrors_wram0() {
        // Write to WRAM bank 0 at 0xC100; read back via Echo RAM at 0xE100
        let mut mmu = Mmu::new();
        mmu.write(0xC100, 0x42);
        assert_eq!(mmu.read(0xE100), 0x42);
    }

    #[test]
    fn echo_ram_mirrors_wram_bankn() {
        // Write to the switchable WRAM bank at 0xD100; read back at 0xF100
        let mut mmu = Mmu::new();
        mmu.write(0xD100, 0x7E);
        assert_eq!(mmu.read(0xF100), 0x7E);
    }

    // ------------------------------------------------------------------
    // SVBK (WRAM bank select) at 0xFF70
    // ------------------------------------------------------------------

    #[test]
    fn svbk_bank_0_maps_to_1() {
        // Writing 0x00 to SVBK must clamp to bank 1
        let mut mmu = Mmu::new();
        mmu.write(0xFF70, 0x00);
        // The register read-back should return 1 (the effective bank)
        assert_eq!(mmu.read(0xFF70), 1);
    }

    #[test]
    fn svbk_bank_switch() {
        let mut mmu = Mmu::new();
        // Write a sentinel to bank 3 before switching
        mmu.write(0xFF70, 0x03); // switch to bank 3
        mmu.write(0xD000, 0x99);
        assert_eq!(mmu.read(0xD000), 0x99);

        // Switch to bank 2 — data in bank 3 must not be visible
        mmu.write(0xFF70, 0x02);
        assert_ne!(mmu.read(0xD000), 0x99);

        // Switch back — data must still be there
        mmu.write(0xFF70, 0x03);
        assert_eq!(mmu.read(0xD000), 0x99);
    }

    // ------------------------------------------------------------------
    // HRAM
    // ------------------------------------------------------------------

    #[test]
    fn hram_read_write() {
        let mut mmu = Mmu::new();
        mmu.write(0xFF80, 0x55);
        assert_eq!(mmu.read(0xFF80), 0x55);
    }

    // ------------------------------------------------------------------
    // IE / IF registers
    // ------------------------------------------------------------------

    #[test]
    fn ie_register() {
        let mut mmu = Mmu::new();
        mmu.write(0xFFFF, 0x1F);
        assert_eq!(mmu.read(0xFFFF), 0x1F);
        assert_eq!(mmu.ie, 0x1F);
    }

    #[test]
    fn interrupt_flag_register() {
        let mut mmu = Mmu::new();
        mmu.write(0xFF0F, 0x05);
        assert_eq!(mmu.read(0xFF0F), 0x05);
        assert_eq!(mmu.interrupt_flag, 0x05);
    }

    // ------------------------------------------------------------------
    // Unusable region
    // ------------------------------------------------------------------

    #[test]
    fn unusable_region_reads_ff() {
        let mmu = Mmu::new();
        assert_eq!(mmu.read(0xFEA0), 0xFF);
        assert_eq!(mmu.read(0xFEFF), 0xFF);
    }

    // ------------------------------------------------------------------
    // Boot ROM
    // ------------------------------------------------------------------

    #[test]
    fn boot_rom_overlay() {
        let mut mmu = Mmu::new();
        let mut boot = vec![0u8; 256];
        boot[0x00] = 0xDE;
        boot[0x10] = 0xAD;
        mmu.set_boot_rom(boot);
        assert!(mmu.boot_rom_active);
        assert_eq!(mmu.read(0x0000), 0xDE);
        assert_eq!(mmu.read(0x0010), 0xAD);
    }

    #[test]
    fn boot_rom_disable() {
        let mut mmu = Mmu::new();
        mmu.set_boot_rom(vec![0xBE; 256]);
        assert!(mmu.boot_rom_active);
        // Writing any non-zero value to 0xFF50 disables the boot ROM
        mmu.write(0xFF50, 0x01);
        assert!(!mmu.boot_rom_active);
        // Now 0x0000 reads from the (empty) cartridge ROM → 0x00
        assert_eq!(mmu.read(0x0000), 0x00);
    }

    // ------------------------------------------------------------------
    // OAM DMA transfer
    // ------------------------------------------------------------------

    #[test]
    fn oam_dma_transfer() {
        let mut mmu = Mmu::new();
        // Write known data into WRAM bank 0 starting at 0xC000
        for i in 0u16..0xA0 {
            mmu.write(0xC000 + i, (i & 0xFF) as u8);
        }
        // Trigger DMA: source page 0xC0 → copies 0xC000-0xC09F into OAM
        mmu.write(0xFF46, 0xC0);
        // Verify OAM was populated
        for i in 0u16..0xA0 {
            assert_eq!(
                mmu.read(0xFE00 + i),
                (i & 0xFF) as u8,
                "OAM[{i:#04x}] mismatch after DMA"
            );
        }
    }

    // ------------------------------------------------------------------
    // Game Genie cheats
    // ------------------------------------------------------------------

    #[test]
    fn gg_cheat_no_compare() {
        // NoMbc empty ROM reads 0x00 at all addresses.
        // A cheat with no compare always replaces the value.
        let mut mmu = Mmu::new();
        mmu.add_gg_cheat(0x0100, 0xBB, None);
        assert_eq!(mmu.read(0x0100), 0xBB);
    }

    #[test]
    fn gg_cheat_with_compare_match() {
        // ROM byte is 0x00; compare=0x00 matches → return new_val
        let mut mmu = Mmu::new();
        mmu.add_gg_cheat(0x0200, 0xCC, Some(0x00));
        assert_eq!(mmu.read(0x0200), 0xCC);
    }

    #[test]
    fn gg_cheat_with_compare_no_match() {
        // ROM byte is 0x00; compare=0x55 does NOT match → return original 0x00
        let mut mmu = Mmu::new();
        mmu.add_gg_cheat(0x0300, 0xDD, Some(0x55));
        assert_eq!(mmu.read(0x0300), 0x00);
    }

    #[test]
    fn gg_cheat_cleared() {
        let mut mmu = Mmu::new();
        mmu.add_gg_cheat(0x0400, 0xEE, None);
        assert_eq!(mmu.read(0x0400), 0xEE);
        mmu.clear_cheats();
        // After clearing, the original ROM value (0x00) is returned
        assert_eq!(mmu.read(0x0400), 0x00);
    }

    // ------------------------------------------------------------------
    // Timer register routing
    // ------------------------------------------------------------------

    #[test]
    fn timer_register_routed() {
        // 0xFF04 is the DIV register. Writing any value resets it to 0.
        let mut mmu = Mmu::new();
        mmu.write(0xFF04, 0xFF); // reset DIV
        assert_eq!(mmu.read(0xFF04), 0x00);
    }

    // ------------------------------------------------------------------
    // Save / load round-trip
    // ------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip() {
        let mut mmu = Mmu::new();
        // Write to WRAM bank 0
        mmu.write(0xC000, 0x11);
        mmu.write(0xC001, 0x22);
        // Write to HRAM
        mmu.write(0xFF80, 0x33);
        mmu.write(0xFF81, 0x44);
        // Write IE and IF
        mmu.ie = 0x1F;
        mmu.interrupt_flag = 0x05;

        // Save state
        let mut state = Vec::new();
        mmu.save_state(&mut state);

        // Clobber the live MMU
        mmu.write(0xC000, 0x00);
        mmu.write(0xFF80, 0x00);
        mmu.ie = 0;
        mmu.interrupt_flag = 0;

        // Restore
        let mut cursor: &[u8] = &state;
        mmu.load_state(&mut cursor);

        assert_eq!(mmu.read(0xC000), 0x11);
        assert_eq!(mmu.read(0xC001), 0x22);
        assert_eq!(mmu.read(0xFF80), 0x33);
        assert_eq!(mmu.read(0xFF81), 0x44);
        assert_eq!(mmu.ie, 0x1F);
        assert_eq!(mmu.interrupt_flag, 0x05);
    }
}
