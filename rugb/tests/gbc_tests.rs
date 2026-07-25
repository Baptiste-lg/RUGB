use rugb::Emulator;

fn make_cgb_rom(cart_type: u8) -> Vec<u8> {
    let mut rom = vec![0x76u8; 0x8000]; // HALT sled
    rom[0x143] = 0x80; // CGB compatible flag
    rom[0x147] = cart_type;
    rom[0x148] = 0x00; // 32KB ROM
    rom[0x149] = 0x02; // 8KB RAM
    rom
}

#[test]
fn test_cgb_detection() {
    let rom = make_cgb_rom(0x00);
    assert!(rugb::cartridge::is_cgb(&rom));

    // DMG-only ROM
    let mut dmg_rom = vec![0x76u8; 0x8000];
    dmg_rom[0x143] = 0x00;
    assert!(!rugb::cartridge::is_cgb(&dmg_rom));
}

#[test]
fn test_cgb_only_detection() {
    let mut rom = vec![0x76u8; 0x8000];
    rom[0x143] = 0xC0; // CGB only
    assert!(rugb::cartridge::is_cgb(&rom));
}

#[test]
fn test_cgb_emulator_creation() {
    let rom = make_cgb_rom(0x00);
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_cgb_run_frame() {
    let rom = make_cgb_rom(0x00);
    let mut emu = Emulator::new(&rom);
    emu.run_frame();
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_cgb_multiple_frames() {
    let rom = make_cgb_rom(0x01); // MBC1 + CGB
    let mut emu = Emulator::new(&rom);
    for _ in 0..30 {
        emu.run_frame();
    }
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_cgb_save_state_roundtrip() {
    let rom = make_cgb_rom(0x00);
    let mut emu = Emulator::new(&rom);
    emu.run_frame();

    let state = emu.save_state();
    assert!(!state.is_empty());

    let mut emu2 = Emulator::new(&rom);
    emu2.load_state(&state);
    let state2 = emu2.save_state();
    assert_eq!(state, state2);
}

#[test]
fn test_dmg_rom_still_works() {
    // Ensure DMG ROMs are not broken by GBC code
    let mut rom = vec![0x76u8; 0x8000];
    rom[0x143] = 0x00; // DMG only
    rom[0x147] = 0x00;
    let mut emu = Emulator::new(&rom);
    emu.run_frame();
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}
