use super::Cpu;
use crate::mmu::Mmu;

impl Cpu {
    /// Execute a CB-prefixed opcode. Returns T-cycle count.
    ///
    /// The 256 CB opcodes follow a regular pattern:
    /// - 0x00-0x07: RLC r    0x08-0x0F: RRC r
    /// - 0x10-0x17: RL r     0x18-0x1F: RR r
    /// - 0x20-0x27: SLA r    0x28-0x2F: SRA r
    /// - 0x30-0x37: SWAP r   0x38-0x3F: SRL r
    /// - 0x40-0x7F: BIT b, r
    /// - 0x80-0xBF: RES b, r
    /// - 0xC0-0xFF: SET b, r
    ///
    /// Register index (low 3 bits): B=0, C=1, D=2, E=3, H=4, L=5, (HL)=6, A=7
    pub(crate) fn execute_cb(&mut self, opcode: u8, mmu: &mut Mmu) -> u32 {
        let reg_idx = opcode & 0x07;

        // Read the operand value from the appropriate register or memory
        let val = self.cb_read_reg(reg_idx, mmu);

        let result = match opcode {
            // --- Rotate/Shift operations (0x00-0x3F) ---
            0x00..=0x07 => self.cb_rlc(val),
            0x08..=0x0F => self.cb_rrc(val),
            0x10..=0x17 => self.cb_rl(val),
            0x18..=0x1F => self.cb_rr(val),
            0x20..=0x27 => self.cb_sla(val),
            0x28..=0x2F => self.cb_sra(val),
            0x30..=0x37 => self.cb_swap(val),
            0x38..=0x3F => self.cb_srl(val),

            // --- BIT b, r (0x40-0x7F) — test bit, don't write back ---
            0x40..=0x7F => {
                let bit = (opcode >> 3) & 0x07;
                self.cb_bit(bit, val);
                // BIT never writes back, return early with correct timing
                return if reg_idx == 6 { 12 } else { 8 };
            }

            // --- RES b, r (0x80-0xBF) — clear bit ---
            0x80..=0xBF => {
                let bit = (opcode >> 3) & 0x07;
                val & !(1 << bit)
            }

            // --- SET b, r (0xC0-0xFF) — set bit ---
            0xC0..=0xFF => {
                let bit = (opcode >> 3) & 0x07;
                val | (1 << bit)
            }
        };

        // Write the result back to the register or memory
        self.cb_write_reg(reg_idx, result, mmu);

        // (HL) operations take 16 cycles, register ops take 8
        if reg_idx == 6 {
            16
        } else {
            8
        }
    }

    fn cb_read_reg(&self, idx: u8, mmu: &Mmu) -> u8 {
        match idx {
            0 => self.regs.b,
            1 => self.regs.c,
            2 => self.regs.d,
            3 => self.regs.e,
            4 => self.regs.h,
            5 => self.regs.l,
            6 => mmu.read(self.regs.hl()),
            7 => self.regs.a,
            _ => unreachable!(),
        }
    }

    fn cb_write_reg(&mut self, idx: u8, val: u8, mmu: &mut Mmu) {
        match idx {
            0 => self.regs.b = val,
            1 => self.regs.c = val,
            2 => self.regs.d = val,
            3 => self.regs.e = val,
            4 => self.regs.h = val,
            5 => self.regs.l = val,
            6 => mmu.write(self.regs.hl(), val),
            7 => self.regs.a = val,
            _ => unreachable!(),
        }
    }
}
