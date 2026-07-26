use super::Cpu;
use crate::mmu::Mmu;

impl Cpu {
    /// Decode and execute one base opcode. Returns T-cycle count.
    pub(crate) fn execute(&mut self, opcode: u8, mmu: &mut Mmu) -> u32 {
        match opcode {
            // --- NOP ---
            0x00 => 4,

            // --- LD rr, nn ---
            0x01 => {
                let v = self.fetch_word(mmu);
                self.regs.set_bc(v);
                12
            }
            0x11 => {
                let v = self.fetch_word(mmu);
                self.regs.set_de(v);
                12
            }
            0x21 => {
                let v = self.fetch_word(mmu);
                self.regs.set_hl(v);
                12
            }
            0x31 => {
                self.regs.sp = self.fetch_word(mmu);
                12
            }

            // --- LD (rr), A / LD A, (rr) ---
            0x02 => {
                mmu.write(self.regs.bc(), self.regs.a);
                8
            }
            0x12 => {
                mmu.write(self.regs.de(), self.regs.a);
                8
            }
            0x0A => {
                self.regs.a = mmu.read(self.regs.bc());
                8
            }
            0x1A => {
                self.regs.a = mmu.read(self.regs.de());
                8
            }

            // --- LD (HL+/-), A / LD A, (HL+/-) ---
            0x22 => {
                let hl = self.regs.hl();
                mmu.write(hl, self.regs.a);
                self.regs.set_hl(hl.wrapping_add(1));
                8
            }
            0x32 => {
                let hl = self.regs.hl();
                mmu.write(hl, self.regs.a);
                self.regs.set_hl(hl.wrapping_sub(1));
                8
            }
            0x2A => {
                let hl = self.regs.hl();
                self.regs.a = mmu.read(hl);
                self.regs.set_hl(hl.wrapping_add(1));
                8
            }
            0x3A => {
                let hl = self.regs.hl();
                self.regs.a = mmu.read(hl);
                self.regs.set_hl(hl.wrapping_sub(1));
                8
            }

            // --- LD (nn), SP ---
            0x08 => {
                let addr = self.fetch_word(mmu);
                mmu.write(addr, self.regs.sp as u8);
                mmu.write(addr.wrapping_add(1), (self.regs.sp >> 8) as u8);
                20
            }

            // --- INC rr ---
            0x03 => {
                self.regs.set_bc(self.regs.bc().wrapping_add(1));
                8
            }
            0x13 => {
                self.regs.set_de(self.regs.de().wrapping_add(1));
                8
            }
            0x23 => {
                self.regs.set_hl(self.regs.hl().wrapping_add(1));
                8
            }
            0x33 => {
                self.regs.sp = self.regs.sp.wrapping_add(1);
                8
            }

            // --- DEC rr ---
            0x0B => {
                self.regs.set_bc(self.regs.bc().wrapping_sub(1));
                8
            }
            0x1B => {
                self.regs.set_de(self.regs.de().wrapping_sub(1));
                8
            }
            0x2B => {
                self.regs.set_hl(self.regs.hl().wrapping_sub(1));
                8
            }
            0x3B => {
                self.regs.sp = self.regs.sp.wrapping_sub(1);
                8
            }

            // --- ADD HL, rr ---
            0x09 => {
                let v = self.regs.bc();
                self.alu_add_hl(v);
                8
            }
            0x19 => {
                let v = self.regs.de();
                self.alu_add_hl(v);
                8
            }
            0x29 => {
                let v = self.regs.hl();
                self.alu_add_hl(v);
                8
            }
            0x39 => {
                let v = self.regs.sp;
                self.alu_add_hl(v);
                8
            }

            // --- INC r ---
            0x04 => {
                self.regs.b = self.alu_inc(self.regs.b);
                4
            }
            0x0C => {
                self.regs.c = self.alu_inc(self.regs.c);
                4
            }
            0x14 => {
                self.regs.d = self.alu_inc(self.regs.d);
                4
            }
            0x1C => {
                self.regs.e = self.alu_inc(self.regs.e);
                4
            }
            0x24 => {
                self.regs.h = self.alu_inc(self.regs.h);
                4
            }
            0x2C => {
                self.regs.l = self.alu_inc(self.regs.l);
                4
            }
            0x34 => {
                let hl = self.regs.hl();
                let v = self.alu_inc(mmu.read(hl));
                mmu.write(hl, v);
                12
            }
            0x3C => {
                self.regs.a = self.alu_inc(self.regs.a);
                4
            }

            // --- DEC r ---
            0x05 => {
                self.regs.b = self.alu_dec(self.regs.b);
                4
            }
            0x0D => {
                self.regs.c = self.alu_dec(self.regs.c);
                4
            }
            0x15 => {
                self.regs.d = self.alu_dec(self.regs.d);
                4
            }
            0x1D => {
                self.regs.e = self.alu_dec(self.regs.e);
                4
            }
            0x25 => {
                self.regs.h = self.alu_dec(self.regs.h);
                4
            }
            0x2D => {
                self.regs.l = self.alu_dec(self.regs.l);
                4
            }
            0x35 => {
                let hl = self.regs.hl();
                let v = self.alu_dec(mmu.read(hl));
                mmu.write(hl, v);
                12
            }
            0x3D => {
                self.regs.a = self.alu_dec(self.regs.a);
                4
            }

            // --- LD r, n (8-bit immediate) ---
            0x06 => {
                self.regs.b = self.fetch_byte(mmu);
                8
            }
            0x0E => {
                self.regs.c = self.fetch_byte(mmu);
                8
            }
            0x16 => {
                self.regs.d = self.fetch_byte(mmu);
                8
            }
            0x1E => {
                self.regs.e = self.fetch_byte(mmu);
                8
            }
            0x26 => {
                self.regs.h = self.fetch_byte(mmu);
                8
            }
            0x2E => {
                self.regs.l = self.fetch_byte(mmu);
                8
            }
            0x36 => {
                let v = self.fetch_byte(mmu);
                mmu.write(self.regs.hl(), v);
                12
            }
            0x3E => {
                self.regs.a = self.fetch_byte(mmu);
                8
            }

            // --- Rotate A instructions ---
            0x07 => {
                self.rlca();
                4
            }
            0x0F => {
                self.rrca();
                4
            }
            0x17 => {
                self.rla();
                4
            }
            0x1F => {
                self.rra();
                4
            }

            // --- DAA ---
            0x27 => {
                // BCD adjust: fix A after BCD addition/subtraction
                let mut a = self.regs.a as i16;
                if !self.regs.flag_n() {
                    if self.regs.flag_h() || (a & 0x0F) > 9 {
                        a += 0x06;
                    }
                    if self.regs.flag_c() || a > 0x9F {
                        a += 0x60;
                    }
                } else {
                    if self.regs.flag_h() {
                        a = (a.wrapping_sub(6)) & 0xFF;
                    }
                    if self.regs.flag_c() {
                        a = a.wrapping_sub(0x60);
                    }
                }
                self.regs.set_flag_h(false);
                if a & 0x100 != 0 {
                    self.regs.set_flag_c(true);
                }
                self.regs.a = a as u8;
                self.regs.set_flag_z(self.regs.a == 0);
                4
            }

            // --- CPL (complement A) ---
            0x2F => {
                self.regs.a = !self.regs.a;
                self.regs.set_flag_n(true);
                self.regs.set_flag_h(true);
                4
            }

            // --- SCF (set carry flag) ---
            0x37 => {
                self.regs.set_flag_n(false);
                self.regs.set_flag_h(false);
                self.regs.set_flag_c(true);
                4
            }

            // --- CCF (complement carry flag) ---
            0x3F => {
                self.regs.set_flag_n(false);
                self.regs.set_flag_h(false);
                self.regs.set_flag_c(!self.regs.flag_c());
                4
            }

            // --- LD r, r' (0x40-0x7F except 0x76) ---
            // B is destination
            0x40 => 4, // LD B, B
            0x41 => {
                self.regs.b = self.regs.c;
                4
            }
            0x42 => {
                self.regs.b = self.regs.d;
                4
            }
            0x43 => {
                self.regs.b = self.regs.e;
                4
            }
            0x44 => {
                self.regs.b = self.regs.h;
                4
            }
            0x45 => {
                self.regs.b = self.regs.l;
                4
            }
            0x46 => {
                self.regs.b = mmu.read(self.regs.hl());
                8
            }
            0x47 => {
                self.regs.b = self.regs.a;
                4
            }
            // C is destination
            0x48 => {
                self.regs.c = self.regs.b;
                4
            }
            0x49 => 4, // LD C, C
            0x4A => {
                self.regs.c = self.regs.d;
                4
            }
            0x4B => {
                self.regs.c = self.regs.e;
                4
            }
            0x4C => {
                self.regs.c = self.regs.h;
                4
            }
            0x4D => {
                self.regs.c = self.regs.l;
                4
            }
            0x4E => {
                self.regs.c = mmu.read(self.regs.hl());
                8
            }
            0x4F => {
                self.regs.c = self.regs.a;
                4
            }
            // D is destination
            0x50 => {
                self.regs.d = self.regs.b;
                4
            }
            0x51 => {
                self.regs.d = self.regs.c;
                4
            }
            0x52 => 4, // LD D, D
            0x53 => {
                self.regs.d = self.regs.e;
                4
            }
            0x54 => {
                self.regs.d = self.regs.h;
                4
            }
            0x55 => {
                self.regs.d = self.regs.l;
                4
            }
            0x56 => {
                self.regs.d = mmu.read(self.regs.hl());
                8
            }
            0x57 => {
                self.regs.d = self.regs.a;
                4
            }
            // E is destination
            0x58 => {
                self.regs.e = self.regs.b;
                4
            }
            0x59 => {
                self.regs.e = self.regs.c;
                4
            }
            0x5A => {
                self.regs.e = self.regs.d;
                4
            }
            0x5B => 4, // LD E, E
            0x5C => {
                self.regs.e = self.regs.h;
                4
            }
            0x5D => {
                self.regs.e = self.regs.l;
                4
            }
            0x5E => {
                self.regs.e = mmu.read(self.regs.hl());
                8
            }
            0x5F => {
                self.regs.e = self.regs.a;
                4
            }
            // H is destination
            0x60 => {
                self.regs.h = self.regs.b;
                4
            }
            0x61 => {
                self.regs.h = self.regs.c;
                4
            }
            0x62 => {
                self.regs.h = self.regs.d;
                4
            }
            0x63 => {
                self.regs.h = self.regs.e;
                4
            }
            0x64 => 4, // LD H, H
            0x65 => {
                self.regs.h = self.regs.l;
                4
            }
            0x66 => {
                self.regs.h = mmu.read(self.regs.hl());
                8
            }
            0x67 => {
                self.regs.h = self.regs.a;
                4
            }
            // L is destination
            0x68 => {
                self.regs.l = self.regs.b;
                4
            }
            0x69 => {
                self.regs.l = self.regs.c;
                4
            }
            0x6A => {
                self.regs.l = self.regs.d;
                4
            }
            0x6B => {
                self.regs.l = self.regs.e;
                4
            }
            0x6C => {
                self.regs.l = self.regs.h;
                4
            }
            0x6D => 4, // LD L, L
            0x6E => {
                self.regs.l = mmu.read(self.regs.hl());
                8
            }
            0x6F => {
                self.regs.l = self.regs.a;
                4
            }
            // (HL) is destination
            0x70 => {
                mmu.write(self.regs.hl(), self.regs.b);
                8
            }
            0x71 => {
                mmu.write(self.regs.hl(), self.regs.c);
                8
            }
            0x72 => {
                mmu.write(self.regs.hl(), self.regs.d);
                8
            }
            0x73 => {
                mmu.write(self.regs.hl(), self.regs.e);
                8
            }
            0x74 => {
                mmu.write(self.regs.hl(), self.regs.h);
                8
            }
            0x75 => {
                mmu.write(self.regs.hl(), self.regs.l);
                8
            }
            // 0x76 = HALT (handled below)
            0x77 => {
                mmu.write(self.regs.hl(), self.regs.a);
                8
            }
            // A is destination
            0x78 => {
                self.regs.a = self.regs.b;
                4
            }
            0x79 => {
                self.regs.a = self.regs.c;
                4
            }
            0x7A => {
                self.regs.a = self.regs.d;
                4
            }
            0x7B => {
                self.regs.a = self.regs.e;
                4
            }
            0x7C => {
                self.regs.a = self.regs.h;
                4
            }
            0x7D => {
                self.regs.a = self.regs.l;
                4
            }
            0x7E => {
                self.regs.a = mmu.read(self.regs.hl());
                8
            }
            0x7F => 4, // LD A, A

            // --- HALT ---
            0x76 => {
                self.halted = true;
                // HALT bug: IME=0 but an interrupt is pending
                if !self.ime && (mmu.read(0xFF0F) & mmu.read(0xFFFF) & 0x1F) != 0 {
                    self.halted = false;
                    self.halt_bug = true;
                }
                4
            }

            // --- ALU A, r ---
            // ADD A, r
            0x80 => {
                let v = self.regs.b;
                self.alu_add(v);
                4
            }
            0x81 => {
                let v = self.regs.c;
                self.alu_add(v);
                4
            }
            0x82 => {
                let v = self.regs.d;
                self.alu_add(v);
                4
            }
            0x83 => {
                let v = self.regs.e;
                self.alu_add(v);
                4
            }
            0x84 => {
                let v = self.regs.h;
                self.alu_add(v);
                4
            }
            0x85 => {
                let v = self.regs.l;
                self.alu_add(v);
                4
            }
            0x86 => {
                let v = mmu.read(self.regs.hl());
                self.alu_add(v);
                8
            }
            0x87 => {
                let v = self.regs.a;
                self.alu_add(v);
                4
            }
            // ADC A, r
            0x88 => {
                let v = self.regs.b;
                self.alu_adc(v);
                4
            }
            0x89 => {
                let v = self.regs.c;
                self.alu_adc(v);
                4
            }
            0x8A => {
                let v = self.regs.d;
                self.alu_adc(v);
                4
            }
            0x8B => {
                let v = self.regs.e;
                self.alu_adc(v);
                4
            }
            0x8C => {
                let v = self.regs.h;
                self.alu_adc(v);
                4
            }
            0x8D => {
                let v = self.regs.l;
                self.alu_adc(v);
                4
            }
            0x8E => {
                let v = mmu.read(self.regs.hl());
                self.alu_adc(v);
                8
            }
            0x8F => {
                let v = self.regs.a;
                self.alu_adc(v);
                4
            }
            // SUB A, r
            0x90 => {
                let v = self.regs.b;
                self.alu_sub(v);
                4
            }
            0x91 => {
                let v = self.regs.c;
                self.alu_sub(v);
                4
            }
            0x92 => {
                let v = self.regs.d;
                self.alu_sub(v);
                4
            }
            0x93 => {
                let v = self.regs.e;
                self.alu_sub(v);
                4
            }
            0x94 => {
                let v = self.regs.h;
                self.alu_sub(v);
                4
            }
            0x95 => {
                let v = self.regs.l;
                self.alu_sub(v);
                4
            }
            0x96 => {
                let v = mmu.read(self.regs.hl());
                self.alu_sub(v);
                8
            }
            0x97 => {
                let v = self.regs.a;
                self.alu_sub(v);
                4
            }
            // SBC A, r
            0x98 => {
                let v = self.regs.b;
                self.alu_sbc(v);
                4
            }
            0x99 => {
                let v = self.regs.c;
                self.alu_sbc(v);
                4
            }
            0x9A => {
                let v = self.regs.d;
                self.alu_sbc(v);
                4
            }
            0x9B => {
                let v = self.regs.e;
                self.alu_sbc(v);
                4
            }
            0x9C => {
                let v = self.regs.h;
                self.alu_sbc(v);
                4
            }
            0x9D => {
                let v = self.regs.l;
                self.alu_sbc(v);
                4
            }
            0x9E => {
                let v = mmu.read(self.regs.hl());
                self.alu_sbc(v);
                8
            }
            0x9F => {
                let v = self.regs.a;
                self.alu_sbc(v);
                4
            }
            // AND A, r
            0xA0 => {
                let v = self.regs.b;
                self.alu_and(v);
                4
            }
            0xA1 => {
                let v = self.regs.c;
                self.alu_and(v);
                4
            }
            0xA2 => {
                let v = self.regs.d;
                self.alu_and(v);
                4
            }
            0xA3 => {
                let v = self.regs.e;
                self.alu_and(v);
                4
            }
            0xA4 => {
                let v = self.regs.h;
                self.alu_and(v);
                4
            }
            0xA5 => {
                let v = self.regs.l;
                self.alu_and(v);
                4
            }
            0xA6 => {
                let v = mmu.read(self.regs.hl());
                self.alu_and(v);
                8
            }
            0xA7 => {
                let v = self.regs.a;
                self.alu_and(v);
                4
            }
            // XOR A, r
            0xA8 => {
                let v = self.regs.b;
                self.alu_xor(v);
                4
            }
            0xA9 => {
                let v = self.regs.c;
                self.alu_xor(v);
                4
            }
            0xAA => {
                let v = self.regs.d;
                self.alu_xor(v);
                4
            }
            0xAB => {
                let v = self.regs.e;
                self.alu_xor(v);
                4
            }
            0xAC => {
                let v = self.regs.h;
                self.alu_xor(v);
                4
            }
            0xAD => {
                let v = self.regs.l;
                self.alu_xor(v);
                4
            }
            0xAE => {
                let v = mmu.read(self.regs.hl());
                self.alu_xor(v);
                8
            }
            0xAF => {
                let v = self.regs.a;
                self.alu_xor(v);
                4
            }
            // OR A, r
            0xB0 => {
                let v = self.regs.b;
                self.alu_or(v);
                4
            }
            0xB1 => {
                let v = self.regs.c;
                self.alu_or(v);
                4
            }
            0xB2 => {
                let v = self.regs.d;
                self.alu_or(v);
                4
            }
            0xB3 => {
                let v = self.regs.e;
                self.alu_or(v);
                4
            }
            0xB4 => {
                let v = self.regs.h;
                self.alu_or(v);
                4
            }
            0xB5 => {
                let v = self.regs.l;
                self.alu_or(v);
                4
            }
            0xB6 => {
                let v = mmu.read(self.regs.hl());
                self.alu_or(v);
                8
            }
            0xB7 => {
                let v = self.regs.a;
                self.alu_or(v);
                4
            }
            // CP A, r
            0xB8 => {
                let v = self.regs.b;
                self.alu_cp(v);
                4
            }
            0xB9 => {
                let v = self.regs.c;
                self.alu_cp(v);
                4
            }
            0xBA => {
                let v = self.regs.d;
                self.alu_cp(v);
                4
            }
            0xBB => {
                let v = self.regs.e;
                self.alu_cp(v);
                4
            }
            0xBC => {
                let v = self.regs.h;
                self.alu_cp(v);
                4
            }
            0xBD => {
                let v = self.regs.l;
                self.alu_cp(v);
                4
            }
            0xBE => {
                let v = mmu.read(self.regs.hl());
                self.alu_cp(v);
                8
            }
            0xBF => {
                let v = self.regs.a;
                self.alu_cp(v);
                4
            }

            // --- ALU A, n (immediate) ---
            0xC6 => {
                let v = self.fetch_byte(mmu);
                self.alu_add(v);
                8
            }
            0xCE => {
                let v = self.fetch_byte(mmu);
                self.alu_adc(v);
                8
            }
            0xD6 => {
                let v = self.fetch_byte(mmu);
                self.alu_sub(v);
                8
            }
            0xDE => {
                let v = self.fetch_byte(mmu);
                self.alu_sbc(v);
                8
            }
            0xE6 => {
                let v = self.fetch_byte(mmu);
                self.alu_and(v);
                8
            }
            0xEE => {
                let v = self.fetch_byte(mmu);
                self.alu_xor(v);
                8
            }
            0xF6 => {
                let v = self.fetch_byte(mmu);
                self.alu_or(v);
                8
            }
            0xFE => {
                let v = self.fetch_byte(mmu);
                self.alu_cp(v);
                8
            }

            // --- JP nn ---
            0xC3 => {
                self.regs.pc = self.fetch_word(mmu);
                16
            }

            // --- JP cc, nn ---
            0xC2 => {
                let addr = self.fetch_word(mmu);
                if !self.regs.flag_z() {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xCA => {
                let addr = self.fetch_word(mmu);
                if self.regs.flag_z() {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xD2 => {
                let addr = self.fetch_word(mmu);
                if !self.regs.flag_c() {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }
            0xDA => {
                let addr = self.fetch_word(mmu);
                if self.regs.flag_c() {
                    self.regs.pc = addr;
                    16
                } else {
                    12
                }
            }

            // --- JP (HL) ---
            0xE9 => {
                self.regs.pc = self.regs.hl();
                4
            }

            // --- JR n ---
            0x18 => {
                let offset = self.fetch_byte(mmu) as i8;
                self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                12
            }

            // --- JR cc, n ---
            0x20 => {
                let o = self.fetch_byte(mmu) as i8;
                if !self.regs.flag_z() {
                    self.regs.pc = self.regs.pc.wrapping_add(o as u16);
                    12
                } else {
                    8
                }
            }
            0x28 => {
                let o = self.fetch_byte(mmu) as i8;
                if self.regs.flag_z() {
                    self.regs.pc = self.regs.pc.wrapping_add(o as u16);
                    12
                } else {
                    8
                }
            }
            0x30 => {
                let o = self.fetch_byte(mmu) as i8;
                if !self.regs.flag_c() {
                    self.regs.pc = self.regs.pc.wrapping_add(o as u16);
                    12
                } else {
                    8
                }
            }
            0x38 => {
                let o = self.fetch_byte(mmu) as i8;
                if self.regs.flag_c() {
                    self.regs.pc = self.regs.pc.wrapping_add(o as u16);
                    12
                } else {
                    8
                }
            }

            // --- CALL nn ---
            0xCD => {
                let addr = self.fetch_word(mmu);
                self.push(mmu, self.regs.pc);
                self.regs.pc = addr;
                24
            }

            // --- CALL cc, nn ---
            0xC4 => {
                let addr = self.fetch_word(mmu);
                if !self.regs.flag_z() {
                    self.push(mmu, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xCC => {
                let addr = self.fetch_word(mmu);
                if self.regs.flag_z() {
                    self.push(mmu, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xD4 => {
                let addr = self.fetch_word(mmu);
                if !self.regs.flag_c() {
                    self.push(mmu, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }
            0xDC => {
                let addr = self.fetch_word(mmu);
                if self.regs.flag_c() {
                    self.push(mmu, self.regs.pc);
                    self.regs.pc = addr;
                    24
                } else {
                    12
                }
            }

            // --- RET ---
            0xC9 => {
                self.regs.pc = self.pop(mmu);
                16
            }

            // --- RET cc ---
            0xC0 => {
                if !self.regs.flag_z() {
                    self.regs.pc = self.pop(mmu);
                    20
                } else {
                    8
                }
            }
            0xC8 => {
                if self.regs.flag_z() {
                    self.regs.pc = self.pop(mmu);
                    20
                } else {
                    8
                }
            }
            0xD0 => {
                if !self.regs.flag_c() {
                    self.regs.pc = self.pop(mmu);
                    20
                } else {
                    8
                }
            }
            0xD8 => {
                if self.regs.flag_c() {
                    self.regs.pc = self.pop(mmu);
                    20
                } else {
                    8
                }
            }

            // --- RETI ---
            0xD9 => {
                self.regs.pc = self.pop(mmu);
                self.ime = true;
                16
            }

            // --- RST vec ---
            0xC7 => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x00;
                16
            }
            0xCF => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x08;
                16
            }
            0xD7 => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x10;
                16
            }
            0xDF => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x18;
                16
            }
            0xE7 => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x20;
                16
            }
            0xEF => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x28;
                16
            }
            0xF7 => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x30;
                16
            }
            0xFF => {
                self.push(mmu, self.regs.pc);
                self.regs.pc = 0x38;
                16
            }

            // --- PUSH rr ---
            0xC5 => {
                let v = self.regs.bc();
                self.push(mmu, v);
                16
            }
            0xD5 => {
                let v = self.regs.de();
                self.push(mmu, v);
                16
            }
            0xE5 => {
                let v = self.regs.hl();
                self.push(mmu, v);
                16
            }
            0xF5 => {
                let v = self.regs.af();
                self.push(mmu, v);
                16
            }

            // --- POP rr ---
            0xC1 => {
                let v = self.pop(mmu);
                self.regs.set_bc(v);
                12
            }
            0xD1 => {
                let v = self.pop(mmu);
                self.regs.set_de(v);
                12
            }
            0xE1 => {
                let v = self.pop(mmu);
                self.regs.set_hl(v);
                12
            }
            0xF1 => {
                let v = self.pop(mmu);
                self.regs.set_af(v); // set_af masks F bits 0-3
                12
            }

            // --- LDH (n), A / LDH A, (n) ---
            0xE0 => {
                let n = self.fetch_byte(mmu) as u16;
                mmu.write(0xFF00 + n, self.regs.a);
                12
            }
            0xF0 => {
                let n = self.fetch_byte(mmu) as u16;
                self.regs.a = mmu.read(0xFF00 + n);
                12
            }

            // --- LD (C), A / LD A, (C) ---
            0xE2 => {
                mmu.write(0xFF00 + self.regs.c as u16, self.regs.a);
                8
            }
            0xF2 => {
                self.regs.a = mmu.read(0xFF00 + self.regs.c as u16);
                8
            }

            // --- LD (nn), A / LD A, (nn) ---
            0xEA => {
                let addr = self.fetch_word(mmu);
                mmu.write(addr, self.regs.a);
                16
            }
            0xFA => {
                let addr = self.fetch_word(mmu);
                self.regs.a = mmu.read(addr);
                16
            }

            // --- LD SP, HL ---
            0xF9 => {
                self.regs.sp = self.regs.hl();
                8
            }

            // --- LD HL, SP+n ---
            0xF8 => {
                let offset = self.fetch_byte(mmu) as i8 as i16;
                let sp = self.regs.sp;
                let result = sp.wrapping_add(offset as u16);
                self.regs.set_hl(result);
                self.regs.set_flag_z(false);
                self.regs.set_flag_n(false);
                // H and C are computed on the unsigned low-byte addition
                self.regs
                    .set_flag_h((sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F);
                self.regs
                    .set_flag_c((sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF);
                12
            }

            // --- ADD SP, n ---
            0xE8 => {
                let offset = self.fetch_byte(mmu) as i8 as i16;
                let sp = self.regs.sp;
                let result = sp.wrapping_add(offset as u16);
                self.regs.set_flag_z(false);
                self.regs.set_flag_n(false);
                self.regs
                    .set_flag_h((sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F);
                self.regs
                    .set_flag_c((sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF);
                self.regs.sp = result;
                16
            }

            // --- DI / EI ---
            0xF3 => {
                self.ime = false;
                4
            }
            0xFB => {
                self.ime_scheduled = true;
                4
            }

            // --- STOP ---
            0x10 => {
                let _ = self.fetch_byte(mmu);
                4
            }

            // --- CB prefix ---
            0xCB => {
                let cb_op = self.fetch_byte(mmu);
                self.execute_cb(cb_op, mmu)
            }

            // Unused opcodes on DMG behave like a 2-byte NOP that locks up the CPU.
            // Treat them as NOP to keep the emulator running.
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "Illegal opcode: 0x{:02X} at PC=0x{:04X}",
                    opcode,
                    self.regs.pc.wrapping_sub(1)
                );
                4
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Cpu;
    use crate::mmu::Mmu;

    /// Write `opcodes` at HRAM (0xFF80), point PC there, step once.
    /// Returns (cpu, mmu, cycles).
    fn exec(opcodes: &[u8]) -> (Cpu, Mmu, u32) {
        exec_with(opcodes, |_, _| {})
    }

    /// Like `exec` but accepts a setup closure run before `step()`.
    fn exec_with<F>(opcodes: &[u8], setup: F) -> (Cpu, Mmu, u32)
    where
        F: FnOnce(&mut Cpu, &mut Mmu),
    {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        for (i, &b) in opcodes.iter().enumerate() {
            mmu.write(0xFF80 + i as u16, b);
        }
        cpu.regs.pc = 0xFF80;
        setup(&mut cpu, &mut mmu);
        let cycles = cpu.step(&mut mmu);
        (cpu, mmu, cycles)
    }

    // -------------------------------------------------------------------------
    // NOP
    // -------------------------------------------------------------------------

    #[test]
    fn nop_4_cycles() {
        let (_, _, cycles) = exec(&[0x00]);
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // LD rr, nn
    // -------------------------------------------------------------------------

    #[test]
    fn ld_bc_nn() {
        // 0x01 lo hi — load 0x1234 into BC
        let (cpu, _, cycles) = exec(&[0x01, 0x34, 0x12]);
        assert_eq!(cpu.regs.bc(), 0x1234);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn ld_sp_nn() {
        // 0x31 lo hi — load 0xABCD into SP
        let (cpu, _, cycles) = exec(&[0x31, 0xCD, 0xAB]);
        assert_eq!(cpu.regs.sp, 0xABCD);
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // LD (rr), A  /  LD A, (rr)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_bc_indirect_a() {
        // 0x02 — write A to (BC). Point BC at WRAM so the write sticks.
        let (_, mmu, cycles) = exec_with(&[0x02], |cpu, _| {
            cpu.regs.set_bc(0xC100);
            cpu.regs.a = 0x42;
        });
        assert_eq!(mmu.read(0xC100), 0x42);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_a_de_indirect() {
        // 0x1A — read (DE) into A. Point DE at WRAM so we can pre-load a value.
        let (cpu, _, cycles) = exec_with(&[0x1A], |cpu, mmu| {
            cpu.regs.set_de(0xC200);
            mmu.write(0xC200, 0x55);
        });
        assert_eq!(cpu.regs.a, 0x55);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // LD (HL±), A
    // -------------------------------------------------------------------------

    #[test]
    fn ld_hli_a() {
        // 0x22 — write A to (HL), then HL++. HL points to WRAM.
        let (cpu, mmu, cycles) = exec_with(&[0x22], |cpu, _| {
            cpu.regs.set_hl(0xC300);
            cpu.regs.a = 0x7F;
        });
        assert_eq!(mmu.read(0xC300), 0x7F);
        assert_eq!(cpu.regs.hl(), 0xC301);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_hld_a() {
        // 0x32 — write A to (HL), then HL--. HL points to WRAM.
        let (cpu, mmu, cycles) = exec_with(&[0x32], |cpu, _| {
            cpu.regs.set_hl(0xC300);
            cpu.regs.a = 0x7F;
        });
        assert_eq!(mmu.read(0xC300), 0x7F);
        assert_eq!(cpu.regs.hl(), 0xC2FF);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // INC / DEC rr
    // -------------------------------------------------------------------------

    #[test]
    fn inc_bc() {
        // 0x03 — BC=0x0013 initially → 0x0014. No flags changed.
        let (cpu, _, cycles) = exec(&[0x03]);
        assert_eq!(cpu.regs.bc(), 0x0014);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn dec_hl() {
        // 0x2B — HL=0x014D initially → 0x014C. No flags changed.
        let (cpu, _, cycles) = exec(&[0x2B]);
        assert_eq!(cpu.regs.hl(), 0x014C);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // INC / DEC r
    // -------------------------------------------------------------------------

    #[test]
    fn inc_b_half_carry() {
        // 0x04 — B=0x0F → 0x10. H flag set, Z clear, N clear.
        let (cpu, _, cycles) = exec_with(&[0x04], |cpu, _| {
            cpu.regs.b = 0x0F;
        });
        assert_eq!(cpu.regs.b, 0x10);
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_c_to_zero() {
        // 0x0D — C=0x01 → 0x00. Z set, N set.
        let (cpu, _, cycles) = exec_with(&[0x0D], |cpu, _| {
            cpu.regs.c = 0x01;
        });
        assert_eq!(cpu.regs.c, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // LD r, n
    // -------------------------------------------------------------------------

    #[test]
    fn ld_b_imm() {
        // 0x06 0x42 — B ← 0x42. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x06, 0x42]);
        assert_eq!(cpu.regs.b, 0x42);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // Rotate A
    // -------------------------------------------------------------------------

    #[test]
    fn opcode_rlca() {
        // 0x07 — A=0x01. Rotated left: 0x02. Carry=0. Z/N/H all clear.
        let (cpu, _, cycles) = exec_with(&[0x07], |cpu, _| {
            cpu.regs.a = 0x85;
        });
        // 0x85 = 1000_0101 → rotated left circular → 0000_1011 = 0x0B, carry=1
        assert_eq!(cpu.regs.a, 0x0B);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn opcode_rrca() {
        // 0x0F — A=0x01: bit0→carry, result=0x80, carry=1.
        let (cpu, _, cycles) = exec_with(&[0x0F], |cpu, _| {
            cpu.regs.a = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x80);
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn opcode_rla() {
        // 0x17 — A=0x80, carry=0. Rotate left through carry: result=0x00, carry=1.
        let (cpu, _, cycles) = exec_with(&[0x17], |cpu, _| {
            cpu.regs.a = 0x80;
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert!(cpu.regs.flag_c()); // old bit7 was 1
        assert!(!cpu.regs.flag_z()); // Z always cleared by RLA
        assert_eq!(cycles, 4);
    }

    #[test]
    fn opcode_rra() {
        // 0x1F — A=0x01, carry=1. Rotate right through carry: result=0x80, carry=1.
        let (cpu, _, cycles) = exec_with(&[0x1F], |cpu, _| {
            cpu.regs.a = 0x01;
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.a, 0x80);
        assert!(cpu.regs.flag_c()); // old bit0 was 1
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // DAA
    // -------------------------------------------------------------------------

    #[test]
    fn daa_after_add() {
        // A=0x11, N=0, H=0, C=0 — both nibbles < 10, no correction needed.
        let (cpu, _, cycles) = exec_with(&[0x27], |cpu, _| {
            cpu.regs.a = 0x11;
            cpu.regs.set_flag_n(false);
            cpu.regs.set_flag_h(false);
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.a, 0x11);
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn daa_bcd_correction() {
        // A=0x0A, N=0, H=0, C=0 — low nibble > 9, DAA adds 0x06 → 0x10.
        let (cpu, _, cycles) = exec_with(&[0x27], |cpu, _| {
            cpu.regs.a = 0x0A;
            cpu.regs.set_flag_n(false);
            cpu.regs.set_flag_h(false);
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.a, 0x10);
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // CPL
    // -------------------------------------------------------------------------

    #[test]
    fn cpl_complements() {
        // 0x2F — A=0xF0 → ~0xF0 = 0x0F. N and H set.
        let (cpu, _, cycles) = exec_with(&[0x2F], |cpu, _| {
            cpu.regs.a = 0xF0;
        });
        assert_eq!(cpu.regs.a, 0x0F);
        assert!(cpu.regs.flag_n());
        assert!(cpu.regs.flag_h());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // SCF / CCF
    // -------------------------------------------------------------------------

    #[test]
    fn scf() {
        // 0x37 — sets C, clears N and H.
        let (cpu, _, cycles) = exec_with(&[0x37], |cpu, _| {
            cpu.regs.set_flag_c(false);
        });
        assert!(cpu.regs.flag_c());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn ccf() {
        // 0x3F — complements C. Initial C set → C clear after.
        let (cpu, _, cycles) = exec_with(&[0x3F], |cpu, _| {
            cpu.regs.set_flag_c(true);
        });
        assert!(!cpu.regs.flag_c());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // LD r, r'
    // -------------------------------------------------------------------------

    #[test]
    fn ld_b_c() {
        // 0x41 — B ← C. Initial C=0x13.
        let (cpu, _, cycles) = exec(&[0x41]);
        assert_eq!(cpu.regs.b, 0x13);
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // HALT
    // -------------------------------------------------------------------------

    #[test]
    fn halt_sets_flag() {
        // 0x76 — sets cpu.halted. IME=0, no pending interrupts → no halt bug.
        let (cpu, _, cycles) = exec(&[0x76]);
        assert!(cpu.halted);
        assert!(!cpu.halt_bug);
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // ALU A, r
    // -------------------------------------------------------------------------

    #[test]
    fn add_a_b() {
        // 0x80 — A += B. A=0x0F, B=0x01 → A=0x10, H set.
        let (cpu, _, cycles) = exec_with(&[0x80], |cpu, _| {
            cpu.regs.a = 0x0F;
            cpu.regs.b = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x10);
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn sub_a_c() {
        // 0x91 — A -= C. A=0x10, C=0x01 → A=0x0F, H set (0 < 1), N set.
        let (cpu, _, cycles) = exec_with(&[0x91], |cpu, _| {
            cpu.regs.a = 0x10;
            cpu.regs.c = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x0F);
        assert!(cpu.regs.flag_n());
        assert!(cpu.regs.flag_h()); // low nibble borrow: 0x0 < 0x1
        assert!(!cpu.regs.flag_c());
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn and_a_d() {
        // 0xA2 — A &= D. A=0xFF, D=0x0F → A=0x0F. H set, Z clear.
        let (cpu, _, cycles) = exec_with(&[0xA2], |cpu, _| {
            cpu.regs.a = 0xFF;
            cpu.regs.d = 0x0F;
        });
        assert_eq!(cpu.regs.a, 0x0F);
        assert!(cpu.regs.flag_h());
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn xor_a_e() {
        // 0xAB — A ^= E. A=0xFF, E=0x0F → A=0xF0. Z clear.
        let (cpu, _, cycles) = exec_with(&[0xAB], |cpu, _| {
            cpu.regs.a = 0xFF;
            cpu.regs.e = 0x0F;
        });
        assert_eq!(cpu.regs.a, 0xF0);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn or_a_h() {
        // 0xB4 — A |= H. A=0xF0, H=0x0F → A=0xFF. Z clear.
        let (cpu, _, cycles) = exec_with(&[0xB4], |cpu, _| {
            cpu.regs.a = 0xF0;
            cpu.regs.h = 0x0F;
        });
        assert_eq!(cpu.regs.a, 0xFF);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn cp_a_l() {
        // 0xBD — CP A, L. A=0x10, L=0x10 → Z set, N set, A unchanged.
        let (cpu, _, cycles) = exec_with(&[0xBD], |cpu, _| {
            cpu.regs.a = 0x10;
            cpu.regs.l = 0x10;
        });
        assert_eq!(cpu.regs.a, 0x10); // A must not change
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn adc_a_b() {
        // 0x88 — A = A + B + carry. A=0x01, B=0x02, carry=1 → A=0x04.
        let (cpu, _, cycles) = exec_with(&[0x88], |cpu, _| {
            cpu.regs.a = 0x01;
            cpu.regs.b = 0x02;
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.a, 0x04);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn sbc_a_b() {
        // 0x98 — A = A - B - carry. A=0x05, B=0x02, carry=1 → A=0x02.
        let (cpu, _, cycles) = exec_with(&[0x98], |cpu, _| {
            cpu.regs.a = 0x05;
            cpu.regs.b = 0x02;
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.a, 0x02);
        assert!(!cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // ALU immediate
    // -------------------------------------------------------------------------

    #[test]
    fn add_a_imm() {
        // 0xC6 0x10 — A += 0x10. A=0x01 → A=0x11. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xC6, 0x10], |cpu, _| {
            cpu.regs.a = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x11);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn sub_a_imm() {
        // 0xD6 0x01 — A -= 0x01. A=0x01 → A=0x00, Z set. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xD6, 0x01], |cpu, _| {
            cpu.regs.a = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn and_a_imm() {
        // 0xE6 0xAA — A &= 0xAA. A=0xFF → A=0xAA. H set. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xE6, 0xAA], |cpu, _| {
            cpu.regs.a = 0xFF;
        });
        assert_eq!(cpu.regs.a, 0xAA);
        assert!(!cpu.regs.flag_z());
        assert!(cpu.regs.flag_h());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn xor_a_imm() {
        // 0xEE 0xFF — A ^= 0xFF. A=0xFF → A=0x00, Z set. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xEE, 0xFF], |cpu, _| {
            cpu.regs.a = 0xFF;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert!(cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn or_a_imm() {
        // 0xF6 0x80 — A |= 0x80. A=0x01 → A=0x81. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xF6, 0x80], |cpu, _| {
            cpu.regs.a = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x81);
        assert!(!cpu.regs.flag_z());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn cp_a_imm() {
        // 0xFE 0x42 — CP A, 0x42. A=0x42 → Z set, N set, A unchanged. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xFE, 0x42], |cpu, _| {
            cpu.regs.a = 0x42;
        });
        assert_eq!(cpu.regs.a, 0x42);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // PUSH / POP
    // -------------------------------------------------------------------------

    #[test]
    fn push_pop_bc() {
        let mut cpu = Cpu::new();
        let mut mmu = Mmu::new();
        // Write PUSH BC at 0xFF80, POP BC at 0xFF81.
        mmu.write(0xFF80, 0xC5); // PUSH BC
        mmu.write(0xFF81, 0xC1); // POP BC
        cpu.regs.pc = 0xFF80;
        // Set BC to a known value.
        cpu.regs.set_bc(0xBEEF);
        let sp_before = cpu.regs.sp;

        let cycles_push = cpu.step(&mut mmu);
        assert_eq!(cycles_push, 16);
        assert_eq!(cpu.regs.sp, sp_before.wrapping_sub(2));

        // Clobber BC to verify POP restores it.
        cpu.regs.set_bc(0x0000);
        let cycles_pop = cpu.step(&mut mmu);
        assert_eq!(cycles_pop, 12);
        assert_eq!(cpu.regs.bc(), 0xBEEF);
        assert_eq!(cpu.regs.sp, sp_before);
    }

    // -------------------------------------------------------------------------
    // JP
    // -------------------------------------------------------------------------

    #[test]
    fn jp_nn() {
        // 0xC3 lo hi — unconditional jump. 16 cycles.
        let (cpu, _, cycles) = exec(&[0xC3, 0x00, 0x20]);
        assert_eq!(cpu.regs.pc, 0x2000);
        assert_eq!(cycles, 16);
    }

    #[test]
    fn jp_nz_taken() {
        // 0xC2 lo hi — jump if Z clear. Clear Z first.
        let (cpu, _, cycles) = exec_with(&[0xC2, 0x00, 0x30], |cpu, _| {
            cpu.regs.set_flag_z(false);
        });
        assert_eq!(cpu.regs.pc, 0x3000);
        assert_eq!(cycles, 16);
    }

    #[test]
    fn jp_nz_not_taken() {
        // 0xC2 lo hi — no jump if Z set. Z is set in initial state (f=0xB0).
        let (cpu, _, cycles) = exec_with(&[0xC2, 0x00, 0x30], |cpu, _| {
            cpu.regs.set_flag_z(true);
        });
        // PC advanced past the 3-byte instruction (0xFF80 + 3 = 0xFF83).
        assert_eq!(cpu.regs.pc, 0xFF83);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn jp_hl() {
        // 0xE9 — jump to HL. Initial HL=0x014D. 4 cycles.
        let (cpu, _, cycles) = exec_with(&[0xE9], |cpu, _| {
            cpu.regs.set_hl(0x1234);
        });
        assert_eq!(cpu.regs.pc, 0x1234);
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // JR
    // -------------------------------------------------------------------------

    #[test]
    fn jr_forward() {
        // 0x18 0x05 — PC after fetch = 0xFF82; add 5 → 0xFF87. 12 cycles.
        let (cpu, _, cycles) = exec(&[0x18, 0x05]);
        assert_eq!(cpu.regs.pc, 0xFF87);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn jr_c_taken() {
        // 0x38 offset — jump if C set. Initial C is set (f=0xB0).
        let (cpu, _, cycles) = exec_with(&[0x38, 0x04], |cpu, _| {
            cpu.regs.set_flag_c(true);
        });
        // PC after fetch = 0xFF82; add 4 → 0xFF86.
        assert_eq!(cpu.regs.pc, 0xFF86);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn jr_c_not_taken() {
        // 0x38 offset — no jump if C clear.
        let (cpu, _, cycles) = exec_with(&[0x38, 0x04], |cpu, _| {
            cpu.regs.set_flag_c(false);
        });
        // PC advances past the 2-byte instruction only.
        assert_eq!(cpu.regs.pc, 0xFF82);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // CALL / RET / RETI
    // -------------------------------------------------------------------------

    #[test]
    fn call_nn() {
        // 0xCD lo hi — push return addr, jump to target. 24 cycles.
        // Opcode at 0xFF80..0xFF82; return addr = 0xFF83.
        let (cpu, mmu, cycles) = exec_with(&[0xCD, 0x00, 0x40], |cpu, _| {
            cpu.regs.sp = 0xFFFE;
        });
        assert_eq!(cpu.regs.pc, 0x4000);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        // Return address on stack: 0xFF83 (low byte at SP, high byte at SP+1).
        assert_eq!(mmu.read(0xFFFC), 0x83);
        assert_eq!(mmu.read(0xFFFD), 0xFF);
        assert_eq!(cycles, 24);
    }

    #[test]
    fn ret() {
        // 0xC9 — pop PC from stack. Pre-push 0x1234. 16 cycles.
        let (cpu, _, cycles) = exec_with(&[0xC9], |cpu, mmu| {
            cpu.regs.sp = 0xFFFC;
            mmu.write(0xFFFC, 0x34); // low byte
            mmu.write(0xFFFD, 0x12); // high byte
        });
        assert_eq!(cpu.regs.pc, 0x1234);
        assert_eq!(cpu.regs.sp, 0xFFFE);
        assert_eq!(cycles, 16);
    }

    #[test]
    fn reti() {
        // 0xD9 — pop PC and enable IME immediately. 16 cycles.
        let (cpu, _, cycles) = exec_with(&[0xD9], |cpu, mmu| {
            cpu.regs.sp = 0xFFFC;
            mmu.write(0xFFFC, 0x56);
            mmu.write(0xFFFD, 0x12);
        });
        assert_eq!(cpu.regs.pc, 0x1256);
        assert!(cpu.ime);
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // RST
    // -------------------------------------------------------------------------

    #[test]
    fn rst_00() {
        // 0xC7 — push PC (0xFF81), jump to 0x0000. 16 cycles.
        let (cpu, mmu, cycles) = exec_with(&[0xC7], |cpu, _| {
            cpu.regs.sp = 0xFFFE;
        });
        assert_eq!(cpu.regs.pc, 0x0000);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        assert_eq!(mmu.read(0xFFFC), 0x81); // return addr low
        assert_eq!(mmu.read(0xFFFD), 0xFF); // return addr high
        assert_eq!(cycles, 16);
    }

    #[test]
    fn rst_38() {
        // 0xFF — push PC (0xFF81), jump to 0x0038. 16 cycles.
        let (cpu, _, cycles) = exec_with(&[0xFF], |cpu, _| {
            cpu.regs.sp = 0xFFFE;
        });
        assert_eq!(cpu.regs.pc, 0x0038);
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // ADD SP, e
    // -------------------------------------------------------------------------

    #[test]
    fn add_sp_positive() {
        // 0xE8 0x05 — SP += 5. SP=0xFF00 → 0xFF05.
        // H: (0x00 & 0x0F) + (5 & 0x0F) = 5 ≤ 15 → H=0.
        // C: (0x00 & 0xFF) + (5 & 0xFF) = 5 ≤ 255 → C=0.
        let (cpu, _, cycles) = exec_with(&[0xE8, 0x05], |cpu, _| {
            cpu.regs.sp = 0xFF00;
        });
        assert_eq!(cpu.regs.sp, 0xFF05);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 16);
    }

    #[test]
    fn add_sp_negative() {
        // 0xE8 0xFB — SP += -5 (0xFB as i8 = -5). SP=0xFF10 → 0xFF0B.
        // As unsigned low-byte calc: 0x10 + 0xFB = 0x10B > 0xFF → C=1.
        // Low nibble: 0x0 + 0xB = 0xB ≤ 0xF → H=0.
        let (cpu, _, cycles) = exec_with(&[0xE8, 0xFB], |cpu, _| {
            cpu.regs.sp = 0xFF10;
        });
        assert_eq!(cpu.regs.sp, 0xFF0B);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(cpu.regs.flag_c());
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // LD HL, SP+e
    // -------------------------------------------------------------------------

    #[test]
    fn ld_hl_sp_e() {
        // 0xF8 0x10 — HL = SP + 16. SP=0x0010 → HL=0x0020.
        // C: (0x10 & 0xFF) + (0x10 & 0xFF) = 32 ≤ 255 → C=0.
        // H: (0x10 & 0x0F) + (0x10 & 0x0F) = 0 ≤ 15 → H=0.
        let (cpu, _, cycles) = exec_with(&[0xF8, 0x10], |cpu, _| {
            cpu.regs.sp = 0x0010;
        });
        assert_eq!(cpu.regs.hl(), 0x0020);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_h());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // LDH
    // -------------------------------------------------------------------------

    #[test]
    fn ldh_n_a() {
        // 0xE0 0x01 — write A to 0xFF01 (serial data register). A=0xAB. 12 cycles.
        let (_, mmu, cycles) = exec_with(&[0xE0, 0x01], |cpu, _| {
            cpu.regs.a = 0xAB;
        });
        assert_eq!(mmu.read(0xFF01), 0xAB);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn ldh_a_n() {
        // 0xF0 0x01 — read 0xFF01 (serial data) into A. 12 cycles.
        let (cpu, _, cycles) = exec_with(&[0xF0, 0x01], |_, mmu| {
            mmu.write(0xFF01, 0x77);
        });
        assert_eq!(cpu.regs.a, 0x77);
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // LD (C), A  /  LD A, (C)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_ffc_a() {
        // 0xE2 — write A to 0xFF00+C. Use C=0x01 → 0xFF01 (serial data). A=0x55. 8 cycles.
        let (_, mmu, cycles) = exec_with(&[0xE2], |cpu, _| {
            cpu.regs.c = 0x01;
            cpu.regs.a = 0x55;
        });
        assert_eq!(mmu.read(0xFF01), 0x55);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_a_ffc() {
        // 0xF2 — read 0xFF00+C into A. C=0x01 → 0xFF01. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xF2], |cpu, mmu| {
            cpu.regs.c = 0x01;
            mmu.write(0xFF01, 0x33);
        });
        assert_eq!(cpu.regs.a, 0x33);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // LD (nn), A
    // -------------------------------------------------------------------------

    #[test]
    fn ld_nn_a() {
        // 0xEA lo hi — write A to address. A=0xBB, addr=0xC000. 16 cycles.
        let (_, mmu, cycles) = exec_with(&[0xEA, 0x00, 0xC0], |cpu, _| {
            cpu.regs.a = 0xBB;
        });
        assert_eq!(mmu.read(0xC000), 0xBB);
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // EI / DI
    // -------------------------------------------------------------------------

    #[test]
    fn ei_schedules() {
        // 0xFB — EI sets ime_scheduled; ime becomes true on the *next* step.
        let (cpu, _, cycles) = exec(&[0xFB]);
        // After this step, ime_scheduled was set then consumed... wait:
        // step() checks ime_scheduled at TOP (before executing opcode).
        // 0xFB opcode sets ime_scheduled = true.
        // So after step: ime_scheduled=true, ime=false.
        assert!(cpu.ime_scheduled);
        assert!(!cpu.ime);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn di_clears() {
        // 0xF3 — DI clears ime immediately.
        let (cpu, _, cycles) = exec_with(&[0xF3], |cpu, _| {
            cpu.ime = true;
        });
        assert!(!cpu.ime);
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // LD rr, nn (DE and HL)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_de_nn() {
        // 0x11 lo hi — load 0x5678 into DE. 12 cycles.
        let (cpu, _, cycles) = exec(&[0x11, 0x78, 0x56]);
        assert_eq!(cpu.regs.de(), 0x5678);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn ld_hl_nn() {
        // 0x21 lo hi — load 0xABCD into HL. 12 cycles.
        let (cpu, _, cycles) = exec(&[0x21, 0xCD, 0xAB]);
        assert_eq!(cpu.regs.hl(), 0xABCD);
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // LD A, (HL+/-)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_a_hli() {
        // 0x2A — A = (HL), HL increments. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0x2A], |cpu, mmu| {
            cpu.regs.set_hl(0xC100);
            mmu.write(0xC100, 0xAB);
        });
        assert_eq!(cpu.regs.a, 0xAB);
        assert_eq!(cpu.regs.hl(), 0xC101);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_a_hld() {
        // 0x3A — A = (HL), HL decrements. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0x3A], |cpu, mmu| {
            cpu.regs.set_hl(0xC200);
            mmu.write(0xC200, 0x77);
        });
        assert_eq!(cpu.regs.a, 0x77);
        assert_eq!(cpu.regs.hl(), 0xC1FF);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // LD (nn), SP
    // -------------------------------------------------------------------------

    #[test]
    fn ld_nn_sp() {
        // 0x08 lo hi — write SP to (nn) as 16-bit LE. SP=0x1234 → mem[addr]=0x34, mem[addr+1]=0x12.
        let (_, mmu, cycles) = exec_with(&[0x08, 0x00, 0xC0], |cpu, _| {
            cpu.regs.sp = 0x1234;
        });
        assert_eq!(mmu.read(0xC000), 0x34); // low byte
        assert_eq!(mmu.read(0xC001), 0x12); // high byte
        assert_eq!(cycles, 20);
    }

    // -------------------------------------------------------------------------
    // INC rr (DE, HL, SP)
    // -------------------------------------------------------------------------

    #[test]
    fn inc_de() {
        // 0x13 — DE=0x00D8 initially → 0x00D9. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x13]);
        assert_eq!(cpu.regs.de(), 0x00D9);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn inc_hl() {
        // 0x23 — HL=0x014D initially → 0x014E. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x23]);
        assert_eq!(cpu.regs.hl(), 0x014E);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn inc_sp() {
        // 0x33 — SP=0xFFFE initially → 0xFFFF. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x33]);
        assert_eq!(cpu.regs.sp, 0xFFFF);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // DEC rr (BC, DE, SP)
    // -------------------------------------------------------------------------

    #[test]
    fn dec_bc() {
        // 0x0B — BC=0x0013 initially → 0x0012. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x0B]);
        assert_eq!(cpu.regs.bc(), 0x0012);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn dec_de() {
        // 0x1B — DE=0x00D8 initially → 0x00D7. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x1B]);
        assert_eq!(cpu.regs.de(), 0x00D7);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn dec_sp() {
        // 0x3B — SP=0xFFFE initially → 0xFFFD. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x3B]);
        assert_eq!(cpu.regs.sp, 0xFFFD);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // INC r (C, D, E, H, L, A)
    // -------------------------------------------------------------------------

    #[test]
    fn inc_c() {
        // 0x0C — C=0x13 initially → 0x14. N clear. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x0C]);
        assert_eq!(cpu.regs.c, 0x14);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn inc_d() {
        // 0x14 — D=0x00 initially → 0x01. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x14]);
        assert_eq!(cpu.regs.d, 0x01);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn inc_e() {
        // 0x1C — E=0xD8 → 0xD9. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x1C]);
        assert_eq!(cpu.regs.e, 0xD9);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn inc_h() {
        // 0x24 — H=0x01 → 0x02. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x24]);
        assert_eq!(cpu.regs.h, 0x02);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn inc_l() {
        // 0x2C — L=0x4D → 0x4E. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x2C]);
        assert_eq!(cpu.regs.l, 0x4E);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn inc_a() {
        // 0x3C — A=0x01 initially → 0x02. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x3C]);
        assert_eq!(cpu.regs.a, 0x02);
        assert!(!cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // DEC r (B, D, E, H, L, A)
    // -------------------------------------------------------------------------

    #[test]
    fn dec_b() {
        // 0x05 — B=0x00 → 0xFF (wraps). N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x05]);
        assert_eq!(cpu.regs.b, 0xFF);
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_d() {
        // 0x15 — D=0x00 → 0xFF. N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x15]);
        assert_eq!(cpu.regs.d, 0xFF);
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_e() {
        // 0x1D — E=0xD8 → 0xD7. N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x1D]);
        assert_eq!(cpu.regs.e, 0xD7);
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_h() {
        // 0x25 — H=0x01 → 0x00. Z set, N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x25]);
        assert_eq!(cpu.regs.h, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_l() {
        // 0x2D — L=0x4D → 0x4C. N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x2D]);
        assert_eq!(cpu.regs.l, 0x4C);
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    #[test]
    fn dec_a() {
        // 0x3D — A=0x01 → 0x00. Z set, N set. 4 cycles.
        let (cpu, _, cycles) = exec(&[0x3D]);
        assert_eq!(cpu.regs.a, 0x00);
        assert!(cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert_eq!(cycles, 4);
    }

    // -------------------------------------------------------------------------
    // LD r, n (C, D, E, H, L, A)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_c_imm() {
        // 0x0E val — C ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x0E, 0xCC]);
        assert_eq!(cpu.regs.c, 0xCC);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_d_imm() {
        // 0x16 val — D ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x16, 0xDD]);
        assert_eq!(cpu.regs.d, 0xDD);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_e_imm() {
        // 0x1E val — E ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x1E, 0xEE]);
        assert_eq!(cpu.regs.e, 0xEE);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_h_imm() {
        // 0x26 val — H ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x26, 0x11]);
        assert_eq!(cpu.regs.h, 0x11);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_l_imm() {
        // 0x2E val — L ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x2E, 0x22]);
        assert_eq!(cpu.regs.l, 0x22);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn ld_a_imm() {
        // 0x3E val — A ← val. 8 cycles.
        let (cpu, _, cycles) = exec(&[0x3E, 0x99]);
        assert_eq!(cpu.regs.a, 0x99);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // LD SP, HL
    // -------------------------------------------------------------------------

    #[test]
    fn ld_sp_hl() {
        // 0xF9 — SP = HL. HL=0x1234 → SP=0x1234. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xF9], |cpu, _| {
            cpu.regs.set_hl(0x1234);
        });
        assert_eq!(cpu.regs.sp, 0x1234);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // JP cc, nn (Z taken/not-taken, C taken, NC taken)
    // -------------------------------------------------------------------------

    #[test]
    fn jp_z_taken() {
        // 0xCA lo hi — jump if Z set. Set Z flag.
        let (cpu, _, cycles) = exec_with(&[0xCA, 0x00, 0x50], |cpu, _| {
            cpu.regs.set_flag_z(true);
        });
        assert_eq!(cpu.regs.pc, 0x5000);
        assert_eq!(cycles, 16);
    }

    #[test]
    fn jp_z_not_taken() {
        // 0xCA lo hi — no jump if Z clear. PC advances 3 bytes.
        let (cpu, _, cycles) = exec_with(&[0xCA, 0x00, 0x50], |cpu, _| {
            cpu.regs.set_flag_z(false);
        });
        assert_eq!(cpu.regs.pc, 0xFF83);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn jp_c_taken() {
        // 0xDA lo hi — jump if C set. Set C flag.
        let (cpu, _, cycles) = exec_with(&[0xDA, 0x00, 0x60], |cpu, _| {
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.pc, 0x6000);
        assert_eq!(cycles, 16);
    }

    #[test]
    fn jp_nc_taken() {
        // 0xD2 lo hi — jump if C clear. Clear C flag.
        let (cpu, _, cycles) = exec_with(&[0xD2, 0x00, 0x70], |cpu, _| {
            cpu.regs.set_flag_c(false);
        });
        assert_eq!(cpu.regs.pc, 0x7000);
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // JR cc (NZ taken, Z taken)
    // -------------------------------------------------------------------------

    #[test]
    fn jr_nz_taken() {
        // 0x20 offset — jump if Z clear. PC after fetch = 0xFF82; add 3 → 0xFF85.
        let (cpu, _, cycles) = exec_with(&[0x20, 0x03], |cpu, _| {
            cpu.regs.set_flag_z(false);
        });
        assert_eq!(cpu.regs.pc, 0xFF85);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn jr_z_taken() {
        // 0x28 offset — jump if Z set. PC after fetch = 0xFF82; add 5 → 0xFF87.
        let (cpu, _, cycles) = exec_with(&[0x28, 0x05], |cpu, _| {
            cpu.regs.set_flag_z(true);
        });
        assert_eq!(cpu.regs.pc, 0xFF87);
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // CALL NZ (taken and not taken)
    // -------------------------------------------------------------------------

    #[test]
    fn call_nz_taken() {
        // 0xC4 lo hi — CALL if Z clear. Return addr = 0xFF83. 24 cycles.
        let (cpu, mmu, cycles) = exec_with(&[0xC4, 0x00, 0x80], |cpu, _| {
            cpu.regs.sp = 0xFFFE;
            cpu.regs.set_flag_z(false);
        });
        assert_eq!(cpu.regs.pc, 0x8000);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        assert_eq!(mmu.read(0xFFFC), 0x83); // return addr low
        assert_eq!(mmu.read(0xFFFD), 0xFF); // return addr high
        assert_eq!(cycles, 24);
    }

    #[test]
    fn call_nz_not_taken() {
        // 0xC4 lo hi — no call if Z set. PC advances past the 3-byte instruction. 12 cycles.
        let (cpu, _, cycles) = exec_with(&[0xC4, 0x00, 0x80], |cpu, _| {
            cpu.regs.set_flag_z(true);
        });
        assert_eq!(cpu.regs.pc, 0xFF83);
        assert_eq!(cycles, 12);
    }

    // -------------------------------------------------------------------------
    // RET NZ (taken and not taken)
    // -------------------------------------------------------------------------

    #[test]
    fn ret_nz_taken() {
        // 0xC0 — RET if Z clear. Pop 0x4321 from stack. 20 cycles.
        let (cpu, _, cycles) = exec_with(&[0xC0], |cpu, mmu| {
            cpu.regs.set_flag_z(false);
            cpu.regs.sp = 0xFFFC;
            mmu.write(0xFFFC, 0x21); // low byte
            mmu.write(0xFFFD, 0x43); // high byte
        });
        assert_eq!(cpu.regs.pc, 0x4321);
        assert_eq!(cpu.regs.sp, 0xFFFE);
        assert_eq!(cycles, 20);
    }

    #[test]
    fn ret_nz_not_taken() {
        // 0xC0 — no RET if Z set. PC advances 1 byte. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xC0], |cpu, _| {
            cpu.regs.set_flag_z(true);
        });
        assert_eq!(cpu.regs.pc, 0xFF81);
        assert_eq!(cycles, 8);
    }

    // -------------------------------------------------------------------------
    // LD A, (nn)
    // -------------------------------------------------------------------------

    #[test]
    fn ld_a_nn() {
        // 0xFA lo hi — A = (nn). Write 0xDE to 0xC500, then load it. 16 cycles.
        let (cpu, _, cycles) = exec_with(&[0xFA, 0x00, 0xC5], |_, mmu| {
            mmu.write(0xC500, 0xDE);
        });
        assert_eq!(cpu.regs.a, 0xDE);
        assert_eq!(cycles, 16);
    }

    // -------------------------------------------------------------------------
    // ADC A, imm  /  SBC A, imm
    // -------------------------------------------------------------------------

    #[test]
    fn adc_a_imm() {
        // 0xCE val — ADC A, val. A=0x10, val=0x01, carry=1 → A=0x12. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xCE, 0x01], |cpu, _| {
            cpu.regs.a = 0x10;
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.a, 0x12);
        assert!(!cpu.regs.flag_z());
        assert!(!cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 8);
    }

    #[test]
    fn sbc_a_imm() {
        // 0xDE val — SBC A, val. A=0x10, val=0x05, carry=1 → A=0x0A. 8 cycles.
        let (cpu, _, cycles) = exec_with(&[0xDE, 0x05], |cpu, _| {
            cpu.regs.a = 0x10;
            cpu.regs.set_flag_c(true);
        });
        assert_eq!(cpu.regs.a, 0x0A);
        assert!(!cpu.regs.flag_z());
        assert!(cpu.regs.flag_n());
        assert!(!cpu.regs.flag_c());
        assert_eq!(cycles, 8);
    }
}
