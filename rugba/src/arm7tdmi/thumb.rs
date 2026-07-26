use super::{Arm7Tdmi, Bus, CpuMode, C_FLAG, T_FLAG, V_FLAG};

/// Execute a single THUMB (16-bit) instruction and return cycles consumed.
pub fn execute_thumb(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u16) -> u32 {
    let op = instruction >> 8;

    match op {
        // ===== Format 1: Move shifted register (LSL/LSR/ASR imm5) =====
        // bits 15-13 = 000, bits 12-11 = opcode (00=LSL, 01=LSR, 10=ASR)
        0x00..=0x07 if (instruction >> 11) != 0b00011 => {
            let shift_op = (instruction >> 11) & 0x03;
            let offset5 = ((instruction >> 6) & 0x1F) as u32;
            let rs = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;
            let source = cpu.regs[rs];

            let result = match shift_op {
                0 => {
                    // LSL
                    if offset5 == 0 {
                        source
                    } else {
                        cpu.set_flag(C_FLAG, (source >> (32 - offset5)) & 1 != 0);
                        source << offset5
                    }
                }
                1 => {
                    // LSR
                    let shift = if offset5 == 0 { 32 } else { offset5 };
                    if shift == 32 {
                        cpu.set_flag(C_FLAG, source >> 31 != 0);
                        0
                    } else {
                        cpu.set_flag(C_FLAG, (source >> (shift - 1)) & 1 != 0);
                        source >> shift
                    }
                }
                2 => {
                    // ASR
                    let shift = if offset5 == 0 { 32 } else { offset5 };
                    if shift >= 32 {
                        let bit31 = (source as i32) >> 31;
                        cpu.set_flag(C_FLAG, bit31 as u32 & 1 != 0);
                        bit31 as u32
                    } else {
                        cpu.set_flag(C_FLAG, (source >> (shift - 1)) & 1 != 0);
                        ((source as i32) >> shift) as u32
                    }
                }
                _ => unreachable!(),
            };

            cpu.regs[rd] = result;
            cpu.set_nz(result);
            1
        }

        // ===== Format 2: Add/Subtract =====
        // bits 15-11 = 00011
        0x18..=0x1F => {
            let i_flag = (instruction >> 10) & 1 != 0;
            let sub = (instruction >> 9) & 1 != 0;
            let rn_or_imm = ((instruction >> 6) & 0x07) as u32;
            let rs = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;

            let operand = if i_flag {
                rn_or_imm
            } else {
                cpu.regs[rn_or_imm as usize]
            };
            let source = cpu.regs[rs];

            let result = if sub {
                let (res, borrow) = source.overflowing_sub(operand);
                cpu.set_flag(C_FLAG, !borrow);
                let v = ((source ^ operand) & (source ^ res)) >> 31 != 0;
                cpu.set_flag(V_FLAG, v);
                res
            } else {
                let (res, carry) = source.overflowing_add(operand);
                cpu.set_flag(C_FLAG, carry);
                let v = (!(source ^ operand) & (source ^ res)) >> 31 != 0;
                cpu.set_flag(V_FLAG, v);
                res
            };

            cpu.regs[rd] = result;
            cpu.set_nz(result);
            1
        }

        // ===== Format 3: Move/Compare/Add/Subtract immediate =====
        // bits 15-13 = 001
        0x20..=0x3F => {
            let op_code = (instruction >> 11) & 0x03;
            let rd = ((instruction >> 8) & 0x07) as usize;
            let imm8 = (instruction & 0xFF) as u32;

            match op_code {
                0 => {
                    // MOV
                    cpu.regs[rd] = imm8;
                    cpu.set_nz(imm8);
                }
                1 => {
                    // CMP
                    let source = cpu.regs[rd];
                    let (res, borrow) = source.overflowing_sub(imm8);
                    cpu.set_flag(C_FLAG, !borrow);
                    let v = ((source ^ imm8) & (source ^ res)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.set_nz(res);
                }
                2 => {
                    // ADD
                    let source = cpu.regs[rd];
                    let (res, carry) = source.overflowing_add(imm8);
                    cpu.set_flag(C_FLAG, carry);
                    let v = (!(source ^ imm8) & (source ^ res)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.regs[rd] = res;
                    cpu.set_nz(res);
                }
                3 => {
                    // SUB
                    let source = cpu.regs[rd];
                    let (res, borrow) = source.overflowing_sub(imm8);
                    cpu.set_flag(C_FLAG, !borrow);
                    let v = ((source ^ imm8) & (source ^ res)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.regs[rd] = res;
                    cpu.set_nz(res);
                }
                _ => unreachable!(),
            }
            1
        }

        // ===== Format 4: ALU operations =====
        // bits 15-10 = 010000
        0x40..=0x43 => {
            let alu_op = (instruction >> 6) & 0x0F;
            let rs = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;
            let a = cpu.regs[rd];
            let b = cpu.regs[rs];

            match alu_op {
                0x0 => {
                    // AND
                    let r = a & b;
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x1 => {
                    // EOR
                    let r = a ^ b;
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x2 => {
                    // LSL
                    let shift = b & 0xFF;
                    let r = if shift == 0 {
                        a
                    } else if shift < 32 {
                        cpu.set_flag(C_FLAG, (a >> (32 - shift)) & 1 != 0);
                        a << shift
                    } else if shift == 32 {
                        cpu.set_flag(C_FLAG, a & 1 != 0);
                        0
                    } else {
                        cpu.set_flag(C_FLAG, false);
                        0
                    };
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x3 => {
                    // LSR
                    let shift = b & 0xFF;
                    let r = if shift == 0 {
                        a
                    } else if shift < 32 {
                        cpu.set_flag(C_FLAG, (a >> (shift - 1)) & 1 != 0);
                        a >> shift
                    } else if shift == 32 {
                        cpu.set_flag(C_FLAG, a >> 31 != 0);
                        0
                    } else {
                        cpu.set_flag(C_FLAG, false);
                        0
                    };
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x4 => {
                    // ASR
                    let shift = b & 0xFF;
                    let r = if shift == 0 {
                        a
                    } else if shift < 32 {
                        cpu.set_flag(C_FLAG, (a >> (shift - 1)) & 1 != 0);
                        ((a as i32) >> shift) as u32
                    } else {
                        let bit31 = (a as i32) >> 31;
                        cpu.set_flag(C_FLAG, bit31 as u32 & 1 != 0);
                        bit31 as u32
                    };
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x5 => {
                    // ADC
                    let c = if cpu.get_flag(C_FLAG) { 1u32 } else { 0 };
                    let (r1, c1) = a.overflowing_add(b);
                    let (r2, c2) = r1.overflowing_add(c);
                    cpu.set_flag(C_FLAG, c1 || c2);
                    let v = (!(a ^ b) & (a ^ r2)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.regs[rd] = r2;
                    cpu.set_nz(r2);
                }
                0x6 => {
                    // SBC
                    let c = if cpu.get_flag(C_FLAG) { 0u32 } else { 1 };
                    let (r1, b1) = a.overflowing_sub(b);
                    let (r2, b2) = r1.overflowing_sub(c);
                    cpu.set_flag(C_FLAG, !(b1 || b2));
                    let v = ((a ^ b) & (a ^ r2)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.regs[rd] = r2;
                    cpu.set_nz(r2);
                }
                0x7 => {
                    // ROR
                    let shift = b & 0xFF;
                    let r = if shift == 0 {
                        a
                    } else {
                        let rot = shift & 31;

                        if rot == 0 {
                            cpu.set_flag(C_FLAG, a >> 31 != 0);
                            a
                        } else {
                            cpu.set_flag(C_FLAG, (a >> (rot - 1)) & 1 != 0);
                            a.rotate_right(rot)
                        }
                    };
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0x8 => {
                    // TST
                    let r = a & b;
                    cpu.set_nz(r);
                }
                0x9 => {
                    // NEG
                    let (r, borrow) = 0u32.overflowing_sub(b);
                    cpu.set_flag(C_FLAG, !borrow);
                    let v = (b & r) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0xA => {
                    // CMP
                    let (r, borrow) = a.overflowing_sub(b);
                    cpu.set_flag(C_FLAG, !borrow);
                    let v = ((a ^ b) & (a ^ r)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.set_nz(r);
                }
                0xB => {
                    // CMN
                    let (r, carry) = a.overflowing_add(b);
                    cpu.set_flag(C_FLAG, carry);
                    let v = (!(a ^ b) & (a ^ r)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.set_nz(r);
                }
                0xC => {
                    // ORR
                    let r = a | b;
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0xD => {
                    // MUL
                    let r = a.wrapping_mul(b);
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0xE => {
                    // BIC
                    let r = a & !b;
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                0xF => {
                    // MVN
                    let r = !b;
                    cpu.regs[rd] = r;
                    cpu.set_nz(r);
                }
                _ => unreachable!(),
            }

            // MUL takes extra cycles
            if alu_op == 0xD {
                4
            } else {
                1
            }
        }

        // ===== Format 5: Hi register operations / BX =====
        // bits 15-10 = 010001
        0x44..=0x47 => {
            let hi_op = (instruction >> 8) & 0x03;
            let h2 = ((instruction >> 6) & 1) as usize;
            let h1 = ((instruction >> 7) & 1) as usize;
            let rs = (((instruction >> 3) & 0x07) as usize) | (h2 << 3);
            let rd = ((instruction & 0x07) as usize) | (h1 << 3);

            let rs_val = if rs == 15 {
                cpu.regs[15].wrapping_add(2)
            } else {
                cpu.regs[rs]
            };
            let rd_val = if rd == 15 {
                cpu.regs[15].wrapping_add(2)
            } else {
                cpu.regs[rd]
            };

            match hi_op {
                0 => {
                    // ADD
                    let result = rd_val.wrapping_add(rs_val);
                    if rd == 15 {
                        cpu.regs[15] = result & !1;
                        return 3;
                    }
                    cpu.regs[rd] = result;
                }
                1 => {
                    // CMP
                    let (r, borrow) = rd_val.overflowing_sub(rs_val);
                    cpu.set_flag(C_FLAG, !borrow);
                    let v = ((rd_val ^ rs_val) & (rd_val ^ r)) >> 31 != 0;
                    cpu.set_flag(V_FLAG, v);
                    cpu.set_nz(r);
                }
                2 => {
                    // MOV
                    if rd == 15 {
                        cpu.regs[15] = rs_val & !1;
                        return 3;
                    }
                    cpu.regs[rd] = rs_val;
                }
                3 => {
                    // BX
                    let addr = rs_val;
                    if addr & 1 == 0 {
                        // Switch to ARM mode
                        cpu.set_flag(T_FLAG, false);
                        cpu.regs[15] = addr & !3;
                    } else {
                        cpu.regs[15] = addr & !1;
                    }
                    return 3;
                }
                _ => unreachable!(),
            }
            1
        }

        // ===== Format 6: PC-relative load =====
        // bits 15-11 = 01001
        0x48..=0x4F => {
            let rd = ((instruction >> 8) & 0x07) as usize;
            let imm8 = (instruction & 0xFF) as u32;
            let addr = (cpu.regs[15] & !2).wrapping_add(imm8 << 2);
            cpu.regs[rd] = bus.read32(addr & !3);
            3
        }

        // ===== Format 7 & 8: Load/Store with register offset =====
        // bits 15-12 = 0101
        0x50..=0x5F => {
            let ro = ((instruction >> 6) & 0x07) as usize;
            let rb = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;
            let addr = cpu.regs[rb].wrapping_add(cpu.regs[ro]);

            let bit9 = (instruction >> 9) & 1 != 0;
            if !bit9 {
                // Format 7: Load/Store register offset
                let l_flag = (instruction >> 11) & 1 != 0;
                let b_flag = (instruction >> 10) & 1 != 0;
                match (l_flag, b_flag) {
                    (false, false) => {
                        bus.write32(addr & !3, cpu.regs[rd]);
                        2
                    }
                    (false, true) => {
                        bus.write8(addr, cpu.regs[rd] as u8);
                        2
                    }
                    (true, false) => {
                        cpu.regs[rd] = bus.read32(addr & !3).rotate_right((addr & 3) * 8);
                        3
                    }
                    (true, true) => {
                        cpu.regs[rd] = bus.read8(addr) as u32;
                        3
                    }
                }
            } else {
                // Format 8: Load/Store sign-extended
                let op = (instruction >> 10) & 0x03;
                match op {
                    0 => {
                        // STRH
                        bus.write16(addr & !1, cpu.regs[rd] as u16);
                        2
                    }
                    1 => {
                        // LDSB
                        cpu.regs[rd] = bus.read8(addr) as i8 as i32 as u32;
                        3
                    }
                    2 => {
                        // LDRH
                        cpu.regs[rd] = bus.read16(addr & !1) as u32;
                        3
                    }
                    3 => {
                        // LDSH
                        cpu.regs[rd] = bus.read16(addr & !1) as i16 as i32 as u32;
                        3
                    }
                    _ => unreachable!(),
                }
            }
        }

        // ===== Format 9: Load/Store with immediate offset =====
        // bits 15-13 = 011
        0x60..=0x7F => {
            let b_flag = (instruction >> 12) & 1 != 0;
            let l_flag = (instruction >> 11) & 1 != 0;
            let offset5 = ((instruction >> 6) & 0x1F) as u32;
            let rb = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;

            let base = cpu.regs[rb];
            if b_flag {
                let addr = base.wrapping_add(offset5);
                if l_flag {
                    cpu.regs[rd] = bus.read8(addr) as u32;
                    3
                } else {
                    bus.write8(addr, cpu.regs[rd] as u8);
                    2
                }
            } else {
                let addr = base.wrapping_add(offset5 << 2);
                if l_flag {
                    cpu.regs[rd] = bus.read32(addr & !3).rotate_right((addr & 3) * 8);
                    3
                } else {
                    bus.write32(addr & !3, cpu.regs[rd]);
                    2
                }
            }
        }

        // ===== Format 10: Load/Store halfword with immediate offset =====
        // bits 15-12 = 1000
        0x80..=0x8F => {
            let l_flag = (instruction >> 11) & 1 != 0;
            let offset5 = ((instruction >> 6) & 0x1F) as u32;
            let rb = ((instruction >> 3) & 0x07) as usize;
            let rd = (instruction & 0x07) as usize;
            let addr = cpu.regs[rb].wrapping_add(offset5 << 1);

            if l_flag {
                cpu.regs[rd] = bus.read16(addr & !1) as u32;
                3
            } else {
                bus.write16(addr & !1, cpu.regs[rd] as u16);
                2
            }
        }

        // ===== Format 11: SP-relative load/store =====
        // bits 15-12 = 1001
        0x90..=0x9F => {
            let l_flag = (instruction >> 11) & 1 != 0;
            let rd = ((instruction >> 8) & 0x07) as usize;
            let imm8 = (instruction & 0xFF) as u32;
            let addr = cpu.regs[13].wrapping_add(imm8 << 2);

            if l_flag {
                cpu.regs[rd] = bus.read32(addr & !3).rotate_right((addr & 3) * 8);
                3
            } else {
                bus.write32(addr & !3, cpu.regs[rd]);
                2
            }
        }

        // ===== Format 12: Load address (ADD Rd, PC/SP, #imm8*4) =====
        // bits 15-12 = 1010
        0xA0..=0xAF => {
            let sp_flag = (instruction >> 11) & 1 != 0;
            let rd = ((instruction >> 8) & 0x07) as usize;
            let imm8 = (instruction & 0xFF) as u32;

            if sp_flag {
                cpu.regs[rd] = cpu.regs[13].wrapping_add(imm8 << 2);
            } else {
                cpu.regs[rd] = (cpu.regs[15] & !2).wrapping_add(imm8 << 2);
            }
            1
        }

        // ===== Format 13: Add offset to SP =====
        // bits 15-8 = 10110000
        0xB0 => {
            let sign = (instruction >> 7) & 1 != 0;
            let imm7 = (instruction & 0x7F) as u32;
            let offset = imm7 << 2;
            if sign {
                cpu.regs[13] = cpu.regs[13].wrapping_sub(offset);
            } else {
                cpu.regs[13] = cpu.regs[13].wrapping_add(offset);
            }
            1
        }

        // ===== Format 14: Push/Pop =====
        // PUSH: bits 15-9 = 1011010
        // POP:  bits 15-9 = 1011110
        0xB4 | 0xB5 => {
            // PUSH
            let store_lr = (instruction >> 8) & 1 != 0;
            let rlist = instruction & 0xFF;
            let mut addr = cpu.regs[13];
            let count = rlist.count_ones() + if store_lr { 1 } else { 0 };
            addr = addr.wrapping_sub(count * 4);
            cpu.regs[13] = addr;

            for i in 0..8 {
                if rlist & (1 << i) != 0 {
                    bus.write32(addr, cpu.regs[i]);
                    addr = addr.wrapping_add(4);
                }
            }
            if store_lr {
                bus.write32(addr, cpu.regs[14]);
            }
            2 + count
        }

        0xBC | 0xBD => {
            // POP
            let load_pc = (instruction >> 8) & 1 != 0;
            let rlist = instruction & 0xFF;
            let mut addr = cpu.regs[13];

            for i in 0..8 {
                if rlist & (1 << i) != 0 {
                    cpu.regs[i] = bus.read32(addr);
                    addr = addr.wrapping_add(4);
                }
            }
            if load_pc {
                let val = bus.read32(addr);
                cpu.regs[15] = val & !1;
                addr = addr.wrapping_add(4);
                cpu.regs[13] = addr;
                return 4 + rlist.count_ones();
            }
            cpu.regs[13] = addr;
            3 + rlist.count_ones()
        }

        // ===== Format 15: Multiple load/store (LDMIA/STMIA) =====
        // bits 15-12 = 1100
        0xC0..=0xCF => {
            let l_flag = (instruction >> 11) & 1 != 0;
            let rb = ((instruction >> 8) & 0x07) as usize;
            let rlist = instruction & 0xFF;
            let mut addr = cpu.regs[rb];
            let count = rlist.count_ones();

            if l_flag {
                for i in 0..8 {
                    if rlist & (1 << i) != 0 {
                        cpu.regs[i] = bus.read32(addr);
                        addr = addr.wrapping_add(4);
                    }
                }
                // Write-back only if Rb not in register list
                if rlist & (1 << rb) == 0 {
                    cpu.regs[rb] = addr;
                }
                3 + count
            } else {
                let base_first = rlist & (1 << rb) != 0 && (rlist & ((1 << rb) - 1)) == 0;
                for i in 0..8 {
                    if rlist & (1 << i) != 0 {
                        bus.write32(addr, cpu.regs[i]);
                        addr = addr.wrapping_add(4);
                    }
                }
                cpu.regs[rb] = addr;
                let _ = base_first;
                2 + count
            }
        }

        // ===== Format 16: Conditional branch =====
        // bits 15-12 = 1101, cond != 1111
        0xD0..=0xDE => {
            let cond = ((instruction >> 8) & 0x0F) as u32;
            if cond == 0x0F {
                // SWI handled below
                return 1;
            }
            if cpu.check_condition(cond) {
                let offset = ((instruction & 0xFF) as i8 as i32) << 1;
                cpu.regs[15] = (cpu.regs[15] as i32).wrapping_add(offset) as u32;
                return 3;
            }
            1
        }

        // ===== Format 17: Software Interrupt (SWI) =====
        // bits 15-8 = 11011111
        0xDF => {
            cpu.enter_exception(CpuMode::Supervisor, 0x08);
            3
        }

        // ===== Format 18: Unconditional branch =====
        // bits 15-11 = 11100
        0xE0..=0xE7 => {
            let offset = instruction & 0x7FF;
            let signed_offset = if offset & 0x400 != 0 {
                ((offset as u32) | 0xFFFFF800) as i32
            } else {
                offset as i32
            };
            cpu.regs[15] = (cpu.regs[15] as i32).wrapping_add(signed_offset << 1) as u32;
            3
        }

        // ===== Format 19: Long branch with link (BL) =====
        // bits 15-12 = 1111
        0xF0..=0xF7 => {
            // First instruction: LR = PC + (offset11 << 12)
            let offset = instruction & 0x7FF;
            let signed = if offset & 0x400 != 0 {
                ((offset as u32) | 0xFFFFF800) as i32
            } else {
                offset as i32
            };
            cpu.regs[14] = (cpu.regs[15] as i32).wrapping_add(signed << 12) as u32;
            1
        }

        0xF8..=0xFF => {
            // Second instruction: PC = LR + (offset11 << 1), LR = (old_PC - 2) | 1
            let offset = (instruction & 0x7FF) as u32;
            let old_pc = cpu.regs[15].wrapping_sub(2);
            let target = cpu.regs[14].wrapping_add(offset << 1);
            cpu.regs[15] = target & !1;
            cpu.regs[14] = old_pc | 1;
            4
        }

        _ => {
            // Undefined instruction
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::execute_thumb;
    use crate::arm7tdmi::{Arm7Tdmi, C_FLAG, N_FLAG, T_FLAG, V_FLAG, Z_FLAG};
    use crate::bus::Bus;

    fn make_cpu_bus() -> (Arm7Tdmi, Bus) {
        let rom = vec![0u8; 0x200];
        let mut cpu = Arm7Tdmi::new();
        // Start in THUMB mode
        cpu.set_flag(T_FLAG, true);
        let bus = Bus::new(rom);
        (cpu, bus)
    }

    // ── Format 1: Shifted register ────────────────────────────────────────────

    #[test]
    fn thumb_lsl_imm() {
        // LSL Rd, Rs, #imm5
        // bits[15:11]=00000, imm5[10:6], Rs[5:3], Rd[2:0]
        // LSL R0, R1, #2  →  imm5=2=0b00010, Rs=1=001, Rd=0=000
        // 0b000_00_00010_001_000 = 0x0088
        let instr: u16 = 0x0088; // LSL R0, R1, #2
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 3;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 12); // 3 << 2
    }

    #[test]
    fn thumb_lsr_imm() {
        // LSR R0, R1 (ALU Format 4, register shift; LSR #32 encodes shift=32 → result 0)
        // alu_op=3 (LSR), Rs=R1=001, Rd=R0=000
        // 0b010000_0011_001_000 = 0x40C8
        let instr: u16 = 0x40C8; // LSR R0, R1  (R0 >>= R1)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 8; // value to shift (Rd is source and dest in ALU format)
        cpu.regs[1] = 1; // shift amount
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 4);
    }

    #[test]
    fn thumb_asr_imm() {
        // ASR R0, R1 (ALU Format 4, register shift; ASR #32 fills with sign bit)
        // alu_op=4 (ASR), Rs=R1=001, Rd=R0=000
        // 0b010000_0100_001_000 = 0x4108
        let instr: u16 = 0x4108; // ASR R0, R1  (R0 >>= R1, arithmetic)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0x8000_0000u32; // negative value to shift (Rd is source and dest)
        cpu.regs[1] = 1; // shift amount
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xC000_0000); // sign bit preserved
        assert!(cpu.get_flag(N_FLAG));
    }

    // ── Format 2: Add/Sub ─────────────────────────────────────────────────────

    #[test]
    fn thumb_add_reg() {
        // ADD R0, R1, R2
        // bits[15:11]=00011, I=0, sub=0, Rn=R2=010, Rs=R1=001, Rd=R0=000
        // 0b00011_0_0_010_001_000 = 0x1888
        let instr: u16 = 0x1888; // ADD R0, R1, R2
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 10;
        cpu.regs[2] = 5;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 15);
    }

    #[test]
    fn thumb_sub_imm3() {
        // SUB R0, R1, #3
        // bits[15:11]=00011, I=1, sub=1, imm3=011, Rs=R1=001, Rd=R0=000
        // 0b00011_1_1_011_001_000 = 0x1EC8
        let instr: u16 = 0x1EC8; // SUB R0, R1, #3
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 10;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 7);
    }

    // ── Format 3: Immediate ───────────────────────────────────────────────────

    #[test]
    fn thumb_mov_imm8() {
        // MOV R3, #42
        // bits[15:11]=00100+Rd=011, op=00, Rd=3=011, imm8=42=0x2A
        // 0b001_00_011_00101010 = 0x232A
        let instr: u16 = 0x232A; // MOV R3, #42
        let (mut cpu, mut bus) = make_cpu_bus();
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[3], 42);
    }

    #[test]
    fn thumb_cmp_imm8() {
        // CMP R0, #5  (R0==5 → Z set)
        // bits[15:13]=001, op=01, Rd=0, imm8=5
        // 0b001_01_000_00000101 = 0x2805
        let instr: u16 = 0x2805; // CMP R0, #5
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 5;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert!(cpu.get_flag(Z_FLAG));
        assert!(cpu.get_flag(C_FLAG)); // no borrow
    }

    #[test]
    fn thumb_add_imm8() {
        // ADD R0, #10
        // bits[15:13]=001, op=10, Rd=0, imm8=10
        // 0b001_10_000_00001010 = 0x300A
        let instr: u16 = 0x300A; // ADD R0, #10
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 5;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 15);
    }

    // ── Format 4: ALU ─────────────────────────────────────────────────────────

    #[test]
    fn thumb_and() {
        // AND R0, R1  (R0 &= R1)
        // bits[15:10]=010000, alu_op=0000, Rs=R1=001, Rd=R0=000
        // 0b010000_0000_001_000 = 0x4008
        let instr: u16 = 0x4008; // AND R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0xFF;
        cpu.regs[1] = 0x0F;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0x0F);
    }

    #[test]
    fn thumb_eor() {
        // EOR R0, R1
        // alu_op=0001
        // 0b010000_0001_001_000 = 0x4048
        let instr: u16 = 0x4048; // EOR R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0b1100;
        cpu.regs[1] = 0b1010;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0b0110);
    }

    #[test]
    fn thumb_orr() {
        // ORR R0, R1  alu_op=1100 = 0xC
        // 0b010000_1100_001_000 = 0x4308
        let instr: u16 = 0x4308; // ORR R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0xF0;
        cpu.regs[1] = 0x0F;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xFF);
    }

    #[test]
    fn thumb_mul() {
        // MUL R0, R1  alu_op=1101=0xD
        // 0b010000_1101_001_000 = 0x4348
        let instr: u16 = 0x4348; // MUL R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 6;
        cpu.regs[1] = 7;
        let cycles = execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 42);
        assert_eq!(cycles, 4); // MUL takes 4 cycles
    }

    #[test]
    fn thumb_neg() {
        // NEG R0, R1  alu_op=1001=0x9
        // 0b010000_1001_001_000 = 0x4248
        let instr: u16 = 0x4248; // NEG R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 5;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0] as i32, -5);
        assert!(cpu.get_flag(N_FLAG));
    }

    #[test]
    fn thumb_mvn() {
        // MVN R0, R1  alu_op=1111=0xF
        // 0b010000_1111_001_000 = 0x43C8
        let instr: u16 = 0x43C8; // MVN R0, R1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x0000_FF00;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], !0x0000_FF00u32);
    }

    // ── Format 5: Hi register ─────────────────────────────────────────────────

    #[test]
    fn thumb_mov_hi() {
        // MOV R8, R0  (hi_op=MOV=10, H1=1, H2=0, Rs=R0=000, Rd=R0 lower=000)
        // bits[15:10]=010001, hi_op=10, H1=1, H2=0, Rs=000, Rd=000
        // 0b010001_10_1_0_000_000 = 0x4680
        let instr: u16 = 0x4680; // MOV R8, R0
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0x1234;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[8], 0x1234);
    }

    #[test]
    fn thumb_bx_to_arm() {
        // BX R0 with bit0=0 → ARM mode
        // hi_op=BX=11, H1=0, H2=0, Rs=R0=000, Rd=ignored(000)
        // 0b010001_11_0_0_000_000 = 0x4700
        let instr: u16 = 0x4700; // BX R0
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0x0300_0000; // bit0=0 → ARM mode
        execute_thumb(&mut cpu, &mut bus, instr);
        assert!(!cpu.in_thumb_mode());
        assert_eq!(cpu.regs[15], 0x0300_0000);
    }

    // ── Format 6: PC-relative load ────────────────────────────────────────────

    #[test]
    fn thumb_ldr_pc() {
        // LDR R0, [PC, #0]  (imm8=0)
        // bits[15:11]=01001, Rd=000, imm8=0x00
        // 0b01001_000_00000000 = 0x4800
        let instr: u16 = 0x4800; // LDR R0, [PC, #0]
        let (mut cpu, mut bus) = make_cpu_bus();
        // addr = (PC & ~2) + imm8*4 = PC (PC must be word-aligned)
        let pc = 0x0300_0010u32;
        cpu.regs[15] = pc;
        bus.write32(pc & !3, 0xCAFE_BABE);
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xCAFE_BABE);
    }

    // ── Format 7: Register offset load/store ──────────────────────────────────

    #[test]
    fn thumb_str_ldr_reg() {
        // STR R0, [R1, R2]  then  LDR R3, [R1, R2]
        // Format 7: bits[15:12]=0101, L=0/1, B=0, bit9=0, Ro=R2=010, Rb=R1=001, Rd=R0=000
        // STR: 0b0101_0_0_0_010_001_000 = 0x5088
        // LDR: 0b0101_1_0_0_010_001_011 = 0x588B
        let str_instr: u16 = 0x5088; // STR R0, [R1, R2]
        let ldr_instr: u16 = 0x588B; // LDR R3, [R1, R2]
        let (mut cpu, mut bus) = make_cpu_bus();
        let base = 0x0300_0000u32;
        cpu.regs[0] = 0xFACE_CAFE;
        cpu.regs[1] = base;
        cpu.regs[2] = 8; // offset
        execute_thumb(&mut cpu, &mut bus, str_instr);
        execute_thumb(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[3], 0xFACE_CAFE);
    }

    // ── Format 9: Immediate offset ────────────────────────────────────────────

    #[test]
    fn thumb_str_ldr_imm() {
        // STR R0, [R1, #8]  then  LDR R2, [R1, #8]
        // Format 9, word: bits[15:13]=011, B=0, L=0/1, offset5=2 (<<2=8), Rb=R1=001, Rd=R0/R2
        // STR: 0b011_0_0_00010_001_000 = 0x6088
        // LDR: 0b011_0_1_00010_001_010 = 0x688A
        let str_instr: u16 = 0x6088; // STR R0, [R1, #8]
        let ldr_instr: u16 = 0x688A; // LDR R2, [R1, #8]
        let (mut cpu, mut bus) = make_cpu_bus();
        let base = 0x0300_0000u32;
        cpu.regs[0] = 0xBEEF_CAFE;
        cpu.regs[1] = base;
        execute_thumb(&mut cpu, &mut bus, str_instr);
        execute_thumb(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[2], 0xBEEF_CAFE);
    }

    // ── Format 11: SP-relative ────────────────────────────────────────────────

    #[test]
    fn thumb_str_ldr_sp() {
        // STR R0, [SP, #0]  then  LDR R1, [SP, #0]
        // Format 11: bits[15:12]=1001, L=0/1, Rd, imm8
        // STR R0: 0b1001_0_000_00000000 = 0x9000
        // LDR R1: 0b1001_1_001_00000000 = 0x9900
        let str_instr: u16 = 0x9000; // STR R0, [SP, #0]
        let ldr_instr: u16 = 0x9900; // LDR R1, [SP, #0]
        let (mut cpu, mut bus) = make_cpu_bus();
        // SP must point to IWRAM
        cpu.regs[13] = 0x0300_0010;
        cpu.regs[0] = 0x1234_5678;
        execute_thumb(&mut cpu, &mut bus, str_instr);
        execute_thumb(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[1], 0x1234_5678);
    }

    // ── Format 13: SP adjust ──────────────────────────────────────────────────

    #[test]
    fn thumb_sp_add() {
        // ADD SP, #4  (imm7=1, offset=4)
        // Format 13: 0b10110000_0_0000001 = 0xB001
        let instr: u16 = 0xB001; // ADD SP, #4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[13] = 0x0300_0100;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[13], 0x0300_0104);
    }

    #[test]
    fn thumb_sp_sub() {
        // SUB SP, #4  (sign=1, imm7=1, offset=4)
        // Format 13: 0b10110000_1_0000001 = 0xB081
        let instr: u16 = 0xB081; // SUB SP, #4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[13] = 0x0300_0100;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[13], 0x0300_00FC);
    }

    // ── Format 14: Push/Pop ───────────────────────────────────────────────────

    #[test]
    fn thumb_push_pop() {
        // PUSH {R0, R1}  then  POP {R2, R3}
        // PUSH: 0b10110100_00000011 = 0xB403 (store_lr=0, rlist=0b00000011 = R0|R1)
        // POP:  0b10111100_00001100 = 0xBC0C (load_pc=0, rlist=0b00001100 = R2|R3)
        let push_instr: u16 = 0xB403; // PUSH {R0, R1}
        let pop_instr: u16 = 0xBC0C;  // POP {R2, R3}
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[13] = 0x0300_0100; // SP in IWRAM
        cpu.regs[0] = 0xAAAA;
        cpu.regs[1] = 0xBBBB;
        execute_thumb(&mut cpu, &mut bus, push_instr);
        let sp_after_push = cpu.regs[13];
        assert_eq!(sp_after_push, 0x0300_00F8); // SP -= 2*4

        execute_thumb(&mut cpu, &mut bus, pop_instr);
        assert_eq!(cpu.regs[2], 0xAAAA);
        assert_eq!(cpu.regs[3], 0xBBBB);
        assert_eq!(cpu.regs[13], 0x0300_0100); // SP restored
    }

    // ── Format 16: Conditional branch ─────────────────────────────────────────

    #[test]
    fn thumb_beq_taken() {
        // BEQ +4  (Z=1 → taken)
        // Format 16: bits[15:12]=1101, cond=0000(EQ), offset8=2 (<<1=4)
        // 0b1101_0000_00000010 = 0xD002
        let instr: u16 = 0xD002; // BEQ +4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0300_0010;
        cpu.set_flag(Z_FLAG, true);
        execute_thumb(&mut cpu, &mut bus, instr);
        // PC += offset<<1 = +4
        assert_eq!(cpu.regs[15], 0x0300_0014);
    }

    #[test]
    fn thumb_beq_not_taken() {
        // BEQ with Z=0 → not taken, PC unchanged
        let instr: u16 = 0xD002; // BEQ +4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0300_0010;
        cpu.set_flag(Z_FLAG, false);
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[15], 0x0300_0010); // PC not advanced by branch
    }

    // ── Format 18: Unconditional branch ──────────────────────────────────────

    #[test]
    fn thumb_b_forward() {
        // B +4  (offset11=2, signed, <<1=4)
        // Format 18: bits[15:11]=11100, offset11=0b00000000010 = 2
        // 0b11100_00000000010 = 0xE002
        let instr: u16 = 0xE002; // B +4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0300_0020;
        execute_thumb(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[15], 0x0300_0024);
    }

    // ── Format 19: BL (two-part) ──────────────────────────────────────────────

    #[test]
    fn thumb_bl() {
        // BL to a known function address.
        // Part 1 (0xF0xx): LR = PC + (signed_offset << 12)
        //   offset11=1 → signed=1 → LR = PC + (1 << 12) = PC + 4096
        // Part 2 (0xF8xx): PC = LR + (offset11 << 1), LR = (PC - 2) | 1
        //   offset11=0 → PC = LR + 0
        let part1: u16 = 0xF001; // high part: offset = 1
        let part2: u16 = 0xF800; // low part: offset = 0
        let (mut cpu, mut bus) = make_cpu_bus();
        let pc_start = 0x0300_0020u32;
        cpu.regs[15] = pc_start;

        // Execute first half-word — sets LR, PC unchanged
        execute_thumb(&mut cpu, &mut bus, part1);
        let expected_lr_after_part1 = pc_start.wrapping_add(0x1000);
        assert_eq!(cpu.regs[14], expected_lr_after_part1);
        assert_eq!(cpu.regs[15], pc_start); // PC still at pc_start

        // Execute second half-word
        // Inside execute_thumb part2: old_pc = cpu.regs[15].wrapping_sub(2)
        let expected_old_pc_inside = pc_start.wrapping_sub(2);
        execute_thumb(&mut cpu, &mut bus, part2);
        // PC = LR + (0 << 1) = expected_lr_after_part1, masked to halfword
        assert_eq!(cpu.regs[15], expected_lr_after_part1 & !1);
        // LR = old_pc | 1 = (pc_start - 2) | 1
        assert_eq!(cpu.regs[14], expected_old_pc_inside | 1);
    }
}
