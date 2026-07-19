/// Integration test: create emulator from a minimal ROM and verify it doesn't panic.

#[test]
fn test_emulator_creation_minimal_rom() {
    // Minimal valid-ish ROM: 32KB of NOPs (0x00 = NOP on SM83)
    let rom = vec![0u8; 0x8000];
    // This should not panic
    let _emu = rugb::Emulator::new(&rom);
}

#[test]
fn test_emulator_run_frame_no_panic() {
    // ROM filled with HALT (0x76) to prevent infinite loops
    let mut rom = vec![0x76u8; 0x8000];
    // Need valid header area (at least cartridge type at 0x147)
    rom[0x147] = 0x00; // No MBC
    let mut emu = rugb::Emulator::new(&rom);
    // Should complete a frame without panicking
    emu.run_frame();
}

#[test]
fn test_save_load_state_roundtrip() {
    let mut rom = vec![0x76u8; 0x8000];
    rom[0x147] = 0x00;
    let mut emu = rugb::Emulator::new(&rom);
    emu.run_frame();

    let state = emu.save_state();
    assert!(!state.is_empty());

    // Load into a fresh emulator
    let mut emu2 = rugb::Emulator::new(&rom);
    emu2.load_state(&state);

    // Both should produce the same state after save
    let state2 = emu2.save_state();
    assert_eq!(state, state2);
}

#[test]
fn test_framebuffer_is_correct_size() {
    let rom = vec![0x76u8; 0x8000];
    let emu = rugb::Emulator::new(&rom);
    let fb = emu.framebuffer();
    assert_eq!(fb.len(), 160 * 144 * 4); // 160×144 RGBA
}
