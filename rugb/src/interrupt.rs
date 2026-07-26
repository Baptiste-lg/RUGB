use crate::cpu::Cpu;
use crate::mmu::Mmu;

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

#[cfg(test)]
mod tests {
    use super::handle_interrupts;
    use crate::cpu::Cpu;
    use crate::mmu::Mmu;

    /// Helper: build a fully armed cpu+mmu ready for a dispatch test.
    /// IE and IF are both set to `flag_bits`, IME=true, SP=0xFFFE, PC=0x0150.
    fn armed(flag_bits: u8) -> (Cpu, Mmu) {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        cpu.ime = true;
        cpu.regs.sp = 0xFFFE;
        cpu.regs.pc = 0x0150;
        mmu.ie = flag_bits;
        mmu.interrupt_flag = flag_bits;
        (cpu, mmu)
    }

    // ------------------------------------------------------------------
    // No-pending cases
    // ------------------------------------------------------------------

    #[test]
    fn no_pending_returns_0() {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        // IE=0 and IF=0 — nothing pending
        assert_eq!(handle_interrupts(&mut cpu, &mut mmu), 0);
    }

    // ------------------------------------------------------------------
    // HALT wakeup
    // ------------------------------------------------------------------

    #[test]
    fn halt_cleared_by_pending() {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        cpu.halted = true;
        cpu.ime = false; // IME off — wake but do not dispatch
        mmu.ie = 0x01;
        mmu.interrupt_flag = 0x01;
        let cycles = handle_interrupts(&mut cpu, &mut mmu);
        assert!(!cpu.halted, "HALT should be cleared by pending interrupt");
        assert_eq!(cycles, 0, "no dispatch when IME=false");
    }

    // ------------------------------------------------------------------
    // IME=false — no dispatch
    // ------------------------------------------------------------------

    #[test]
    fn ime_false_no_dispatch() {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        cpu.ime = false;
        cpu.regs.pc = 0x0150;
        mmu.ie = 0x01;
        mmu.interrupt_flag = 0x01;
        let cycles = handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cycles, 0);
        assert_eq!(cpu.regs.pc, 0x0150, "PC must not change when IME=false");
    }

    // ------------------------------------------------------------------
    // Vector dispatch tests
    // ------------------------------------------------------------------

    #[test]
    fn dispatches_vblank() {
        let (mut cpu, mut mmu) = armed(0x01);
        handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cpu.regs.pc, 0x0040);
        assert_eq!(
            mmu.interrupt_flag & 0x01,
            0,
            "VBlank IF bit must be cleared"
        );
    }

    #[test]
    fn dispatches_stat() {
        let (mut cpu, mut mmu) = armed(0x02);
        handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cpu.regs.pc, 0x0048);
        assert_eq!(mmu.interrupt_flag & 0x02, 0);
    }

    #[test]
    fn dispatches_timer() {
        let (mut cpu, mut mmu) = armed(0x04);
        handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cpu.regs.pc, 0x0050);
        assert_eq!(mmu.interrupt_flag & 0x04, 0);
    }

    #[test]
    fn dispatches_serial() {
        let (mut cpu, mut mmu) = armed(0x08);
        handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cpu.regs.pc, 0x0058);
        assert_eq!(mmu.interrupt_flag & 0x08, 0);
    }

    #[test]
    fn dispatches_joypad() {
        let (mut cpu, mut mmu) = armed(0x10);
        handle_interrupts(&mut cpu, &mut mmu);
        assert_eq!(cpu.regs.pc, 0x0060);
        assert_eq!(mmu.interrupt_flag & 0x10, 0);
    }

    // ------------------------------------------------------------------
    // Post-dispatch state
    // ------------------------------------------------------------------

    #[test]
    fn ime_cleared_after_dispatch() {
        let (mut cpu, mut mmu) = armed(0x01);
        handle_interrupts(&mut cpu, &mut mmu);
        assert!(!cpu.ime, "IME must be cleared after servicing an interrupt");
    }

    #[test]
    fn pc_pushed_to_stack() {
        let (mut cpu, mut mmu) = armed(0x01);
        // SP starts at 0xFFFE, PC at 0x0150
        handle_interrupts(&mut cpu, &mut mmu);
        // SP should have decremented by 2
        assert_eq!(cpu.regs.sp, 0xFFFC);
        // Stacked value is the old PC (0x0150), little-endian
        let lo = mmu.read(cpu.regs.sp) as u16;
        let hi = mmu.read(cpu.regs.sp + 1) as u16;
        let stacked_pc = (hi << 8) | lo;
        assert_eq!(stacked_pc, 0x0150, "stacked PC must equal the old PC");
    }

    #[test]
    fn returns_20_cycles() {
        let (mut cpu, mut mmu) = armed(0x01);
        assert_eq!(handle_interrupts(&mut cpu, &mut mmu), 20);
    }

    // ------------------------------------------------------------------
    // Priority: only lowest bit is serviced
    // ------------------------------------------------------------------

    #[test]
    fn only_lowest_priority() {
        // Both VBlank (bit 0) and STAT (bit 1) pending
        let (mut cpu, mut mmu) = armed(0x03);
        handle_interrupts(&mut cpu, &mut mmu);
        // Bit 0 serviced → VBlank vector
        assert_eq!(cpu.regs.pc, 0x0040);
        // Bit 1 (STAT) must still be set in IF
        assert_ne!(mmu.interrupt_flag & 0x02, 0, "STAT IF bit must remain set");
        // Bit 0 (VBlank) must be cleared
        assert_eq!(
            mmu.interrupt_flag & 0x01,
            0,
            "VBlank IF bit must be cleared"
        );
    }
}
