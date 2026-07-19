#[cfg(test)]
mod tests {
    use crate::io::IoRegisters;
    use crate::ppu::Ppu;

    #[test]
    fn test_ppu_initial_state() {
        let ppu = Ppu::new();
        // Framebuffer should be all zeros (black)
        assert!(ppu.framebuffer.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_vcount_increments() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // One full scanline = 1232 cycles
        ppu.tick(1232, &mut io, &vram, &palette);
        assert_eq!(io.vcount, 1);

        // Two more scanlines
        ppu.tick(1232 * 2, &mut io, &vram, &palette);
        assert_eq!(io.vcount, 3);
    }

    #[test]
    fn test_vblank_flag_set_at_line_160() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // Advance to line 160 (V-blank start)
        ppu.tick(1232 * 160, &mut io, &vram, &palette);
        assert_eq!(io.vcount, 160);
        assert!(io.dispstat & 0x01 != 0); // V-blank flag set
    }

    #[test]
    fn test_vblank_irq_fires_when_enabled() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        io.dispstat = 0x08; // Enable V-blank IRQ
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // Advance to V-blank
        let mut total_irqs = 0u16;
        for _ in 0..160 {
            total_irqs |= ppu.tick(1232, &mut io, &vram, &palette);
        }
        assert!(total_irqs & 0x01 != 0); // V-blank IRQ raised
    }

    #[test]
    fn test_hblank_flag_set_during_hblank() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // Advance 960 cycles (past draw period into H-blank)
        ppu.tick(960, &mut io, &vram, &palette);
        assert!(io.dispstat & 0x02 != 0); // H-blank flag set
    }

    #[test]
    fn test_full_frame_wraps_to_zero() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // Full frame: 228 scanlines × 1232 cycles = 280896
        ppu.tick(1232 * 228, &mut io, &vram, &palette);
        assert_eq!(io.vcount, 0); // Wrapped back
        assert!(io.dispstat & 0x01 == 0); // V-blank cleared
    }

    #[test]
    fn test_mode3_renders_pixels() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        io.dispcnt = 3; // Mode 3
        let mut vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        // Write a red pixel (RGB555: R=31, G=0, B=0 = 0x001F) at (0,0)
        vram[0] = 0x1F;
        vram[1] = 0x00;

        // Tick one full scanline (enters H-blank, which triggers render of line 0)
        ppu.tick(1232, &mut io, &vram, &palette);

        // Check framebuffer pixel 0: should be R=248, G=0, B=0, A=255
        assert_eq!(ppu.framebuffer[0], 0xF8); // R (31 << 3)
        assert_eq!(ppu.framebuffer[1], 0x00); // G
        assert_eq!(ppu.framebuffer[2], 0x00); // B
        assert_eq!(ppu.framebuffer[3], 0xFF); // A
    }

    #[test]
    fn test_mode4_renders_indexed_pixel() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        io.dispcnt = 4; // Mode 4
        let mut vram = vec![0u8; 0x18000];
        let mut palette = vec![0u8; 0x400];

        // Set palette entry 1 to green (RGB555: 0x03E0)
        palette[2] = 0xE0;
        palette[3] = 0x03;

        // Set pixel (0,0) to palette index 1
        vram[0] = 1;

        ppu.tick(1232, &mut io, &vram, &palette);

        assert_eq!(ppu.framebuffer[0], 0x00); // R
        assert_eq!(ppu.framebuffer[1], 0xF8); // G (31 << 3)
        assert_eq!(ppu.framebuffer[2], 0x00); // B
        assert_eq!(ppu.framebuffer[3], 0xFF); // A
    }

    #[test]
    fn test_vcount_match_irq() {
        let mut ppu = Ppu::new();
        let mut io = IoRegisters::new();
        // Set V-count target to line 5, enable V-count IRQ
        io.dispstat = (5 << 8) | 0x20;
        let vram = vec![0u8; 0x18000];
        let palette = vec![0u8; 0x400];

        let mut total_irqs = 0u16;
        for _ in 0..6 {
            total_irqs |= ppu.tick(1232, &mut io, &vram, &palette);
        }
        assert!(total_irqs & 0x04 != 0); // V-count match IRQ
    }
}
