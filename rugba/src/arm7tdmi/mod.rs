pub mod arm;
pub mod registers;
pub mod thumb;

use crate::bus::Bus;
pub use registers::*;

/// ARM7TDMI CPU core.
pub struct Arm7Tdmi {
    /// General-purpose registers R0-R15 (current mode view)
    pub regs: [u32; 16],
    /// Current Program Status Register
    pub cpsr: u32,
    /// Banked registers for mode switching
    pub banked: BankedRegisters,
}

impl Arm7Tdmi {
    pub fn new() -> Self {
        let mut cpu = Arm7Tdmi {
            regs: [0; 16],
            cpsr: CpuMode::System as u32 | I_FLAG | F_FLAG,
            banked: BankedRegisters::new(),
        };
        // Initial state after reset: ARM mode, IRQ/FIQ disabled, System mode
        cpu.regs[13] = 0x03007F00; // SP default (IWRAM top)
        cpu.regs[15] = 0x08000000; // PC at ROM start (HLE skips BIOS)
        cpu
    }

    /// Execute one instruction. Returns cycles consumed.
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if bus.io.halted {
            // CPU halted — just consume 1 cycle, check for IRQ
            if bus.io.irq_flags & bus.io.ie != 0 {
                bus.io.halted = false;
            }
            return 1;
        }

        if self.in_thumb_mode() {
            let pc = self.regs[15] & !1;
            let instruction = bus.read16(pc);
            self.regs[15] = pc.wrapping_add(2);
            thumb::execute_thumb(self, bus, instruction)
        } else {
            let pc = self.regs[15] & !3;
            let instruction = bus.read32(pc);
            self.regs[15] = pc.wrapping_add(4);
            arm::execute_arm(self, bus, instruction)
        }
    }

    /// Check if CPU is in THUMB mode (T bit set in CPSR).
    #[inline]
    pub fn in_thumb_mode(&self) -> bool {
        self.cpsr & T_FLAG != 0
    }

    /// Get current operating mode.
    pub fn mode(&self) -> CpuMode {
        CpuMode::from_bits(self.cpsr)
    }

    /// Get the SPSR for the current mode (System/User have no SPSR — returns CPSR).
    pub fn spsr(&self) -> u32 {
        let idx = self.mode().bank_index();
        if idx == 0 {
            self.cpsr
        } else {
            self.banked.spsr[idx]
        }
    }

    /// Set the SPSR for the current mode.
    pub fn set_spsr(&mut self, val: u32) {
        let idx = self.mode().bank_index();
        if idx != 0 {
            self.banked.spsr[idx] = val;
        }
    }

    /// Evaluate a 4-bit condition code against current flags.
    pub fn check_condition(&self, cond: u32) -> bool {
        let n = self.cpsr & N_FLAG != 0;
        let z = self.cpsr & Z_FLAG != 0;
        let c = self.cpsr & C_FLAG != 0;
        let v = self.cpsr & V_FLAG != 0;

        match cond {
            0x0 => z,             // EQ
            0x1 => !z,            // NE
            0x2 => c,             // CS/HS
            0x3 => !c,            // CC/LO
            0x4 => n,             // MI
            0x5 => !n,            // PL
            0x6 => v,             // VS
            0x7 => !v,            // VC
            0x8 => c && !z,       // HI
            0x9 => !c || z,       // LS
            0xA => n == v,        // GE
            0xB => n != v,        // LT
            0xC => !z && n == v,  // GT
            0xD => z || n != v,   // LE
            0xE => true,          // AL (always)
            _ => true,            // NV (treat as always on ARM7TDMI)
        }
    }

    /// Get a flag bit from CPSR.
    #[inline]
    pub fn get_flag(&self, flag: u32) -> bool {
        self.cpsr & flag != 0
    }

    /// Set a flag bit in CPSR.
    #[inline]
    pub fn set_flag(&mut self, flag: u32, val: bool) {
        if val {
            self.cpsr |= flag;
        } else {
            self.cpsr &= !flag;
        }
    }

    /// Set N and Z flags based on a 32-bit result.
    #[inline]
    pub fn set_nz(&mut self, result: u32) {
        self.set_flag(N_FLAG, result & 0x8000_0000 != 0);
        self.set_flag(Z_FLAG, result == 0);
    }

    /// Switch to a new CPU mode, banking registers as needed.
    pub fn switch_mode(&mut self, new_mode: CpuMode) {
        let old_mode = self.mode();
        if old_mode == new_mode {
            return;
        }

        let old_idx = old_mode.bank_index();
        let new_idx = new_mode.bank_index();

        // Save current R13/R14 to old bank
        self.banked.r13[old_idx] = self.regs[13];
        self.banked.r14[old_idx] = self.regs[14];

        // Handle FIQ R8-R12 banking
        if old_mode == CpuMode::Fiq {
            for i in 0..5 {
                self.banked.fiq_r8_r12[i] = self.regs[8 + i];
            }
            for i in 0..5 {
                self.regs[8 + i] = self.banked.usr_r8_r12[i];
            }
        } else if new_mode == CpuMode::Fiq {
            for i in 0..5 {
                self.banked.usr_r8_r12[i] = self.regs[8 + i];
            }
            for i in 0..5 {
                self.regs[8 + i] = self.banked.fiq_r8_r12[i];
            }
        }

        // Restore R13/R14 from new bank
        self.regs[13] = self.banked.r13[new_idx];
        self.regs[14] = self.banked.r14[new_idx];

        // Update mode bits in CPSR
        self.cpsr = (self.cpsr & !0x1F) | new_mode as u32;
    }

    /// Enter an exception (IRQ, SWI, etc.).
    pub fn enter_exception(&mut self, mode: CpuMode, vector: u32) {
        let old_cpsr = self.cpsr;
        let return_addr = self.regs[15]; // Already advanced past current instruction

        self.switch_mode(mode);
        self.banked.spsr[mode.bank_index()] = old_cpsr;
        self.regs[14] = return_addr;
        self.cpsr |= I_FLAG; // Disable IRQs
        self.cpsr &= !T_FLAG; // Switch to ARM mode
        self.regs[15] = vector;
    }

    /// Handle a software interrupt (SWI) with HLE BIOS emulation.
    pub fn handle_swi(&mut self, bus: &mut Bus, comment: u32) {
        match comment {
            0x00 => {
                // SoftReset — jump to ROM entry
                self.regs[15] = 0x08000000;
                self.cpsr = CpuMode::System as u32;
            }
            0x02 => {
                // Halt
                bus.io.halted = true;
            }
            0x05 => {
                // VBlankIntrWait — set wait flags and halt
                bus.io.halted = true;
            }
            0x06 => {
                // Div: R0 = R0 / R1, R1 = R0 % R1, R3 = |R0/R1|
                let num = self.regs[0] as i32;
                let den = self.regs[1] as i32;
                if den != 0 {
                    self.regs[0] = (num / den) as u32;
                    self.regs[1] = (num % den) as u32;
                    self.regs[3] = (num / den).unsigned_abs();
                }
            }
            0x08 => {
                // Sqrt: R0 = sqrt(R0)
                let val = self.regs[0] as f64;
                self.regs[0] = val.sqrt() as u32;
            }
            0x0B | 0x0C => {
                // CpuSet / CpuFastSet — memory fill/copy
                let src = self.regs[0];
                let dst = self.regs[1];
                let ctrl = self.regs[2];
                let count = ctrl & 0x1FFFFF;
                let fixed = ctrl & (1 << 24) != 0;
                let word32 = ctrl & (1 << 26) != 0;

                if word32 {
                    for i in 0..count {
                        let s = if fixed { src } else { src + i * 4 };
                        let val = bus.read32(s);
                        bus.write32(dst + i * 4, val);
                    }
                } else {
                    for i in 0..count {
                        let s = if fixed { src } else { src + i * 2 };
                        let val = bus.read16(s);
                        bus.write16(dst + i * 2, val);
                    }
                }
            }
            _ => {} // Unimplemented SWI — do nothing
        }
    }
}
