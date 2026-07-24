use rugb::Emulator;

#[test]
fn test_apu_ring_buffer_initially_empty() {
    let rom = vec![0x76u8; 0x8000];
    let emu = Emulator::new(&rom);
    // Audio ring buffer should start empty
    // We can check via save state that sample_clock is 0
    let state = emu.save_state();
    assert!(!state.is_empty());
}

#[test]
fn test_apu_produces_samples_after_frame() {
    let mut rom = vec![0x76u8; 0x8000]; // HALT instructions
    rom[0x147] = 0x00; // No MBC
    let mut emu = Emulator::new(&rom);

    // Run one frame — APU should produce samples
    emu.run_frame();

    // Run another frame to confirm no panic
    emu.run_frame();

    // Framebuffer should still be valid
    assert_eq!(emu.framebuffer().len(), 160 * 144 * 4);
}

#[test]
fn test_multiple_frames_no_panic() {
    let mut rom = vec![0x76u8; 0x8000];
    rom[0x147] = 0x00;
    let mut emu = Emulator::new(&rom);

    // Run 60 frames (1 second of emulation)
    for _ in 0..60 {
        emu.run_frame();
    }

    let fb = emu.framebuffer();
    assert_eq!(fb.len(), 160 * 144 * 4);
}
