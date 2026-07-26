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

#[cfg(test)]
mod tests {
    use super::super::Cpu;
    use crate::mmu::Mmu;

    fn exec_cb_with<F>(cb_op: u8, setup: F) -> (Cpu, Mmu, u32)
    where
        F: FnOnce(&mut Cpu, &mut Mmu),
    {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        mmu.write(0xFF80, 0xCB);
        mmu.write(0xFF81, cb_op);
        cpu.regs.pc = 0xFF80;
        setup(&mut cpu, &mut mmu);
        let cycles = cpu.step(&mut mmu);
        (cpu, mmu, cycles)
    }

    // -------------------------------------------------------------------------
    // RLC — 0x00-0x07
    // -------------------------------------------------------------------------

    #[test]
    fn cb_rlc_b() {
        // CB 0x00 — RLC B. B=0x85 (1000_0101) → 0x0B (0000_1011). C=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x00, |cpu, _| {
            cpu.regs.b = 0x85;
        });
        assert_eq!(cpu.regs.b, 0x0B);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // RRC — 0x08-0x0F
    // -------------------------------------------------------------------------

    #[test]
    fn cb_rrc_c() {
        // CB 0x09 — RRC C. C=0x01 → 0x80. C=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x09, |cpu, _| {
            cpu.regs.c = 0x01;
        });
        assert_eq!(cpu.regs.c, 0x80);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // RL — 0x10-0x17
    // -------------------------------------------------------------------------

    #[test]
    fn cb_rl_d() {
        // CB 0x12 — RL D. D=0x80, carry=0 → result=0x00, carry=1, Z=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x12, |cpu, _| {
            cpu.regs.d = 0x80;
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.d, 0x00);
        assert!(cpu.regs.flag_c());
        assert!(cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // RR — 0x18-0x1F
    // -------------------------------------------------------------------------

    #[test]
    fn cb_rr_e() {
        // CB 0x1B — RR E. E=0x01, carry=0 → result=0x00, C=1, Z=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x1B, |cpu, _| {
            cpu.regs.e = 0x01;
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.e, 0x00);
        assert!(cpu.regs.flag_c());
        assert!(cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // SLA — 0x20-0x27
    // -------------------------------------------------------------------------

    #[test]
    fn cb_sla_h() {
        // CB 0x24 — SLA H. H=0x81 → 0x02, C=1 (MSB was 1). 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x24, |cpu, _| {
            cpu.regs.h = 0x81;
        });
        assert_eq!(cpu.regs.h, 0x02);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // SRA — 0x28-0x2F
    // -------------------------------------------------------------------------

    #[test]
    fn cb_sra_l() {
        // CB 0x2D — SRA L. L=0x81 (1000_0001) → 0xC0 (1100_0000). C=1 (old bit0).
        // Sign bit preserved: result bit7 = original bit7 = 1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x2D, |cpu, _| {
            cpu.regs.l = 0x81;
        });
        assert_eq!(cpu.regs.l, 0xC0);
        assert!(cpu.regs.flag_c()); // old bit 0 was 1
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // SWAP — 0x30-0x37
    // -------------------------------------------------------------------------

    #[test]
    fn cb_swap_a() {
        // CB 0x37 — SWAP A. A=0xAB → 0xBA. Z=0. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x37, |cpu, _| {
            cpu.regs.a = 0xAB;
        });
        assert_eq!(cpu.regs.a, 0xBA);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // SRL — 0x38-0x3F
    // -------------------------------------------------------------------------

    #[test]
    fn cb_srl_b() {
        // CB 0x38 — SRL B. B=0x81 → 0x40. C=1 (bit0 was 1). Bit7 becomes 0. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x38, |cpu, _| {
            cpu.regs.b = 0x81;
        });
        assert_eq!(cpu.regs.b, 0x40);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // BIT — 0x40-0x7F
    // -------------------------------------------------------------------------

    #[test]
    fn cb_bit_0_a() {
        // CB 0x47 — BIT 0, A. A has bit 0 set → Z=0. H=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x47, |cpu, _| {
            cpu.regs.a = 0x01; // bit 0 is set
        });
        assert!(!cpu.regs.flag_z()); // bit IS set → Z clear
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn cb_bit_1_a() {
        // CB 0x4F — BIT 1, A. A=0x01 → bit 1 not set → Z=1. H=1. 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x4F, |cpu, _| {
            cpu.regs.a = 0x01; // bit 1 is NOT set
        });
        assert!(cpu.regs.flag_z()); // bit NOT set → Z set
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // RES — 0x80-0xBF
    // -------------------------------------------------------------------------

    #[test]
    fn cb_res_b() {
        // CB 0x80 — RES 0, B. B=0xFF → 0xFE (bit 0 cleared). 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x80, |cpu, _| {
            cpu.regs.b = 0xFF;
        });
        assert_eq!(cpu.regs.b, 0xFE);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // SET — 0xC0-0xFF
    // -------------------------------------------------------------------------

    #[test]
    fn cb_set_b() {
        // CB 0xC0 — SET 0, B. B=0x00 → 0x01 (bit 0 set). 8 cycles.
        let (cpu, _, cycles) = exec_cb_with(0xC0, |cpu, _| {
            cpu.regs.b = 0x00;
        });
        assert_eq!(cpu.regs.b, 0x01);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // (HL) indirect — 16/12 cycles
    // -------------------------------------------------------------------------

    #[test]
    fn cb_hl_indirect_rlc() {
        // CB 0x06 — RLC (HL). HL points to HRAM value 0x80 → 0x01, C=1. 16 cycles.
        let (cpu, mmu, cycles) = exec_cb_with(0x06, |cpu, mmu| {
            cpu.regs.set_hl(0xFF90);
            mmu.write(0xFF90, 0x80);
        });
        assert_eq!(mmu.read(0xFF90), 0x01);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 16);
    }

    #[test]
    fn cb_hl_indirect_bit() {
        // CB 0x46 — BIT 0, (HL). Value=0x02 → bit 0 not set → Z=1. 12 cycles.
        let (cpu, _, cycles) = exec_cb_with(0x46, |cpu, mmu| {
            cpu.regs.set_hl(0xFF90);
            mmu.write(0xFF90, 0x02); // bit 0 is NOT set
        });
        assert!(cpu.regs.flag_z()); // bit not set → Z set
        assert!(cpu.regs.flag_h());
        assert_eq!(cycles, 12);
    }

    #[test]
    fn cb_hl_indirect_set() {
        // CB 0xC6 — SET 0, (HL). Value=0x00 → 0x01 written back. 16 cycles.
        let (_, mmu, cycles) = exec_cb_with(0xC6, |cpu, mmu| {
            cpu.regs.set_hl(0xFF90);
            mmu.write(0xFF90, 0x00);
        });
        assert_eq!(mmu.read(0xFF90), 0x01);
        assert_eq!(cycles, 16);
    }
}
