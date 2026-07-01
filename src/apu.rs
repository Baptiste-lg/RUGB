/// APU — Audio Processing Unit (register storage only for now).
///
/// Stores writes to 0xFF10-0xFF3F. No audio generation yet —
/// this is a stub so the MMU has somewhere to route audio register writes
/// instead of dropping them.

pub struct Apu {
    /// Raw register storage for NR10-NR52 and wave RAM (0xFF10-0xFF3F)
    regs: [u8; 0x30],
}

impl Apu {
    pub fn new() -> Self {
        Apu { regs: [0; 0x30] }
    }

    pub fn read(&self, addr: u16) -> u8 {
        let idx = (addr - 0xFF10) as usize;
        if idx < self.regs.len() {
            self.regs[idx]
        } else {
            0xFF
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        let idx = (addr - 0xFF10) as usize;
        if idx < self.regs.len() {
            self.regs[idx] = val;
        }
    }
}
