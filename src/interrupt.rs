use crate::cpu::Cpu;
use crate::mmu::Mmu;

//! Interrupt controller — dispatches pending interrupts by priority.

/// Interrupt vectors:
///   VBlank=0x0040(bit 0), STAT=0x0048(bit 1), Timer=0x0050(bit 2),
///   Serial=0x0058(bit 3), Joypad=0x0060(bit 4)
pub fn handle_interrupts(cpu: &mut Cpu, mmu: &mut Mmu) -> u32 {
    let pending = mmu.interrupt_flag & mmu.ie & 0x1F;

    if pending != 0 {
        // Any pending+enabled interrupt wakes the CPU from HALT
        cpu.halted = false;
    }

    if !cpu.ime || pending == 0 {
        return 0;
    }

    // Service the highest-priority pending interrupt (lowest bit number)
    for bit in 0..5u8 {
        if pending & (1 << bit) != 0 {
            cpu.ime = false;
            // Acknowledge: clear this interrupt's flag
            mmu.interrupt_flag &= !(1 << bit);
            // Push current PC and jump to the interrupt vector
            let pc = cpu.regs.pc;
            cpu.regs.sp = cpu.regs.sp.wrapping_sub(1);
            mmu.write(cpu.regs.sp, (pc >> 8) as u8);
            cpu.regs.sp = cpu.regs.sp.wrapping_sub(1);
            mmu.write(cpu.regs.sp, pc as u8);
            cpu.regs.pc = 0x0040 + (bit as u16) * 8;
            return 20; // Interrupt dispatch costs 5 M-cycles
        }
    }
    0
}
