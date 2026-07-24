#[cfg(test)]
mod tests {
    use crate::arm7tdmi::*;
    use crate::bus::Bus;

    fn make_cpu_bus() -> (Arm7Tdmi, Bus) {
        // Minimal ROM with NOP sled
        let rom = vec![0u8; 0x100];
        let cpu = Arm7Tdmi::new();
        let bus = Bus::new(rom);
        (cpu, bus)
    }

    #[test]
    fn test_cpu_initial_state() {
        let cpu = Arm7Tdmi::new();
        assert_eq!(cpu.regs[15], 0x08000000); // PC at ROM start
        assert_eq!(cpu.regs[13], 0x03007F00); // SP in IWRAM
        assert_eq!(cpu.mode(), CpuMode::System);
        assert!(!cpu.in_thumb_mode());
        assert!(cpu.get_flag(I_FLAG)); // IRQs disabled
    }

    #[test]
    fn test_condition_codes() {
        let mut cpu = Arm7Tdmi::new();

        // EQ: Z set
        cpu.cpsr |= Z_FLAG;
        assert!(cpu.check_condition(0x0));
        assert!(!cpu.check_condition(0x1)); // NE

        // CS: C set
        cpu.cpsr = CpuMode::System as u32 | C_FLAG;
        assert!(cpu.check_condition(0x2));
        assert!(!cpu.check_condition(0x3)); // CC

        // MI: N set
        cpu.cpsr = CpuMode::System as u32 | N_FLAG;
        assert!(cpu.check_condition(0x4));
        assert!(!cpu.check_condition(0x5)); // PL

        // AL: always
        cpu.cpsr = CpuMode::System as u32;
        assert!(cpu.check_condition(0xE));
    }

    #[test]
    fn test_set_nz_flags() {
        let mut cpu = Arm7Tdmi::new();

        cpu.set_nz(0);
        assert!(cpu.get_flag(Z_FLAG));
        assert!(!cpu.get_flag(N_FLAG));

        cpu.set_nz(0x8000_0000);
        assert!(!cpu.get_flag(Z_FLAG));
        assert!(cpu.get_flag(N_FLAG));

        cpu.set_nz(42);
        assert!(!cpu.get_flag(Z_FLAG));
        assert!(!cpu.get_flag(N_FLAG));
    }

    #[test]
    fn test_mode_switching() {
        let mut cpu = Arm7Tdmi::new();
        cpu.regs[13] = 0x1111;
        cpu.regs[14] = 0x2222;

        cpu.switch_mode(CpuMode::Irq);
        assert_eq!(cpu.mode(), CpuMode::Irq);
        // IRQ has its own SP/LR
        assert_ne!(cpu.regs[13], 0x1111);

        // Switch back
        cpu.switch_mode(CpuMode::System);
        assert_eq!(cpu.regs[13], 0x1111);
        assert_eq!(cpu.regs[14], 0x2222);
    }

    #[test]
    fn test_enter_exception() {
        let mut cpu = Arm7Tdmi::new();
        cpu.cpsr = CpuMode::System as u32 | T_FLAG; // In THUMB mode
        cpu.regs[15] = 0x08000100;

        cpu.enter_exception(CpuMode::Irq, 0x18);

        assert_eq!(cpu.mode(), CpuMode::Irq);
        assert_eq!(cpu.regs[15], 0x18); // PC = IRQ vector
        assert!(!cpu.in_thumb_mode()); // Switched to ARM
        assert!(cpu.get_flag(I_FLAG)); // IRQs disabled
        assert_eq!(cpu.regs[14], 0x08000100); // LR = return address
    }

    #[test]
    fn test_swi_div() {
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 10;
        cpu.regs[1] = 3;

        cpu.handle_swi(&mut bus, 0x06); // Div

        assert_eq!(cpu.regs[0], 3); // 10 / 3 = 3
        assert_eq!(cpu.regs[1], 1); // 10 % 3 = 1
        assert_eq!(cpu.regs[3], 3); // |10/3| = 3
    }

    #[test]
    fn test_swi_div_negative() {
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = (-10i32) as u32;
        cpu.regs[1] = 3;

        cpu.handle_swi(&mut bus, 0x06);

        assert_eq!(cpu.regs[0] as i32, -3); // -10 / 3 = -3
        assert_eq!(cpu.regs[1] as i32, -1); // -10 % 3 = -1
        assert_eq!(cpu.regs[3], 3); // |-10/3| = 3
    }

    #[test]
    fn test_swi_sqrt() {
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 144;
        cpu.handle_swi(&mut bus, 0x08);
        assert_eq!(cpu.regs[0], 12);
    }

    #[test]
    fn test_halted_cpu_wakes_on_irq() {
        let (mut cpu, mut bus) = make_cpu_bus();
        bus.io.halted = true;
        bus.io.ie = 0x01; // V-blank IRQ enabled
        bus.io.irq_flags = 0x00; // No pending IRQ

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 1);
        assert!(bus.io.halted); // Still halted

        bus.io.irq_flags = 0x01; // V-blank IRQ fires
        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 1);
        assert!(!bus.io.halted); // Woke up
    }
}
