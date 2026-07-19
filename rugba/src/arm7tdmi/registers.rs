/// ARM7TDMI operating modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuMode {
    System = 0x1F,
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
}

impl CpuMode {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0x1F {
            0x10 => CpuMode::User,
            0x11 => CpuMode::Fiq,
            0x12 => CpuMode::Irq,
            0x13 => CpuMode::Supervisor,
            0x17 => CpuMode::Abort,
            0x1B => CpuMode::Undefined,
            _ => CpuMode::System,
        }
    }

    pub fn bank_index(self) -> usize {
        match self {
            CpuMode::System | CpuMode::User => 0,
            CpuMode::Fiq => 1,
            CpuMode::Irq => 2,
            CpuMode::Supervisor => 3,
            CpuMode::Abort => 4,
            CpuMode::Undefined => 5,
        }
    }
}

/// CPSR/SPSR flag bit positions
pub const N_FLAG: u32 = 1 << 31;
pub const Z_FLAG: u32 = 1 << 30;
pub const C_FLAG: u32 = 1 << 29;
pub const V_FLAG: u32 = 1 << 28;
pub const I_FLAG: u32 = 1 << 7;
pub const F_FLAG: u32 = 1 << 6;
pub const T_FLAG: u32 = 1 << 5;

/// Banked registers for each mode
pub struct BankedRegisters {
    /// R13 (SP) banked per mode (6 banks: Sys/Usr, FIQ, IRQ, SVC, ABT, UND)
    pub r13: [u32; 6],
    /// R14 (LR) banked per mode
    pub r14: [u32; 6],
    /// SPSR banked per mode (index 0 unused — System/User have no SPSR)
    pub spsr: [u32; 6],
    /// R8-R12 FIQ-banked (only FIQ has separate copies)
    pub fiq_r8_r12: [u32; 5],
    /// R8-R12 non-FIQ (shared by all other modes)
    pub usr_r8_r12: [u32; 5],
}

impl BankedRegisters {
    pub fn new() -> Self {
        BankedRegisters {
            r13: [0x03007F00, 0x03007F00, 0x03007FA0, 0x03007FE0, 0x03007F00, 0x03007F00],
            r14: [0; 6],
            spsr: [0; 6],
            fiq_r8_r12: [0; 5],
            usr_r8_r12: [0; 5],
        }
    }
}
