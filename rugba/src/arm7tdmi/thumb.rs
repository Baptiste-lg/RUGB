use super::{Arm7Tdmi, Bus, CpuMode, C_FLAG, N_FLAG, T_FLAG, V_FLAG, Z_FLAG};

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
                        let result = if rot == 0 {
                            cpu.set_flag(C_FLAG, a >> 31 != 0);
                            a
                        } else {
                            cpu.set_flag(C_FLAG, (a >> (rot - 1)) & 1 != 0);
                            a.rotate_right(rot)
                        };
                        result
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
