#[test]
fn test_gba_with_dma_timer_no_panic() {
    let rom = vec![0u8; 0x200];
    let mut emu = rugba::GbaEmulator::new(&rom);
    // Run a few frames — exercises DMA, timer, APU paths
    for _ in 0..5 {
        emu.run_frame();
    }
    assert_eq!(emu.framebuffer().len(), 240 * 160 * 4);
}

#[test]
fn test_gba_flash_detection_sram() {
    // ROM containing "SRAM_V" string triggers SRAM backup detection
    let mut rom = vec![0u8; 0x200];
    // Write "SRAM_V" at some offset
    let sram_str = b"SRAM_V";
    rom[0x100..0x106].copy_from_slice(sram_str);
    let emu = rugba::GbaEmulator::new(&rom);
    assert!(emu.has_battery());
}

#[test]
fn test_gba_flash_detection_flash() {
    let mut rom = vec![0u8; 0x200];
    let flash_str = b"FLASH_V";
    rom[0x100..0x107].copy_from_slice(flash_str);
    let emu = rugba::GbaEmulator::new(&rom);
    assert!(emu.has_battery());
}

#[test]
fn test_gba_no_backup_detection() {
    let rom = vec![0u8; 0x200];
    let emu = rugba::GbaEmulator::new(&rom);
    // No backup strings in ROM = no battery
    assert!(!emu.has_battery());
}
