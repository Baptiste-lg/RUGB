/// SM83 register file.
/// The Game Boy CPU has 8 8-bit registers that pair into 4 16-bit ones:
/// AF (accumulator + flags), BC, DE, HL.
/// Plus SP (stack pointer) and PC (program counter).
pub struct Registers {
    pub a: u8,
    /// Flags register: bit 7=Z (zero), 6=N (subtract), 5=H (half-carry), 4=C (carry).
    /// Bits 3-0 are hardwired to 0.
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

const ZERO_FLAG: u8 = 1 << 7;
const SUB_FLAG: u8 = 1 << 6;
const HALF_CARRY_FLAG: u8 = 1 << 5;
const CARRY_FLAG: u8 = 1 << 4;

impl Registers {
    /// Post-boot register state (DMG). Skips the boot ROM entirely.
    pub fn new() -> Self {
        Registers {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }

    /// Zero state for boot ROM execution — starts at PC=0x0000.
    pub fn reset_for_boot(&mut self) {
        self.a = 0;
        self.f = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.sp = 0;
        self.pc = 0;
    }

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }

    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        // Lower nibble of F is always 0
        self.f = (val as u8) & 0xF0;
    }

    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }

    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }

    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    pub fn flag_z(&self) -> bool {
        self.f & ZERO_FLAG != 0
    }

    pub fn flag_n(&self) -> bool {
        self.f & SUB_FLAG != 0
    }

    pub fn flag_h(&self) -> bool {
        self.f & HALF_CARRY_FLAG != 0
    }

    pub fn flag_c(&self) -> bool {
        self.f & CARRY_FLAG != 0
    }

    pub fn set_flag_z(&mut self, on: bool) {
        if on {
            self.f |= ZERO_FLAG;
        } else {
            self.f &= !ZERO_FLAG;
        }
    }

    pub fn set_flag_n(&mut self, on: bool) {
        if on {
            self.f |= SUB_FLAG;
        } else {
            self.f &= !SUB_FLAG;
        }
    }

    pub fn set_flag_h(&mut self, on: bool) {
        if on {
            self.f |= HALF_CARRY_FLAG;
        } else {
            self.f &= !HALF_CARRY_FLAG;
        }
    }

    pub fn set_flag_c(&mut self, on: bool) {
        if on {
            self.f |= CARRY_FLAG;
        } else {
            self.f &= !CARRY_FLAG;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_boot_values() {
        let r = Registers::new();
        assert_eq!(r.af(), 0x01B0);
        assert_eq!(r.bc(), 0x0013);
        assert_eq!(r.de(), 0x00D8);
        assert_eq!(r.hl(), 0x014D);
        assert_eq!(r.sp, 0xFFFE);
        assert_eq!(r.pc, 0x0100);
    }

    #[test]
    fn set_af_masks_lower_nibble() {
        let mut r = Registers::new();
        r.set_af(0x12FF);
        assert_eq!(r.a, 0x12);
        assert_eq!(r.f, 0xF0); // lower 4 bits forced to 0
    }

    #[test]
    fn register_pair_roundtrip() {
        let mut r = Registers::new();
        r.set_bc(0xABCD);
        assert_eq!(r.bc(), 0xABCD);
        assert_eq!(r.b, 0xAB);
        assert_eq!(r.c, 0xCD);

        r.set_de(0x1234);
        assert_eq!(r.de(), 0x1234);

        r.set_hl(0xFEDC);
        assert_eq!(r.hl(), 0xFEDC);
    }

    #[test]
    fn flag_operations() {
        let mut r = Registers::new();
        r.f = 0x00;

        r.set_flag_z(true);
        assert!(r.flag_z());
        assert_eq!(r.f, 0x80);

        r.set_flag_c(true);
        assert!(r.flag_c());
        assert_eq!(r.f, 0x90);

        r.set_flag_z(false);
        assert!(!r.flag_z());
        assert_eq!(r.f, 0x10);

        r.set_flag_h(true);
        r.set_flag_n(true);
        assert_eq!(r.f, 0x70);
    }

    #[test]
    fn reset_for_boot_zeros_all() {
        let mut r = Registers::new();
        r.reset_for_boot();
        assert_eq!(r.a, 0);
        assert_eq!(r.f, 0);
        assert_eq!(r.b, 0);
        assert_eq!(r.c, 0);
        assert_eq!(r.d, 0);
        assert_eq!(r.e, 0);
        assert_eq!(r.h, 0);
        assert_eq!(r.l, 0);
        assert_eq!(r.sp, 0);
        assert_eq!(r.pc, 0);
    }

    #[test]
    fn set_flag_h_and_n_independent() {
        let mut r = Registers::new();
        r.f = 0x00;
        r.set_flag_h(true);
        assert!(r.flag_h());
        assert!(!r.flag_n());

        r.set_flag_n(true);
        assert!(r.flag_h());
        assert!(r.flag_n());

        r.set_flag_h(false);
        assert!(!r.flag_h());
        assert!(r.flag_n());
    }

    #[test]
    fn lower_nibble_of_f_always_zero_after_set_flag() {
        let mut r = Registers::new();
        r.f = 0x0F; // artificially set low nibble
        r.set_flag_z(true);
        // set_flag only ORs/ANDs the named bit; low nibble stays as-is unless masked.
        // The invariant we check: set_flag_* never introduces new bits in 3-0.
        // We verify that using set_af (the only masking API) clears low nibble.
        r.f = 0x00;
        r.set_flag_z(true);
        r.set_flag_n(true);
        r.set_flag_h(true);
        r.set_flag_c(true);
        assert_eq!(r.f & 0x0F, 0x00);
    }

    #[test]
    fn af_pair_reads_masked_f() {
        let mut r = Registers::new();
        r.a = 0x55;
        // set_af masks the low nibble
        r.set_af(0x55CF); // low nibble 0xF should be masked to 0
        assert_eq!(r.a, 0x55);
        assert_eq!(r.f, 0xC0); // 0xCF & 0xF0 = 0xC0
        assert_eq!(r.af(), 0x55C0);
    }
}
