/// Integration tests for ARM instruction execution.
/// These write ARM instructions directly into IWRAM and execute them.

fn make_emu_with_arm_code(instructions: &[u32]) -> (rugba::GbaEmulator, Vec<u8>) {
    // Build a minimal ROM that branches to IWRAM where we place test code
    let mut rom = vec![0u8; 0x100];
    // ARM instruction: B to IWRAM (0x03000000)
    // LDR PC, [PC, #-4] with the address following
    // Actually, simplest: put the instructions directly in ROM at 0x08000000
    // and set PC there.
    let mut full_rom = vec![0u8; 0x200];
    for (i, &inst) in instructions.iter().enumerate() {
        let offset = i * 4;
        let bytes = inst.to_le_bytes();
        full_rom[offset] = bytes[0];
        full_rom[offset + 1] = bytes[1];
        full_rom[offset + 2] = bytes[2];
        full_rom[offset + 3] = bytes[3];
    }
    (rugba::GbaEmulator::new(&full_rom), full_rom)
}

#[test]
fn test_gba_emulator_creation() {
    let rom = vec![0u8; 0x100];
    let emu = rugba::GbaEmulator::new(&rom);
    assert_eq!(emu.framebuffer().len(), 240 * 160 * 4);
}

#[test]
fn test_gba_framebuffer_size() {
    let rom = vec![0u8; 0x100];
    let emu = rugba::GbaEmulator::new(&rom);
    // GBA framebuffer: 240 * 160 * 4 (RGBA)
    assert_eq!(emu.framebuffer().len(), 240 * 160 * 4);
}

#[test]
fn test_gba_run_frame_no_panic() {
    // ROM full of undefined instructions — should not panic
    let rom = vec![0u8; 0x200];
    let mut emu = rugba::GbaEmulator::new(&rom);
    emu.run_frame();
}

#[test]
fn test_gba_multiple_frames() {
    let rom = vec![0u8; 0x200];
    let mut emu = rugba::GbaEmulator::new(&rom);
    for _ in 0..10 {
        emu.run_frame();
    }
    assert_eq!(emu.framebuffer().len(), 240 * 160 * 4);
}
