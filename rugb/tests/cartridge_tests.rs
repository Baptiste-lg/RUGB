use rugb::Emulator;

fn make_rom(cart_type: u8, ram_size: u8) -> Vec<u8> {
    let mut rom = vec![0x76u8; 0x8000]; // HALT sled
    rom[0x147] = cart_type;
    rom[0x148] = 0x00; // ROM size: 32KB
    rom[0x149] = ram_size;
    rom
}

#[test]
fn test_no_mbc_creation() {
    let rom = make_rom(0x00, 0x00);
    let emu = Emulator::new(&rom);
    emu.run_frame();
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc1_creation() {
    let rom = make_rom(0x01, 0x02); // MBC1, 8KB RAM
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc1_battery_creation() {
    let rom = make_rom(0x03, 0x03); // MBC1+RAM+Battery, 32KB RAM
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc2_creation() {
    let rom = make_rom(0x05, 0x00); // MBC2
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc3_creation() {
    let rom = make_rom(0x13, 0x03); // MBC3+RAM+Battery
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc5_creation() {
    let rom = make_rom(0x19, 0x02); // MBC5
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_mbc5_rumble_creation() {
    let rom = make_rom(0x1C, 0x02); // MBC5+Rumble
    let emu = Emulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_unsupported_cart_type_falls_back() {
    let rom = make_rom(0xFE, 0x00); // Unsupported type
    let emu = Emulator::new(&rom);
    // Should fall back to NoMBC without panicking
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_save_state_across_cart_types() {
    for cart_type in &[0x00, 0x01, 0x05, 0x13, 0x19] {
        let rom = make_rom(*cart_type, 0x02);
        let mut emu = Emulator::new(&rom);
        emu.run_frame();
        let state = emu.save_state();

        let mut emu2 = Emulator::new(&rom);
        emu2.load_state(&state);

        let state2 = emu2.save_state();
        assert_eq!(
            state, state2,
            "Save state mismatch for cart type 0x{:02X}",
            cart_type
        );
    }
}
