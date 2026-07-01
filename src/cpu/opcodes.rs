use super::Cpu;
use crate::mmu::Mmu;

impl Cpu {
    /// Decode and execute one base opcode. Returns T-cycle count.
    pub(crate) fn execute(&mut self, opcode: u8, mmu: &mut Mmu) -> u32 {
        match opcode {
            // --- NOP ---
            0x00 => 4,

            // --- LD rr, nn ---
            0x01 => { let v = self.fetch_word(mmu); self.regs.set_bc(v); 12 }
            0x11 => { let v = self.fetch_word(mmu); self.regs.set_de(v); 12 }
            0x21 => { let v = self.fetch_word(mmu); self.regs.set_hl(v); 12 }
            0x31 => { self.regs.sp = self.fetch_word(mmu); 12 }

            // --- LD (rr), A / LD A, (rr) ---
            0x02 => { mmu.write(self.regs.bc(), self.regs.a); 8 }
            0x12 => { mmu.write(self.regs.de(), self.regs.a); 8 }
            0x0A => { self.regs.a = mmu.read(self.regs.bc()); 8 }
            0x1A => { self.regs.a = mmu.read(self.regs.de()); 8 }

            // --- LD (HL+/-), A / LD A, (HL+/-) ---
            0x22 => { let hl = self.regs.hl(); mmu.write(hl, self.regs.a); self.regs.set_hl(hl.wrapping_add(1)); 8 }
            0x32 => { let hl = self.regs.hl(); mmu.write(hl, self.regs.a); self.regs.set_hl(hl.wrapping_sub(1)); 8 }
            0x2A => { let hl = self.regs.hl(); self.regs.a = mmu.read(hl); self.regs.set_hl(hl.wrapping_add(1)); 8 }
            0x3A => { let hl = self.regs.hl(); self.regs.a = mmu.read(hl); self.regs.set_hl(hl.wrapping_sub(1)); 8 }

            // --- LD (nn), SP ---
            0x08 => {
                let addr = self.fetch_word(mmu);
                mmu.write(addr, self.regs.sp as u8);
                mmu.write(addr.wrapping_add(1), (self.regs.sp >> 8) as u8);
                20
            }

            // --- INC rr ---
            0x03 => { self.regs.set_bc(self.regs.bc().wrapping_add(1)); 8 }
            0x13 => { self.regs.set_de(self.regs.de().wrapping_add(1)); 8 }
            0x23 => { self.regs.set_hl(self.regs.hl().wrapping_add(1)); 8 }
            0x33 => { self.regs.sp = self.regs.sp.wrapping_add(1); 8 }

            // --- DEC rr ---
            0x0B => { self.regs.set_bc(self.regs.bc().wrapping_sub(1)); 8 }
            0x1B => { self.regs.set_de(self.regs.de().wrapping_sub(1)); 8 }
            0x2B => { self.regs.set_hl(self.regs.hl().wrapping_sub(1)); 8 }
            0x3B => { self.regs.sp = self.regs.sp.wrapping_sub(1); 8 }

            // --- ADD HL, rr ---
            0x09 => { let v = self.regs.bc(); self.alu_add_hl(v); 8 }
            0x19 => { let v = self.regs.de(); self.alu_add_hl(v); 8 }
            0x29 => { let v = self.regs.hl(); self.alu_add_hl(v); 8 }
            0x39 => { let v = self.regs.sp; self.alu_add_hl(v); 8 }

            // --- INC r ---
            0x04 => { self.regs.b = self.alu_inc(self.regs.b); 4 }
            0x0C => { self.regs.c = self.alu_inc(self.regs.c); 4 }
            0x14 => { self.regs.d = self.alu_inc(self.regs.d); 4 }
            0x1C => { self.regs.e = self.alu_inc(self.regs.e); 4 }
            0x24 => { self.regs.h = self.alu_inc(self.regs.h); 4 }
            0x2C => { self.regs.l = self.alu_inc(self.regs.l); 4 }
            0x34 => { let hl = self.regs.hl(); let v = self.alu_inc(mmu.read(hl)); mmu.write(hl, v); 12 }
            0x3C => { self.regs.a = self.alu_inc(self.regs.a); 4 }

            // --- DEC r ---
            0x05 => { self.regs.b = self.alu_dec(self.regs.b); 4 }
            0x0D => { self.regs.c = self.alu_dec(self.regs.c); 4 }
            0x15 => { self.regs.d = self.alu_dec(self.regs.d); 4 }
            0x1D => { self.regs.e = self.alu_dec(self.regs.e); 4 }
            0x25 => { self.regs.h = self.alu_dec(self.regs.h); 4 }
            0x2D => { self.regs.l = self.alu_dec(self.regs.l); 4 }
            0x35 => { let hl = self.regs.hl(); let v = self.alu_dec(mmu.read(hl)); mmu.write(hl, v); 12 }
            0x3D => { self.regs.a = self.alu_dec(self.regs.a); 4 }

            // --- LD r, n (8-bit immediate) ---
            0x06 => { self.regs.b = self.fetch_byte(mmu); 8 }
            0x0E => { self.regs.c = self.fetch_byte(mmu); 8 }
            0x16 => { self.regs.d = self.fetch_byte(mmu); 8 }
            0x1E => { self.regs.e = self.fetch_byte(mmu); 8 }
            0x26 => { self.regs.h = self.fetch_byte(mmu); 8 }
            0x2E => { self.regs.l = self.fetch_byte(mmu); 8 }
            0x36 => { let v = self.fetch_byte(mmu); mmu.write(self.regs.hl(), v); 12 }
            0x3E => { self.regs.a = self.fetch_byte(mmu); 8 }

            // --- Rotate A instructions ---
            0x07 => { self.rlca(); 4 }
            0x0F => { self.rrca(); 4 }
            0x17 => { self.rla(); 4 }
            0x1F => { self.rra(); 4 }

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
            0x41 => { self.regs.b = self.regs.c; 4 }
            0x42 => { self.regs.b = self.regs.d; 4 }
            0x43 => { self.regs.b = self.regs.e; 4 }
            0x44 => { self.regs.b = self.regs.h; 4 }
            0x45 => { self.regs.b = self.regs.l; 4 }
            0x46 => { self.regs.b = mmu.read(self.regs.hl()); 8 }
            0x47 => { self.regs.b = self.regs.a; 4 }
            // C is destination
            0x48 => { self.regs.c = self.regs.b; 4 }
            0x49 => 4, // LD C, C
            0x4A => { self.regs.c = self.regs.d; 4 }
            0x4B => { self.regs.c = self.regs.e; 4 }
            0x4C => { self.regs.c = self.regs.h; 4 }
            0x4D => { self.regs.c = self.regs.l; 4 }
            0x4E => { self.regs.c = mmu.read(self.regs.hl()); 8 }
            0x4F => { self.regs.c = self.regs.a; 4 }
            // D is destination
            0x50 => { self.regs.d = self.regs.b; 4 }
            0x51 => { self.regs.d = self.regs.c; 4 }
            0x52 => 4, // LD D, D
            0x53 => { self.regs.d = self.regs.e; 4 }
            0x54 => { self.regs.d = self.regs.h; 4 }
            0x55 => { self.regs.d = self.regs.l; 4 }
            0x56 => { self.regs.d = mmu.read(self.regs.hl()); 8 }
            0x57 => { self.regs.d = self.regs.a; 4 }
            // E is destination
            0x58 => { self.regs.e = self.regs.b; 4 }
            0x59 => { self.regs.e = self.regs.c; 4 }
            0x5A => { self.regs.e = self.regs.d; 4 }
            0x5B => 4, // LD E, E
            0x5C => { self.regs.e = self.regs.h; 4 }
            0x5D => { self.regs.e = self.regs.l; 4 }
            0x5E => { self.regs.e = mmu.read(self.regs.hl()); 8 }
            0x5F => { self.regs.e = self.regs.a; 4 }
            // H is destination
            0x60 => { self.regs.h = self.regs.b; 4 }
            0x61 => { self.regs.h = self.regs.c; 4 }
            0x62 => { self.regs.h = self.regs.d; 4 }
            0x63 => { self.regs.h = self.regs.e; 4 }
            0x64 => 4, // LD H, H
            0x65 => { self.regs.h = self.regs.l; 4 }
            0x66 => { self.regs.h = mmu.read(self.regs.hl()); 8 }
            0x67 => { self.regs.h = self.regs.a; 4 }
            // L is destination
            0x68 => { self.regs.l = self.regs.b; 4 }
            0x69 => { self.regs.l = self.regs.c; 4 }
            0x6A => { self.regs.l = self.regs.d; 4 }
            0x6B => { self.regs.l = self.regs.e; 4 }
            0x6C => { self.regs.l = self.regs.h; 4 }
            0x6D => 4, // LD L, L
            0x6E => { self.regs.l = mmu.read(self.regs.hl()); 8 }
            0x6F => { self.regs.l = self.regs.a; 4 }
            // (HL) is destination
            0x70 => { mmu.write(self.regs.hl(), self.regs.b); 8 }
            0x71 => { mmu.write(self.regs.hl(), self.regs.c); 8 }
            0x72 => { mmu.write(self.regs.hl(), self.regs.d); 8 }
            0x73 => { mmu.write(self.regs.hl(), self.regs.e); 8 }
            0x74 => { mmu.write(self.regs.hl(), self.regs.h); 8 }
            0x75 => { mmu.write(self.regs.hl(), self.regs.l); 8 }
            // 0x76 = HALT (handled below)
            0x77 => { mmu.write(self.regs.hl(), self.regs.a); 8 }
            // A is destination
            0x78 => { self.regs.a = self.regs.b; 4 }
            0x79 => { self.regs.a = self.regs.c; 4 }
            0x7A => { self.regs.a = self.regs.d; 4 }
            0x7B => { self.regs.a = self.regs.e; 4 }
            0x7C => { self.regs.a = self.regs.h; 4 }
            0x7D => { self.regs.a = self.regs.l; 4 }
            0x7E => { self.regs.a = mmu.read(self.regs.hl()); 8 }
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
            0x80 => { let v = self.regs.b; self.alu_add(v); 4 }
            0x81 => { let v = self.regs.c; self.alu_add(v); 4 }
            0x82 => { let v = self.regs.d; self.alu_add(v); 4 }
            0x83 => { let v = self.regs.e; self.alu_add(v); 4 }
            0x84 => { let v = self.regs.h; self.alu_add(v); 4 }
            0x85 => { let v = self.regs.l; self.alu_add(v); 4 }
            0x86 => { let v = mmu.read(self.regs.hl()); self.alu_add(v); 8 }
            0x87 => { let v = self.regs.a; self.alu_add(v); 4 }
            // ADC A, r
            0x88 => { let v = self.regs.b; self.alu_adc(v); 4 }
            0x89 => { let v = self.regs.c; self.alu_adc(v); 4 }
            0x8A => { let v = self.regs.d; self.alu_adc(v); 4 }
            0x8B => { let v = self.regs.e; self.alu_adc(v); 4 }
            0x8C => { let v = self.regs.h; self.alu_adc(v); 4 }
            0x8D => { let v = self.regs.l; self.alu_adc(v); 4 }
            0x8E => { let v = mmu.read(self.regs.hl()); self.alu_adc(v); 8 }
            0x8F => { let v = self.regs.a; self.alu_adc(v); 4 }
            // SUB A, r
            0x90 => { let v = self.regs.b; self.alu_sub(v); 4 }
            0x91 => { let v = self.regs.c; self.alu_sub(v); 4 }
            0x92 => { let v = self.regs.d; self.alu_sub(v); 4 }
            0x93 => { let v = self.regs.e; self.alu_sub(v); 4 }
            0x94 => { let v = self.regs.h; self.alu_sub(v); 4 }
            0x95 => { let v = self.regs.l; self.alu_sub(v); 4 }
            0x96 => { let v = mmu.read(self.regs.hl()); self.alu_sub(v); 8 }
            0x97 => { let v = self.regs.a; self.alu_sub(v); 4 }
            // SBC A, r
            0x98 => { let v = self.regs.b; self.alu_sbc(v); 4 }
            0x99 => { let v = self.regs.c; self.alu_sbc(v); 4 }
            0x9A => { let v = self.regs.d; self.alu_sbc(v); 4 }
            0x9B => { let v = self.regs.e; self.alu_sbc(v); 4 }
            0x9C => { let v = self.regs.h; self.alu_sbc(v); 4 }
            0x9D => { let v = self.regs.l; self.alu_sbc(v); 4 }
            0x9E => { let v = mmu.read(self.regs.hl()); self.alu_sbc(v); 8 }
            0x9F => { let v = self.regs.a; self.alu_sbc(v); 4 }
            // AND A, r
            0xA0 => { let v = self.regs.b; self.alu_and(v); 4 }
            0xA1 => { let v = self.regs.c; self.alu_and(v); 4 }
            0xA2 => { let v = self.regs.d; self.alu_and(v); 4 }
            0xA3 => { let v = self.regs.e; self.alu_and(v); 4 }
            0xA4 => { let v = self.regs.h; self.alu_and(v); 4 }
            0xA5 => { let v = self.regs.l; self.alu_and(v); 4 }
            0xA6 => { let v = mmu.read(self.regs.hl()); self.alu_and(v); 8 }
            0xA7 => { let v = self.regs.a; self.alu_and(v); 4 }
            // XOR A, r
            0xA8 => { let v = self.regs.b; self.alu_xor(v); 4 }
            0xA9 => { let v = self.regs.c; self.alu_xor(v); 4 }
            0xAA => { let v = self.regs.d; self.alu_xor(v); 4 }
            0xAB => { let v = self.regs.e; self.alu_xor(v); 4 }
            0xAC => { let v = self.regs.h; self.alu_xor(v); 4 }
            0xAD => { let v = self.regs.l; self.alu_xor(v); 4 }
            0xAE => { let v = mmu.read(self.regs.hl()); self.alu_xor(v); 8 }
            0xAF => { let v = self.regs.a; self.alu_xor(v); 4 }
            // OR A, r
            0xB0 => { let v = self.regs.b; self.alu_or(v); 4 }
            0xB1 => { let v = self.regs.c; self.alu_or(v); 4 }
            0xB2 => { let v = self.regs.d; self.alu_or(v); 4 }
            0xB3 => { let v = self.regs.e; self.alu_or(v); 4 }
            0xB4 => { let v = self.regs.h; self.alu_or(v); 4 }
            0xB5 => { let v = self.regs.l; self.alu_or(v); 4 }
            0xB6 => { let v = mmu.read(self.regs.hl()); self.alu_or(v); 8 }
            0xB7 => { let v = self.regs.a; self.alu_or(v); 4 }
            // CP A, r
            0xB8 => { let v = self.regs.b; self.alu_cp(v); 4 }
            0xB9 => { let v = self.regs.c; self.alu_cp(v); 4 }
            0xBA => { let v = self.regs.d; self.alu_cp(v); 4 }
            0xBB => { let v = self.regs.e; self.alu_cp(v); 4 }
            0xBC => { let v = self.regs.h; self.alu_cp(v); 4 }
            0xBD => { let v = self.regs.l; self.alu_cp(v); 4 }
            0xBE => { let v = mmu.read(self.regs.hl()); self.alu_cp(v); 8 }
            0xBF => { let v = self.regs.a; self.alu_cp(v); 4 }

            // --- ALU A, n (immediate) ---
            0xC6 => { let v = self.fetch_byte(mmu); self.alu_add(v); 8 }
            0xCE => { let v = self.fetch_byte(mmu); self.alu_adc(v); 8 }
            0xD6 => { let v = self.fetch_byte(mmu); self.alu_sub(v); 8 }
            0xDE => { let v = self.fetch_byte(mmu); self.alu_sbc(v); 8 }
            0xE6 => { let v = self.fetch_byte(mmu); self.alu_and(v); 8 }
            0xEE => { let v = self.fetch_byte(mmu); self.alu_xor(v); 8 }
            0xF6 => { let v = self.fetch_byte(mmu); self.alu_or(v); 8 }
            0xFE => { let v = self.fetch_byte(mmu); self.alu_cp(v); 8 }

            // --- JP nn ---
            0xC3 => { self.regs.pc = self.fetch_word(mmu); 16 }

            // --- JP cc, nn ---
            0xC2 => { let addr = self.fetch_word(mmu); if !self.regs.flag_z() { self.regs.pc = addr; 16 } else { 12 } }
            0xCA => { let addr = self.fetch_word(mmu); if  self.regs.flag_z() { self.regs.pc = addr; 16 } else { 12 } }
            0xD2 => { let addr = self.fetch_word(mmu); if !self.regs.flag_c() { self.regs.pc = addr; 16 } else { 12 } }
            0xDA => { let addr = self.fetch_word(mmu); if  self.regs.flag_c() { self.regs.pc = addr; 16 } else { 12 } }

            // --- JP (HL) ---
            0xE9 => { self.regs.pc = self.regs.hl(); 4 }

            // --- JR n ---
            0x18 => {
                let offset = self.fetch_byte(mmu) as i8;
                self.regs.pc = self.regs.pc.wrapping_add(offset as u16);
                12
            }

            // --- JR cc, n ---
            0x20 => { let o = self.fetch_byte(mmu) as i8; if !self.regs.flag_z() { self.regs.pc = self.regs.pc.wrapping_add(o as u16); 12 } else { 8 } }
            0x28 => { let o = self.fetch_byte(mmu) as i8; if  self.regs.flag_z() { self.regs.pc = self.regs.pc.wrapping_add(o as u16); 12 } else { 8 } }
            0x30 => { let o = self.fetch_byte(mmu) as i8; if !self.regs.flag_c() { self.regs.pc = self.regs.pc.wrapping_add(o as u16); 12 } else { 8 } }
            0x38 => { let o = self.fetch_byte(mmu) as i8; if  self.regs.flag_c() { self.regs.pc = self.regs.pc.wrapping_add(o as u16); 12 } else { 8 } }

            // --- CALL nn ---
            0xCD => {
                let addr = self.fetch_word(mmu);
                self.push(mmu, self.regs.pc);
                self.regs.pc = addr;
                24
            }

            // --- CALL cc, nn ---
            0xC4 => { let addr = self.fetch_word(mmu); if !self.regs.flag_z() { self.push(mmu, self.regs.pc); self.regs.pc = addr; 24 } else { 12 } }
            0xCC => { let addr = self.fetch_word(mmu); if  self.regs.flag_z() { self.push(mmu, self.regs.pc); self.regs.pc = addr; 24 } else { 12 } }
            0xD4 => { let addr = self.fetch_word(mmu); if !self.regs.flag_c() { self.push(mmu, self.regs.pc); self.regs.pc = addr; 24 } else { 12 } }
            0xDC => { let addr = self.fetch_word(mmu); if  self.regs.flag_c() { self.push(mmu, self.regs.pc); self.regs.pc = addr; 24 } else { 12 } }

            // --- RET ---
            0xC9 => { self.regs.pc = self.pop(mmu); 16 }

            // --- RET cc ---
            0xC0 => { if !self.regs.flag_z() { self.regs.pc = self.pop(mmu); 20 } else { 8 } }
            0xC8 => { if  self.regs.flag_z() { self.regs.pc = self.pop(mmu); 20 } else { 8 } }
            0xD0 => { if !self.regs.flag_c() { self.regs.pc = self.pop(mmu); 20 } else { 8 } }
            0xD8 => { if  self.regs.flag_c() { self.regs.pc = self.pop(mmu); 20 } else { 8 } }

            // --- RETI ---
            0xD9 => {
                self.regs.pc = self.pop(mmu);
                self.ime = true;
                16
            }

            // --- RST vec ---
            0xC7 => { self.push(mmu, self.regs.pc); self.regs.pc = 0x00; 16 }
            0xCF => { self.push(mmu, self.regs.pc); self.regs.pc = 0x08; 16 }
            0xD7 => { self.push(mmu, self.regs.pc); self.regs.pc = 0x10; 16 }
            0xDF => { self.push(mmu, self.regs.pc); self.regs.pc = 0x18; 16 }
            0xE7 => { self.push(mmu, self.regs.pc); self.regs.pc = 0x20; 16 }
            0xEF => { self.push(mmu, self.regs.pc); self.regs.pc = 0x28; 16 }
            0xF7 => { self.push(mmu, self.regs.pc); self.regs.pc = 0x30; 16 }
            0xFF => { self.push(mmu, self.regs.pc); self.regs.pc = 0x38; 16 }

            // --- PUSH rr ---
            0xC5 => { let v = self.regs.bc(); self.push(mmu, v); 16 }
            0xD5 => { let v = self.regs.de(); self.push(mmu, v); 16 }
            0xE5 => { let v = self.regs.hl(); self.push(mmu, v); 16 }
            0xF5 => { let v = self.regs.af(); self.push(mmu, v); 16 }

            // --- POP rr ---
            0xC1 => { let v = self.pop(mmu); self.regs.set_bc(v); 12 }
            0xD1 => { let v = self.pop(mmu); self.regs.set_de(v); 12 }
            0xE1 => { let v = self.pop(mmu); self.regs.set_hl(v); 12 }
            0xF1 => { let v = self.pop(mmu); self.regs.set_af(v); 12 } // set_af masks F bits 0-3

            // --- LDH (n), A / LDH A, (n) ---
            0xE0 => { let n = self.fetch_byte(mmu) as u16; mmu.write(0xFF00 + n, self.regs.a); 12 }
            0xF0 => { let n = self.fetch_byte(mmu) as u16; self.regs.a = mmu.read(0xFF00 + n); 12 }

            // --- LD (C), A / LD A, (C) ---
            0xE2 => { mmu.write(0xFF00 + self.regs.c as u16, self.regs.a); 8 }
            0xF2 => { self.regs.a = mmu.read(0xFF00 + self.regs.c as u16); 8 }

            // --- LD (nn), A / LD A, (nn) ---
            0xEA => { let addr = self.fetch_word(mmu); mmu.write(addr, self.regs.a); 16 }
            0xFA => { let addr = self.fetch_word(mmu); self.regs.a = mmu.read(addr); 16 }

            // --- LD SP, HL ---
            0xF9 => { self.regs.sp = self.regs.hl(); 8 }

            // --- LD HL, SP+n ---
            0xF8 => {
                let offset = self.fetch_byte(mmu) as i8 as i16;
                let sp = self.regs.sp;
                let result = sp.wrapping_add(offset as u16);
                self.regs.set_hl(result);
                self.regs.set_flag_z(false);
                self.regs.set_flag_n(false);
                // H and C are computed on the unsigned low-byte addition
                self.regs.set_flag_h((sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F);
                self.regs.set_flag_c((sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF);
                12
            }

            // --- ADD SP, n ---
            0xE8 => {
                let offset = self.fetch_byte(mmu) as i8 as i16;
                let sp = self.regs.sp;
                let result = sp.wrapping_add(offset as u16);
                self.regs.set_flag_z(false);
                self.regs.set_flag_n(false);
                self.regs.set_flag_h((sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F);
                self.regs.set_flag_c((sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF);
                self.regs.sp = result;
                16
            }

            // --- DI / EI ---
            0xF3 => { self.ime = false; 4 }
            0xFB => { self.ime_scheduled = true; 4 }

            // --- STOP ---
            0x10 => { let _ = self.fetch_byte(mmu); 4 }

            // --- CB prefix ---
            0xCB => {
                let cb_op = self.fetch_byte(mmu);
                self.execute_cb(cb_op, mmu)
            }

            // Unused opcodes on DMG behave like a 2-byte NOP that locks up the CPU.
            // Treat them as NOP to keep the emulator running.
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFD => {
                #[cfg(debug_assertions)]
                eprintln!("Illegal opcode: 0x{:02X} at PC=0x{:04X}", opcode, self.regs.pc.wrapping_sub(1));
                4
            }
        }
    }
}
