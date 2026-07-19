mod cb_opcodes;
mod opcodes;
pub mod registers;

use crate::mmu::Mmu;
use crate::savestate::*;
use registers::Registers;

pub struct Cpu {
    pub regs: Registers,
    pub halted: bool,
    /// Interrupt Master Enable — gates all interrupt handling
    pub ime: bool,
    /// EI sets this; IME becomes true after the *next* instruction
    pub ime_scheduled: bool,
    /// HALT bug: when HALT runs with IME=0 and pending interrupts,
    /// the next opcode byte is read twice (PC fails to advance)
    pub halt_bug: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            regs: Registers::new(),
            halted: false,
            ime: false,
            ime_scheduled: false,
            halt_bug: false,
        }
    }

    pub fn step(&mut self, mmu: &mut Mmu) -> u32 {
        // Handle delayed IME enable (EI takes effect after the following instruction)
        if self.ime_scheduled {
            self.ime_scheduled = false;
            self.ime = true;
        }

        if self.halted {
            return 4;
        }

        let opcode = self.fetch_byte(mmu);

        // HALT bug: the byte we just fetched should be read again
        if self.halt_bug {
            self.halt_bug = false;
            self.regs.pc = self.regs.pc.wrapping_sub(1);
        }

        self.execute(opcode, mmu)
    }

    /// Read byte at PC and advance PC
    fn fetch_byte(&mut self, mmu: &Mmu) -> u8 {
        let val = mmu.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        val
    }

    /// Read 16-bit little-endian value at PC and advance PC by 2
    fn fetch_word(&mut self, mmu: &Mmu) -> u16 {
        let lo = self.fetch_byte(mmu) as u16;
        let hi = self.fetch_byte(mmu) as u16;
        hi << 8 | lo
    }

    /// Push a 16-bit value onto the stack (SP decrements first, big end at higher addr)
    fn push(&mut self, mmu: &mut Mmu, val: u16) {
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.write(self.regs.sp, (val >> 8) as u8);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.write(self.regs.sp, val as u8);
    }

    /// Pop a 16-bit value from the stack
    fn pop(&mut self, mmu: &Mmu) -> u16 {
        let lo = mmu.read(self.regs.sp) as u16;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let hi = mmu.read(self.regs.sp) as u16;
        self.regs.sp = self.regs.sp.wrapping_add(1);
        hi << 8 | lo
    }

    // -- ALU helpers --

    fn alu_add(&mut self, val: u8) {
        let a = self.regs.a;
        let result = a as u16 + val as u16;
        self.regs.a = result as u8;
        self.regs.set_flag_z(self.regs.a == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h((a & 0x0F) + (val & 0x0F) > 0x0F);
        self.regs.set_flag_c(result > 0xFF);
    }

    fn alu_adc(&mut self, val: u8) {
        let carry = if self.regs.flag_c() { 1u8 } else { 0 };
        let a = self.regs.a;
        let result = a as u16 + val as u16 + carry as u16;
        self.regs.a = result as u8;
        self.regs.set_flag_z(self.regs.a == 0);
        self.regs.set_flag_n(false);
        self.regs
            .set_flag_h((a & 0x0F) + (val & 0x0F) + carry > 0x0F);
        self.regs.set_flag_c(result > 0xFF);
    }

    fn alu_sub(&mut self, val: u8) {
        let a = self.regs.a;
        let result = a.wrapping_sub(val);
        self.regs.a = result;
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(true);
        self.regs.set_flag_h((a & 0x0F) < (val & 0x0F));
        self.regs.set_flag_c(a < val);
    }

    fn alu_sbc(&mut self, val: u8) {
        let carry = if self.regs.flag_c() { 1u8 } else { 0 };
        let a = self.regs.a;
        let result = (a as u16)
            .wrapping_sub(val as u16)
            .wrapping_sub(carry as u16);
        self.regs.a = result as u8;
        self.regs.set_flag_z(result as u8 == 0);
        self.regs.set_flag_n(true);
        self.regs.set_flag_h((a & 0x0F) < (val & 0x0F) + carry);
        self.regs.set_flag_c(result > 0xFF);
    }

    fn alu_and(&mut self, val: u8) {
        self.regs.a &= val;
        self.regs.set_flag_z(self.regs.a == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(true);
        self.regs.set_flag_c(false);
    }

    fn alu_xor(&mut self, val: u8) {
        self.regs.a ^= val;
        self.regs.set_flag_z(self.regs.a == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(false);
    }

    fn alu_or(&mut self, val: u8) {
        self.regs.a |= val;
        self.regs.set_flag_z(self.regs.a == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(false);
    }

    /// CP is subtraction with the result thrown away — only flags change
    fn alu_cp(&mut self, val: u8) {
        let a = self.regs.a;
        self.alu_sub(val);
        self.regs.a = a;
    }

    fn alu_inc(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h((val & 0x0F) + 1 > 0x0F);
        // C flag not affected
        result
    }

    fn alu_dec(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(true);
        self.regs.set_flag_h((val & 0x0F) == 0);
        // C flag not affected
        result
    }

    /// 16-bit ADD HL,rr — only sets N, H, C (Z untouched)
    fn alu_add_hl(&mut self, val: u16) {
        let hl = self.regs.hl();
        let result = hl as u32 + val as u32;
        self.regs.set_hl(result as u16);
        self.regs.set_flag_n(false);
        self.regs
            .set_flag_h((hl & 0x0FFF) + (val & 0x0FFF) > 0x0FFF);
        self.regs.set_flag_c(result > 0xFFFF);
    }

    // -- Rotate helpers (non-CB versions always clear Z) --

    fn rlca(&mut self) {
        let a = self.regs.a;
        let carry = a >> 7;
        self.regs.a = (a << 1) | carry;
        self.regs.set_flag_z(false);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(carry != 0);
    }

    fn rrca(&mut self) {
        let a = self.regs.a;
        let carry = a & 1;
        self.regs.a = (a >> 1) | (carry << 7);
        self.regs.set_flag_z(false);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(carry != 0);
    }

    fn rla(&mut self) {
        let a = self.regs.a;
        let old_carry = if self.regs.flag_c() { 1u8 } else { 0 };
        self.regs.a = (a << 1) | old_carry;
        self.regs.set_flag_z(false);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(a & 0x80 != 0);
    }

    fn rra(&mut self) {
        let a = self.regs.a;
        let old_carry = if self.regs.flag_c() { 1u8 } else { 0 };
        self.regs.a = (a >> 1) | (old_carry << 7);
        self.regs.set_flag_z(false);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(a & 1 != 0);
    }

    // -- CB-prefixed rotate/shift helpers (these DO set Z normally) --

    pub(crate) fn cb_rlc(&mut self, val: u8) -> u8 {
        let carry = val >> 7;
        let result = (val << 1) | carry;
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(carry != 0);
        result
    }

    pub(crate) fn cb_rrc(&mut self, val: u8) -> u8 {
        let carry = val & 1;
        let result = (val >> 1) | (carry << 7);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(carry != 0);
        result
    }

    pub(crate) fn cb_rl(&mut self, val: u8) -> u8 {
        let old_carry = if self.regs.flag_c() { 1u8 } else { 0 };
        let result = (val << 1) | old_carry;
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(val & 0x80 != 0);
        result
    }

    pub(crate) fn cb_rr(&mut self, val: u8) -> u8 {
        let old_carry = if self.regs.flag_c() { 1u8 } else { 0 };
        let result = (val >> 1) | (old_carry << 7);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(val & 1 != 0);
        result
    }

    pub(crate) fn cb_sla(&mut self, val: u8) -> u8 {
        let result = val << 1;
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(val & 0x80 != 0);
        result
    }

    /// Arithmetic right shift — bit 7 stays the same (sign-preserving)
    pub(crate) fn cb_sra(&mut self, val: u8) -> u8 {
        let result = (val >> 1) | (val & 0x80);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(val & 1 != 0);
        result
    }

    pub(crate) fn cb_swap(&mut self, val: u8) -> u8 {
        let result = val.rotate_left(4);
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(false);
        result
    }

    /// Logical right shift — bit 7 becomes 0
    pub(crate) fn cb_srl(&mut self, val: u8) -> u8 {
        let result = val >> 1;
        self.regs.set_flag_z(result == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(false);
        self.regs.set_flag_c(val & 1 != 0);
        result
    }

    pub(crate) fn cb_bit(&mut self, bit: u8, val: u8) {
        self.regs.set_flag_z(val & (1 << bit) == 0);
        self.regs.set_flag_n(false);
        self.regs.set_flag_h(true);
        // C flag not affected
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_u8(d, self.regs.a);
        push_u8(d, self.regs.f);
        push_u8(d, self.regs.b);
        push_u8(d, self.regs.c);
        push_u8(d, self.regs.d);
        push_u8(d, self.regs.e);
        push_u8(d, self.regs.h);
        push_u8(d, self.regs.l);
        push_u16(d, self.regs.sp);
        push_u16(d, self.regs.pc);
        push_bool(d, self.halted);
        push_bool(d, self.ime);
        push_bool(d, self.ime_scheduled);
        push_bool(d, self.halt_bug);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.regs.a = pop_u8(d);
        self.regs.f = pop_u8(d);
        self.regs.b = pop_u8(d);
        self.regs.c = pop_u8(d);
        self.regs.d = pop_u8(d);
        self.regs.e = pop_u8(d);
        self.regs.h = pop_u8(d);
        self.regs.l = pop_u8(d);
        self.regs.sp = pop_u16(d);
        self.regs.pc = pop_u16(d);
        self.halted = pop_bool(d);
        self.ime = pop_bool(d);
        self.ime_scheduled = pop_bool(d);
        self.halt_bug = pop_bool(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cpu_mmu() -> (Cpu, Mmu) {
        (Cpu::new(), Mmu::new())
    }

    #[test]
    fn alu_add_basic() {
        let (mut cpu, _) = make_cpu_mmu();
        cpu.regs.a = 0x3A;
        cpu.alu_add(0x07);
        assert_eq!(cpu.regs.a, 0x41);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(cpu.regs.flag_h()); // 0xA + 0x7 = 0x11 > 0xF
        assert!(!cpu.regs.flag_c());
    }

    #[test]
    fn alu_add_overflow() {
        let (mut cpu, _) = make_cpu_mmu();
        cpu.regs.a = 0xFF;
        cpu.alu_add(0x01);
        assert_eq!(cpu.regs.a, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_c());
        assert!(cpu.regs.flag_h());
    }

    #[test]
    fn alu_sub_borrow() {
        let (mut cpu, _) = make_cpu_mmu();
        cpu.regs.a = 0x10;
        cpu.alu_sub(0x20);
        assert_eq!(cpu.regs.a, 0xF0);
        assert!(!cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert!(cpu.regs.flag_c());
    }

    #[test]
    fn alu_cp_does_not_modify_a() {
        let (mut cpu, _) = make_cpu_mmu();
        cpu.regs.a = 0x42;
        cpu.alu_cp(0x42);
        assert_eq!(cpu.regs.a, 0x42);
        assert!(cpu.regs.flag_z());
    }

    #[test]
    fn push_pop_roundtrip() {
        let (mut cpu, mut mmu) = make_cpu_mmu();
        cpu.regs.sp = 0xFFFE;
        cpu.push(&mut mmu, 0x1234);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        let val = cpu.pop(&mmu);
        assert_eq!(val, 0x1234);
        assert_eq!(cpu.regs.sp, 0xFFFE);
    }

    #[test]
    fn inc_half_carry() {
        let (mut cpu, _) = make_cpu_mmu();
        let result = cpu.alu_inc(0x0F);
        assert_eq!(result, 0x10);
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_z());
    }

    #[test]
    fn dec_to_zero() {
        let (mut cpu, _) = make_cpu_mmu();
        let result = cpu.alu_dec(0x01);
        assert_eq!(result, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
    }

    #[test]
    fn cb_swap_nonzero() {
        let (mut cpu, _) = make_cpu_mmu();
        let result = cpu.cb_swap(0xAB);
        assert_eq!(result, 0xBA);
        assert!(!cpu.regs.flag_z());
    }

    #[test]
    fn cb_swap_zero() {
        let (mut cpu, _) = make_cpu_mmu();
        let result = cpu.cb_swap(0x00);
        assert_eq!(result, 0x00);
        assert!(cpu.regs.flag_z());
    }

    #[test]
    fn cb_bit_test() {
        let (mut cpu, _) = make_cpu_mmu();
        cpu.cb_bit(0, 0x01);
        assert!(!cpu.regs.flag_z()); // bit 0 is set
        cpu.cb_bit(1, 0x01);
        assert!(cpu.regs.flag_z()); // bit 1 is not set
    }
}
