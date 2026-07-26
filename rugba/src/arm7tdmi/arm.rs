use super::{Arm7Tdmi, Bus, CpuMode, C_FLAG, N_FLAG, T_FLAG, V_FLAG, Z_FLAG};

/// Barrel shifter: applies shift operation and updates carry flag.
#[inline]
fn barrel_shift(
    _cpu: &Arm7Tdmi,
    operand: u32,
    shift_type: u32,
    amount: u32,
    carry: &mut bool,
) -> u32 {
    if amount == 0 {
        // Special cases for zero shift amount (encoded shifts)
        match shift_type {
            0 => operand, // LSL #0 = no shift
            1 => {
                // LSR #0 means LSR #32
                *carry = (operand >> 31) != 0;
                0
            }
            2 => {
                // ASR #0 means ASR #32
                *carry = (operand >> 31) != 0;
                if *carry {
                    0xFFFF_FFFF
                } else {
                    0
                }
            }
            3 => {
                // ROR #0 means RRX (rotate right extended by 1)
                let old_carry = *carry as u32;
                *carry = (operand & 1) != 0;
                (old_carry << 31) | (operand >> 1)
            }
            _ => operand,
        }
    } else {
        match shift_type {
            0 => {
                // LSL
                if amount >= 32 {
                    *carry = if amount == 32 {
                        (operand & 1) != 0
                    } else {
                        false
                    };
                    0
                } else {
                    *carry = ((operand >> (32 - amount)) & 1) != 0;
                    operand << amount
                }
            }
            1 => {
                // LSR
                if amount >= 32 {
                    *carry = if amount == 32 {
                        (operand >> 31) != 0
                    } else {
                        false
                    };
                    0
                } else {
                    *carry = ((operand >> (amount - 1)) & 1) != 0;
                    operand >> amount
                }
            }
            2 => {
                // ASR
                if amount >= 32 {
                    let sign = (operand as i32) >> 31;
                    *carry = sign != 0;
                    sign as u32
                } else {
                    *carry = (((operand as i32) >> (amount - 1)) & 1) != 0;
                    ((operand as i32) >> amount) as u32
                }
            }
            3 => {
                // ROR
                let amount = amount & 31;
                if amount == 0 {
                    *carry = (operand >> 31) != 0;
                    operand
                } else {
                    let result = operand.rotate_right(amount);
                    *carry = (result >> 31) != 0;
                    result
                }
            }
            _ => operand,
        }
    }
}

/// Decode operand 2 for data processing with immediate shift.
#[inline]
fn decode_operand2_imm_shift(cpu: &Arm7Tdmi, instruction: u32, carry: &mut bool) -> u32 {
    let rm = instruction & 0xF;
    let shift_type = (instruction >> 5) & 3;
    let shift_amount = (instruction >> 7) & 0x1F;
    let mut operand = cpu.regs[rm as usize];
    if rm == 15 {
        operand = operand.wrapping_add(8);
    }
    barrel_shift(cpu, operand, shift_type, shift_amount, carry)
}

/// Decode operand 2 for data processing with register shift.
#[inline]
fn decode_operand2_reg_shift(cpu: &Arm7Tdmi, instruction: u32, carry: &mut bool) -> u32 {
    let rm = instruction & 0xF;
    let shift_type = (instruction >> 5) & 3;
    let rs = (instruction >> 8) & 0xF;
    let mut operand = cpu.regs[rm as usize];
    if rm == 15 {
        operand = operand.wrapping_add(12); // extra +4 due to prefetch with reg shift
    }
    let shift_amount = cpu.regs[rs as usize] & 0xFF;
    if shift_amount == 0 {
        return operand;
    }
    barrel_shift(cpu, operand, shift_type, shift_amount, carry)
}

/// Decode immediate operand (rotated 8-bit value).
#[inline]
fn decode_rotated_imm(_cpu: &Arm7Tdmi, instruction: u32, carry: &mut bool) -> u32 {
    let imm = instruction & 0xFF;
    let rotate = ((instruction >> 8) & 0xF) * 2;
    if rotate == 0 {
        imm
    } else {
        let result = imm.rotate_right(rotate);
        *carry = (result >> 31) != 0;
        result
    }
}

/// Add with carry flag output.
#[inline]
fn add_with_flags(a: u32, b: u32, set_flags: bool, cpu: &mut Arm7Tdmi) -> u32 {
    let result = a.wrapping_add(b);
    if set_flags {
        cpu.set_nz(result);
        cpu.set_flag(C_FLAG, result < a); // unsigned overflow
        let va = (a >> 31) & 1;
        let vb = (b >> 31) & 1;
        let vr = (result >> 31) & 1;
        cpu.set_flag(V_FLAG, (va == vb) && (va != vr));
    }
    result
}

/// Subtract with flags (a - b).
#[inline]
fn sub_with_flags(a: u32, b: u32, set_flags: bool, cpu: &mut Arm7Tdmi) -> u32 {
    let result = a.wrapping_sub(b);
    if set_flags {
        cpu.set_nz(result);
        cpu.set_flag(C_FLAG, a >= b); // no borrow
        let va = (a >> 31) & 1;
        let vb = (b >> 31) & 1;
        let vr = (result >> 31) & 1;
        cpu.set_flag(V_FLAG, (va != vb) && (va != vr));
    }
    result
}

/// Execute a single 32-bit ARM instruction. Returns cycles consumed.
pub fn execute_arm(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u32) -> u32 {
    // Check condition (bits 31-28)
    let cond = instruction >> 28;
    if !cpu.check_condition(cond) {
        return 1; // 1S cycle for failed condition
    }

    // Decode by bits 27-25
    let bits_27_25 = (instruction >> 25) & 0x7;
    let bits_27_20 = (instruction >> 20) & 0xFF;
    let bits_7_4 = (instruction >> 4) & 0xF;

    // Branch Exchange (BX): 0001_0010_1111_1111_1111_0001
    if instruction & 0x0FFF_FFF0 == 0x012F_FF10 {
        return exec_bx(cpu, instruction);
    }

    // SWP/SWPB: bits 27-23 = 00010, bits 11-4 = 0000_1001
    if (bits_27_20 & 0xFB) == 0x10 && bits_7_4 == 0x9 {
        return exec_swp(cpu, bus, instruction);
    }

    // Multiply: bits 27-22 = 000000, bits 7-4 = 1001
    if (bits_27_20 & 0xFC) == 0x00 && bits_7_4 == 0x9 {
        return exec_multiply(cpu, instruction);
    }

    // Multiply Long: bits 27-23 = 00001, bits 7-4 = 1001
    if (bits_27_20 & 0xF8) == 0x08 && bits_7_4 == 0x9 {
        return exec_multiply_long(cpu, instruction);
    }

    // Halfword Transfer: bits 27-25 = 000, bit 7=1, bit 4=1, bits 6-5 != 00
    if bits_27_25 == 0 && (bits_7_4 & 0x9) == 0x9 && ((instruction >> 5) & 3) != 0 {
        return exec_halfword_transfer(cpu, bus, instruction);
    }

    match bits_27_25 {
        0b000 | 0b001 => {
            // Data processing or PSR transfer
            let opcode = (instruction >> 21) & 0xF;
            let s_bit = (instruction >> 20) & 1;

            // MRS: bits 27-23 = 00010, bit 21=0, bits 11-0 = 0
            if (bits_27_20 & 0xFB) == 0x10 && (instruction & 0xFFF) == 0 {
                return exec_mrs(cpu, instruction);
            }

            // MSR: bits 27-23 = 00x10, bit 21=1
            if (bits_27_20 & 0xFB) == 0x12 || (bits_27_20 & 0xFB) == 0x32 {
                return exec_msr(cpu, instruction);
            }

            exec_data_processing(cpu, bus, instruction, opcode, s_bit != 0)
        }
        0b010 | 0b011 => {
            // Single data transfer (LDR/STR)
            exec_single_transfer(cpu, bus, instruction)
        }
        0b100 => {
            // Block data transfer (LDM/STM)
            exec_block_transfer(cpu, bus, instruction)
        }
        0b101 => {
            // Branch (B/BL)
            exec_branch(cpu, instruction)
        }
        0b111 => {
            // SWI (bits 27-24 = 1111)
            if (instruction >> 24) & 0xF == 0xF {
                exec_swi(cpu);
            }
            1
        }
        _ => 1, // Coprocessor or undefined
    }
}

fn exec_bx(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let rn = instruction & 0xF;
    let addr = cpu.regs[rn as usize];
    if addr & 1 != 0 {
        // Switch to THUMB
        cpu.set_flag(T_FLAG, true);
        cpu.regs[15] = addr & !1;
    } else {
        cpu.regs[15] = addr & !3;
    }
    3 // 2S + 1N (pipeline flush)
}

fn exec_branch(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let link = (instruction >> 24) & 1;
    // 24-bit signed offset, shifted left 2
    let offset = ((instruction & 0x00FF_FFFF) as i32) << 8 >> 6; // sign extend and *4
    let pc = cpu.regs[15].wrapping_add(8);

    if link != 0 {
        cpu.regs[14] = pc.wrapping_sub(4); // return address = next instruction
    }

    cpu.regs[15] = pc.wrapping_add(offset as u32);
    3 // 2S + 1N
}

fn exec_data_processing(
    cpu: &mut Arm7Tdmi,
    _bus: &mut Bus,
    instruction: u32,
    opcode: u32,
    s_bit: bool,
) -> u32 {
    let rd = ((instruction >> 12) & 0xF) as usize;
    let rn_idx = ((instruction >> 16) & 0xF) as usize;
    let is_imm = (instruction >> 25) & 1 != 0;

    let mut carry = cpu.get_flag(C_FLAG);

    // Get operand 1 (Rn)
    let mut op1 = cpu.regs[rn_idx];
    if rn_idx == 15 {
        op1 = op1.wrapping_add(8);
        if !is_imm && (instruction >> 4) & 1 != 0 {
            op1 = op1.wrapping_add(4); // register shift adds extra +4
        }
    }

    // Get operand 2
    let op2 = if is_imm {
        decode_rotated_imm(cpu, instruction, &mut carry)
    } else if (instruction >> 4) & 1 != 0 {
        // Register shift
        decode_operand2_reg_shift(cpu, instruction, &mut carry)
    } else {
        // Immediate shift
        decode_operand2_imm_shift(cpu, instruction, &mut carry)
    };

    let mut cycles = 1u32;
    let mut write_result = true;

    let result = match opcode {
        0x0 => {
            // AND
            let r = op1 & op2;
            if s_bit {
                cpu.set_nz(r);
                cpu.set_flag(C_FLAG, carry);
            }
            r
        }
        0x1 => {
            // EOR
            let r = op1 ^ op2;
            if s_bit {
                cpu.set_nz(r);
                cpu.set_flag(C_FLAG, carry);
            }
            r
        }
        0x2 => {
            // SUB
            sub_with_flags(op1, op2, s_bit, cpu)
        }
        0x3 => {
            // RSB
            sub_with_flags(op2, op1, s_bit, cpu)
        }
        0x4 => {
            // ADD
            add_with_flags(op1, op2, s_bit, cpu)
        }
        0x5 => {
            // ADC
            let c = cpu.get_flag(C_FLAG) as u32;
            let tmp = op1.wrapping_add(op2).wrapping_add(c);
            if s_bit {
                cpu.set_nz(tmp);
                let carry_out = (op1 as u64) + (op2 as u64) + (c as u64) > 0xFFFF_FFFF;
                cpu.set_flag(C_FLAG, carry_out);
                let va = (op1 >> 31) & 1;
                let vb = (op2 >> 31) & 1;
                let vr = (tmp >> 31) & 1;
                cpu.set_flag(V_FLAG, (va == vb) && (va != vr));
            }
            tmp
        }
        0x6 => {
            // SBC (op1 - op2 - !carry)
            let c = cpu.get_flag(C_FLAG) as u32;
            let tmp = op1.wrapping_sub(op2).wrapping_sub(1 - c);
            if s_bit {
                cpu.set_nz(tmp);
                let borrow = (op1 as u64) < (op2 as u64) + (1 - c as u64);
                cpu.set_flag(C_FLAG, !borrow);
                let va = (op1 >> 31) & 1;
                let vb = (op2 >> 31) & 1;
                let vr = (tmp >> 31) & 1;
                cpu.set_flag(V_FLAG, (va != vb) && (va != vr));
            }
            tmp
        }
        0x7 => {
            // RSC (op2 - op1 - !carry)
            let c = cpu.get_flag(C_FLAG) as u32;
            let tmp = op2.wrapping_sub(op1).wrapping_sub(1 - c);
            if s_bit {
                cpu.set_nz(tmp);
                let borrow = (op2 as u64) < (op1 as u64) + (1 - c as u64);
                cpu.set_flag(C_FLAG, !borrow);
                let va = (op2 >> 31) & 1;
                let vb = (op1 >> 31) & 1;
                let vr = (tmp >> 31) & 1;
                cpu.set_flag(V_FLAG, (va != vb) && (va != vr));
            }
            tmp
        }
        0x8 => {
            // TST
            write_result = false;
            let r = op1 & op2;
            cpu.set_nz(r);
            cpu.set_flag(C_FLAG, carry);
            r
        }
        0x9 => {
            // TEQ
            write_result = false;
            let r = op1 ^ op2;
            cpu.set_nz(r);
            cpu.set_flag(C_FLAG, carry);
            r
        }
        0xA => {
            // CMP
            write_result = false;
            sub_with_flags(op1, op2, true, cpu)
        }
        0xB => {
            // CMN
            write_result = false;
            add_with_flags(op1, op2, true, cpu)
        }
        0xC => {
            // ORR
            let r = op1 | op2;
            if s_bit {
                cpu.set_nz(r);
                cpu.set_flag(C_FLAG, carry);
            }
            r
        }
        0xD => {
            // MOV
            if s_bit {
                cpu.set_nz(op2);
                cpu.set_flag(C_FLAG, carry);
            }
            op2
        }
        0xE => {
            // BIC
            let r = op1 & !op2;
            if s_bit {
                cpu.set_nz(r);
                cpu.set_flag(C_FLAG, carry);
            }
            r
        }
        0xF => {
            // MVN
            let r = !op2;
            if s_bit {
                cpu.set_nz(r);
                cpu.set_flag(C_FLAG, carry);
            }
            r
        }
        _ => 0,
    };

    if write_result {
        if rd == 15 {
            cpu.regs[15] = result & !3;
            if s_bit {
                // Restore CPSR from SPSR
                cpu.cpsr = cpu.spsr();
            }
            cycles = 3; // pipeline flush
        } else {
            cpu.regs[rd] = result;
        }
    }

    // Extra cycle for register-specified shift
    if !is_imm && (instruction >> 4) & 1 != 0 {
        cycles += 1;
    }

    cycles
}

fn exec_multiply(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let rd = ((instruction >> 16) & 0xF) as usize;
    let rn = ((instruction >> 12) & 0xF) as usize;
    let rs = ((instruction >> 8) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;
    let accumulate = (instruction >> 21) & 1 != 0;
    let s_bit = (instruction >> 20) & 1 != 0;

    let mut result = cpu.regs[rm].wrapping_mul(cpu.regs[rs]);
    let mut cycles = multiply_cycles(cpu.regs[rs]);

    if accumulate {
        result = result.wrapping_add(cpu.regs[rn]);
        cycles += 1;
    }

    cpu.regs[rd] = result;

    if s_bit {
        cpu.set_nz(result);
        // C flag is destroyed (unpredictable)
    }

    cycles
}

fn exec_multiply_long(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let rd_hi = ((instruction >> 16) & 0xF) as usize;
    let rd_lo = ((instruction >> 12) & 0xF) as usize;
    let rs = ((instruction >> 8) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;
    let signed = (instruction >> 22) & 1 != 0;
    let accumulate = (instruction >> 21) & 1 != 0;
    let s_bit = (instruction >> 20) & 1 != 0;

    let mut result: u64 = if signed {
        (cpu.regs[rm] as i32 as i64).wrapping_mul(cpu.regs[rs] as i32 as i64) as u64
    } else {
        (cpu.regs[rm] as u64).wrapping_mul(cpu.regs[rs] as u64)
    };

    let mut cycles = multiply_cycles(cpu.regs[rs]) + 1;

    if accumulate {
        let acc = ((cpu.regs[rd_hi] as u64) << 32) | (cpu.regs[rd_lo] as u64);
        result = result.wrapping_add(acc);
        cycles += 1;
    }

    cpu.regs[rd_lo] = result as u32;
    cpu.regs[rd_hi] = (result >> 32) as u32;

    if s_bit {
        cpu.set_nz(cpu.regs[rd_hi]); // N from bit 63, Z from full 64-bit
        let z = result == 0;
        cpu.set_flag(Z_FLAG, z);
        cpu.set_flag(N_FLAG, (result >> 63) != 0);
    }

    cycles
}

/// Determine multiply cycle count based on Rs value.
fn multiply_cycles(rs: u32) -> u32 {
    if rs & 0xFFFF_FF00 == 0 || rs & 0xFFFF_FF00 == 0xFFFF_FF00 {
        2
    } else if rs & 0xFFFF_0000 == 0 || rs & 0xFFFF_0000 == 0xFFFF_0000 {
        3
    } else if rs & 0xFF00_0000 == 0 || rs & 0xFF00_0000 == 0xFF00_0000 {
        4
    } else {
        5
    }
}

fn exec_single_transfer(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u32) -> u32 {
    let is_reg = (instruction >> 25) & 1 != 0;
    let pre = (instruction >> 24) & 1 != 0;
    let up = (instruction >> 23) & 1 != 0;
    let byte = (instruction >> 22) & 1 != 0;
    let writeback = (instruction >> 21) & 1 != 0;
    let load = (instruction >> 20) & 1 != 0;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;

    // Calculate offset
    let offset = if is_reg {
        let rm = (instruction & 0xF) as usize;
        let shift_type = (instruction >> 5) & 3;
        let shift_amount = (instruction >> 7) & 0x1F;
        let mut carry = cpu.get_flag(C_FLAG);
        barrel_shift(cpu, cpu.regs[rm], shift_type, shift_amount, &mut carry)
    } else {
        instruction & 0xFFF
    };

    let mut base = cpu.regs[rn];
    if rn == 15 {
        base = base.wrapping_add(8);
    }

    let addr = if pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        base
    };

    let cycles;

    if load {
        let val = if byte {
            bus.read8(addr) as u32
        } else {
            // Word load: rotate misaligned reads
            let aligned = addr & !3;
            let val = bus.read32(aligned);
            let rot = (addr & 3) * 8;
            val.rotate_right(rot)
        };

        if rd == 15 {
            cpu.regs[15] = val & !3;
            cycles = 5; // 1N + 1S + 1I + pipeline flush
        } else {
            cpu.regs[rd] = val;
            cycles = 3; // 1S + 1N + 1I
        }
    } else {
        // Store
        let val = if rd == 15 {
            cpu.regs[15].wrapping_add(12) // PC + 12 for stores
        } else {
            cpu.regs[rd]
        };

        if byte {
            bus.write8(addr, val as u8);
        } else {
            bus.write32(addr & !3, val);
        }
        cycles = 2; // 2N
    }

    // Post-index: always writeback; Pre-index: writeback only if W bit set
    let final_addr = if !pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        addr
    };

    if (!pre || writeback) && !(load && rd == rn) {
        cpu.regs[rn] = final_addr;
    }

    cycles
}

fn exec_halfword_transfer(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u32) -> u32 {
    let pre = (instruction >> 24) & 1 != 0;
    let up = (instruction >> 23) & 1 != 0;
    let imm_offset = (instruction >> 22) & 1 != 0;
    let writeback = (instruction >> 21) & 1 != 0;
    let load = (instruction >> 20) & 1 != 0;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;
    let sh = (instruction >> 5) & 3;

    let offset = if imm_offset {
        ((instruction >> 4) & 0xF0) | (instruction & 0xF)
    } else {
        cpu.regs[(instruction & 0xF) as usize]
    };

    let mut base = cpu.regs[rn];
    if rn == 15 {
        base = base.wrapping_add(8);
    }

    let addr = if pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        base
    };

    let cycles;

    if load {
        let val = match sh {
            1 => {
                // LDRH - unsigned halfword
                bus.read16(addr & !1) as u32
            }
            2 => {
                // LDRSB - signed byte
                bus.read8(addr) as i8 as i32 as u32
            }
            3 => {
                // LDRSH - signed halfword
                bus.read16(addr & !1) as i16 as i32 as u32
            }
            _ => 0,
        };

        if rd == 15 {
            cpu.regs[15] = val & !3;
            cycles = 5;
        } else {
            cpu.regs[rd] = val;
            cycles = 3; // 1S + 1N + 1I
        }
    } else {
        // STRH
        let val = if rd == 15 {
            cpu.regs[15].wrapping_add(12)
        } else {
            cpu.regs[rd]
        };
        bus.write16(addr & !1, val as u16);
        cycles = 2; // 2N
    }

    // Writeback
    let final_addr = if !pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        addr
    };

    if (!pre || writeback) && !(load && rd == rn) {
        cpu.regs[rn] = final_addr;
    }

    cycles
}

fn exec_block_transfer(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u32) -> u32 {
    let pre = (instruction >> 24) & 1 != 0;
    let up = (instruction >> 23) & 1 != 0;
    let s_bit = (instruction >> 22) & 1 != 0;
    let writeback = (instruction >> 21) & 1 != 0;
    let load = (instruction >> 20) & 1 != 0;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let reg_list = instruction & 0xFFFF;

    let base = cpu.regs[rn];
    let reg_count = reg_list.count_ones();

    // Empty register list: transfer PC only, offset 0x40
    if reg_count == 0 {
        if load {
            cpu.regs[15] = bus.read32(base);
        } else {
            bus.write32(base, cpu.regs[15].wrapping_add(8));
        }
        cpu.regs[rn] = if up {
            base.wrapping_add(0x40)
        } else {
            base.wrapping_sub(0x40)
        };
        return 3;
    }

    // Calculate start address
    let start_addr = if up {
        base
    } else {
        base.wrapping_sub(reg_count * 4)
    };

    let _addr = start_addr;
    if !up && pre {
        // Decrement before is same as going up from (base - n*4)
    }

    // Adjust for addressing mode
    let mut current_addr = if up {
        if pre {
            base.wrapping_add(4)
        } else {
            base
        }
    } else if pre {
        base.wrapping_sub(reg_count * 4)
    } else {
        base.wrapping_sub(reg_count * 4).wrapping_add(4)
    };

    let mut cycles = if load { 2u32 } else { 1u32 };
    let mut first = true;

    for i in 0..16u32 {
        if reg_list & (1 << i) == 0 {
            continue;
        }

        if load {
            let val = bus.read32(current_addr & !3);
            if s_bit && (reg_list & (1 << 15)) != 0 {
                // S bit with R15 in list: restore CPSR from SPSR
                cpu.regs[i as usize] = val;
                if i == 15 {
                    cpu.cpsr = cpu.spsr();
                    cpu.regs[15] = val & !3;
                }
            } else if s_bit {
                // S bit without R15: access user-mode registers
                // Simplified: just write normally (full impl would bank switch)
                cpu.regs[i as usize] = val;
            } else {
                cpu.regs[i as usize] = val;
            }

            if i == 15 {
                cycles += 2; // pipeline flush extra
            }
        } else {
            // Store
            let val = if i == 15 {
                cpu.regs[15].wrapping_add(12)
            } else {
                cpu.regs[i as usize]
            };
            bus.write32(current_addr & !3, val);
        }

        current_addr = current_addr.wrapping_add(4);
        if !first {
            cycles += 1; // 1S per additional register
        }
        first = false;
    }

    // Writeback
    if writeback {
        cpu.regs[rn] = if up {
            base.wrapping_add(reg_count * 4)
        } else {
            base.wrapping_sub(reg_count * 4)
        };
    }

    cycles
}

fn exec_swp(cpu: &mut Arm7Tdmi, bus: &mut Bus, instruction: u32) -> u32 {
    let byte = (instruction >> 22) & 1 != 0;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;

    let addr = cpu.regs[rn];

    if byte {
        let tmp = bus.read8(addr) as u32;
        bus.write8(addr, cpu.regs[rm] as u8);
        cpu.regs[rd] = tmp;
    } else {
        let aligned = addr & !3;
        let tmp = bus.read32(aligned);
        let rot = (addr & 3) * 8;
        let tmp = tmp.rotate_right(rot);
        bus.write32(aligned, cpu.regs[rm]);
        cpu.regs[rd] = tmp;
    }

    4 // 1S + 2N + 1I
}

fn exec_mrs(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let rd = ((instruction >> 12) & 0xF) as usize;
    let use_spsr = (instruction >> 22) & 1 != 0;

    cpu.regs[rd] = if use_spsr { cpu.spsr() } else { cpu.cpsr };

    1
}

fn exec_msr(cpu: &mut Arm7Tdmi, instruction: u32) -> u32 {
    let use_spsr = (instruction >> 22) & 1 != 0;
    let is_imm = (instruction >> 25) & 1 != 0;

    let value = if is_imm {
        let imm = instruction & 0xFF;
        let rotate = ((instruction >> 8) & 0xF) * 2;
        imm.rotate_right(rotate)
    } else {
        cpu.regs[(instruction & 0xF) as usize]
    };

    // Field mask bits (bits 19-16)
    let field_mask = (instruction >> 16) & 0xF;
    let mut mask = 0u32;
    if field_mask & 1 != 0 {
        mask |= 0x0000_00FF; // control
    }
    if field_mask & 2 != 0 {
        mask |= 0x0000_FF00; // extension
    }
    if field_mask & 4 != 0 {
        mask |= 0x00FF_0000; // status
    }
    if field_mask & 8 != 0 {
        mask |= 0xFF00_0000; // flags
    }

    if use_spsr {
        let spsr = cpu.spsr();
        let new_val = (spsr & !mask) | (value & mask);
        // Would need cpu.set_spsr(new_val) - simplified
        // For now just set it via the cpsr path as placeholder
        let _ = new_val;
    } else {
        cpu.cpsr = (cpu.cpsr & !mask) | (value & mask);
    }

    1
}

fn exec_swi(cpu: &mut Arm7Tdmi) {
    cpu.enter_exception(CpuMode::Supervisor, 0x08);
}

#[cfg(test)]
mod tests {
    use super::execute_arm;
    use crate::arm7tdmi::{Arm7Tdmi, C_FLAG, N_FLAG, T_FLAG, V_FLAG, Z_FLAG};
    use crate::bus::Bus;

    fn make_cpu_bus() -> (Arm7Tdmi, Bus) {
        let rom = vec![0u8; 0x200];
        let cpu = Arm7Tdmi::new();
        let bus = Bus::new(rom);
        (cpu, bus)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Build a data-processing instruction (condition = AL = 0xE).
    /// opcode: 4-bit ALU op; s: S-bit; rn: 4-bit; rd: 4-bit; operand2: 12-bit.
    /// Set bit 25 to 1 for immediate operand2.
    fn dp_imm(opcode: u32, s: bool, rn: u32, rd: u32, imm12: u32) -> u32 {
        0xE000_0000 | (1 << 25) | (opcode << 21) | ((s as u32) << 20) | (rn << 16) | (rd << 12) | imm12
    }

    /// Build a data-processing instruction with register Rm (no shift).
    fn dp_reg(opcode: u32, s: bool, rn: u32, rd: u32, rm: u32) -> u32 {
        0xE000_0000 | (opcode << 21) | ((s as u32) << 20) | (rn << 16) | (rd << 12) | rm
    }

    // ── Barrel shifter ────────────────────────────────────────────────────────

    #[test]
    fn lsl_by_0_noop() {
        // MOV R0, R1, LSL #0  →  R0 = R1  (shift_type=0, shift_amount=0)
        // Encoding: cond=AL, I=0, op=MOV(0xD), S=0, Rn=0, Rd=0, shift_amount=0, shift_type=0, Rm=1
        let instr = dp_reg(0xD, false, 0, 0, 1);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xABCD;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xABCD);
    }

    #[test]
    fn lsl_by_1() {
        // MOV R0, R1, LSL #1  →  R0 = R1 << 1
        // shift_amount=1, shift_type=0(LSL) → bits [11:7]=00001, [6:5]=00
        let instr: u32 = 0xE1A0_0081; // MOV R0, R1, LSL #1 (no S)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 5;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 10);
    }

    #[test]
    fn lsl_carry_out() {
        // MOV R0, R1, LSL #1 with S set, R1=0x8000_0000 → carry = 1, result = 0
        // MOVS R0, R1, LSL #1
        let instr: u32 = 0xE1B0_0081; // MOVS R0, R1, LSL #1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x8000_0000;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0);
        assert!(cpu.get_flag(C_FLAG), "Carry should be set from bit 31");
        assert!(cpu.get_flag(Z_FLAG), "Result is zero");
    }

    #[test]
    fn lsr_by_0_means_32() {
        // MOVS R0, R1, LSR #0  →  LSR #32  →  result=0, carry=bit31 of R1
        // shift_type=1(LSR), shift_amount=0 (encodes LSR #32), S=1
        let instr: u32 = 0xE1B0_0021; // MOVS R0, R1, LSR #0 (encodes LSR #32)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x8000_0001; // bit31=1
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0);
        assert!(cpu.get_flag(C_FLAG), "Carry = former bit 31");
    }

    #[test]
    fn asr_sign_preserving() {
        // MOVS R0, R1, ASR #1  →  arithmetic right shift preserves sign
        // shift_type=2(ASR), shift_amount=1
        let instr: u32 = 0xE1B0_00C1; // MOVS R0, R1, ASR #1
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x8000_0000u32; // negative
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xC000_0000); // sign preserved
        assert!(cpu.get_flag(N_FLAG));
    }

    #[test]
    fn ror_basic() {
        // MOVS R0, R1, ROR #4  →  rotate right 4
        // shift_type=3(ROR), shift_amount=4 → bits[11:7]=00100,[6:5]=11
        let instr: u32 = 0xE1B0_0261; // MOVS R0, R1, ROR #4
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x0000_0010; // bit4 set
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0x0000_0001); // rotated to bit0
    }

    // ── Data processing ───────────────────────────────────────────────────────

    #[test]
    fn mov_immediate() {
        // MOV R0, #42
        let instr = dp_imm(0xD, false, 0, 0, 42);
        let (mut cpu, mut bus) = make_cpu_bus();
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 42);
    }

    #[test]
    fn mov_sets_nz() {
        // MOVS R0, #0  →  Z set, N clear
        let instr = dp_imm(0xD, true, 0, 0, 0);
        let (mut cpu, mut bus) = make_cpu_bus();
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0);
        assert!(cpu.get_flag(Z_FLAG));
        assert!(!cpu.get_flag(N_FLAG));
    }

    #[test]
    fn add_no_flags() {
        // ADD R0, R1, R2 (no S bit) — flags not modified
        let instr = dp_reg(0x4, false, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 10;
        cpu.regs[2] = 20;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 30);
        // Flags untouched (C should remain clear)
        assert!(!cpu.get_flag(C_FLAG));
    }

    #[test]
    fn add_with_s_sets_carry() {
        // ADDS R0, R1, R2 — unsigned overflow → carry
        let instr = dp_reg(0x4, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xFFFF_FFFF;
        cpu.regs[2] = 1;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0);
        assert!(cpu.get_flag(C_FLAG));
        assert!(cpu.get_flag(Z_FLAG));
    }

    #[test]
    fn sub_basic() {
        // SUB R0, R1, #5
        let instr = dp_imm(0x2, false, 1, 0, 5);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 10;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 5);
    }

    #[test]
    fn sub_borrow() {
        // SUBS R0, R1, #10  where R1=5 → result negative, C cleared (borrow)
        let instr = dp_imm(0x2, true, 1, 0, 10);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 5;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0] as i32, -5);
        assert!(!cpu.get_flag(C_FLAG)); // borrow occurred
        assert!(cpu.get_flag(N_FLAG));
    }

    #[test]
    fn and_basic() {
        // ANDS R0, R1, R2
        let instr = dp_reg(0x0, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xFF00;
        cpu.regs[2] = 0x0F0F;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0x0F00);
    }

    #[test]
    fn orr_basic() {
        // ORRS R0, R1, R2
        let instr = dp_reg(0xC, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xF0;
        cpu.regs[2] = 0x0F;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xFF);
    }

    #[test]
    fn eor_basic() {
        // EORS R0, R1, R2
        let instr = dp_reg(0x1, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0b1100;
        cpu.regs[2] = 0b1010;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0b0110);
    }

    #[test]
    fn bic_basic() {
        // BICS R0, R1, R2  →  R0 = R1 & ~R2
        let instr = dp_reg(0xE, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xFF;
        cpu.regs[2] = 0x0F;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0xF0);
    }

    #[test]
    fn cmp_sets_flags() {
        // CMP R1, R1  →  result 0, Z set
        // opcode=CMP(0xA), S always set for CMP, Rn=R1, Rd=0 (unused)
        let instr = dp_reg(0xA, true, 1, 0, 1);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 42;
        execute_arm(&mut cpu, &mut bus, instr);
        assert!(cpu.get_flag(Z_FLAG));
        assert!(cpu.get_flag(C_FLAG)); // no borrow
        assert!(!cpu.get_flag(N_FLAG));
    }

    #[test]
    fn tst_sets_z() {
        // TST R1, #0  →  Z set (result=0)
        let instr = dp_imm(0x8, true, 1, 0, 0);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0xFFFF;
        execute_arm(&mut cpu, &mut bus, instr);
        assert!(cpu.get_flag(Z_FLAG));
    }

    #[test]
    fn mvn_inverts() {
        // MVNS R0, R1  →  R0 = ~R1
        let instr = dp_reg(0xF, true, 0, 0, 1);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 0x0000_FF00;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], !0x0000_FF00u32);
    }

    #[test]
    fn adc_with_carry() {
        // ADCS R0, R1, R2  with carry=1  →  R0 = R1 + R2 + 1
        // opcode=ADC(0x5)
        let instr = dp_reg(0x5, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 10;
        cpu.regs[2] = 20;
        cpu.set_flag(C_FLAG, true);
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 31); // 10 + 20 + 1
    }

    #[test]
    fn rsb_reverses() {
        // RSBS R0, R1, R2  →  R0 = R2 - R1  (operands swapped)
        // opcode=RSB(0x3), Rn=R1, Rd=R0, Rm=R2
        let instr = dp_reg(0x3, true, 1, 0, 2);
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 3;
        cpu.regs[2] = 10;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 7); // 10 - 3
    }

    // ── Branches ──────────────────────────────────────────────────────────────

    #[test]
    fn b_forward() {
        // B +8 bytes from current PC (offset field = 0 → PC+8+0 = PC+8)
        // Branch: cond=AL, bits[27:25]=101, L=0, offset24=0
        // exec_branch: pc = cpu.regs[15]+8; target = pc + (offset<<2)
        // offset=0 → target = cpu.regs[15]+8
        let instr: u32 = 0xEA00_0000; // B +0 (i.e. PC+8)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0300_0000;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[15], 0x0300_0008);
    }

    #[test]
    fn b_backward() {
        // B -4 (offset = -1 in 24-bit signed → 0xFFFFFF, shifted: -1 * 4 = -4)
        // But exec_branch adds +8 first: target = (PC+8) + (-4) = PC+4
        let instr: u32 = 0xEAFF_FFFF; // B -4 bytes (signed offset = -1, *4 = -4)
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0300_0010;
        execute_arm(&mut cpu, &mut bus, instr);
        // pc+8 = 0x0300_0018; + (-4) = 0x0300_0014
        assert_eq!(cpu.regs[15], 0x0300_0014);
    }

    #[test]
    fn bl_saves_lr() {
        // BL saves LR = (PC+8) - 4 = PC+4
        let instr: u32 = 0xEB00_0000; // BL offset=0
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[15] = 0x0800_0000;
        execute_arm(&mut cpu, &mut bus, instr);
        // LR = (PC+8) - 4 = PC+4
        assert_eq!(cpu.regs[14], 0x0800_0004);
        assert_eq!(cpu.regs[15], 0x0800_0008);
    }

    #[test]
    fn bx_to_thumb() {
        // BX R0 with R0 bit0=1 → THUMB mode, PC = R0 & ~1
        // BX encoding: 0xE12FFF10 | Rm
        let instr: u32 = 0xE12F_FF10; // BX R0
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0x0300_0001; // bit0 set → THUMB
        execute_arm(&mut cpu, &mut bus, instr);
        assert!(cpu.in_thumb_mode());
        assert_eq!(cpu.regs[15], 0x0300_0000);
    }

    // ── Load / Store ──────────────────────────────────────────────────────────

    #[test]
    fn str_ldr_roundtrip() {
        // STR R1, [R2]  then  LDR R0, [R2]
        // STR: cond=AL, I=0, P=1, U=1, B=0, W=0, L=0, Rn=R2, Rd=R1, offset=0
        // Encoding: 0xE580_0000 | (Rn<<16) | (Rd<<12) | offset
        let str_instr: u32 = 0xE582_1000; // STR R1, [R2, #0]
        let ldr_instr: u32 = 0xE592_0000; // LDR R0, [R2, #0]
        let (mut cpu, mut bus) = make_cpu_bus();
        let addr = 0x0300_0000u32; // IWRAM
        cpu.regs[1] = 0xDEAD_BEEF;
        cpu.regs[2] = addr;
        execute_arm(&mut cpu, &mut bus, str_instr);
        execute_arm(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[0], 0xDEAD_BEEF);
    }

    #[test]
    fn strb_ldrb() {
        // STRB R1, [R2]  then  LDRB R0, [R2]
        let strb_instr: u32 = 0xE5C2_1000; // STRB R1, [R2, #0]
        let ldrb_instr: u32 = 0xE5D2_0000; // LDRB R0, [R2, #0]
        let (mut cpu, mut bus) = make_cpu_bus();
        let addr = 0x0300_0004u32;
        cpu.regs[1] = 0xAB;
        cpu.regs[2] = addr;
        execute_arm(&mut cpu, &mut bus, strb_instr);
        execute_arm(&mut cpu, &mut bus, ldrb_instr);
        assert_eq!(cpu.regs[0], 0xAB);
    }

    #[test]
    fn ldr_pre_index() {
        // LDR R0, [R1, #4]  (pre-indexed, no writeback)
        // 0xE591_0004: P=1, U=1, B=0, W=0, L=1, Rn=R1, Rd=R0, offset=4
        let ldr_instr: u32 = 0xE591_0004; // LDR R0, [R1, #4]
        let (mut cpu, mut bus) = make_cpu_bus();
        let base = 0x0300_0000u32;
        cpu.regs[1] = base;
        // write sentinel at base+4
        bus.write32(base + 4, 0x1234_5678);
        execute_arm(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[0], 0x1234_5678);
        assert_eq!(cpu.regs[1], base); // base not updated (no writeback)
    }

    #[test]
    fn ldr_post_index() {
        // LDR R0, [R1], #4  (post-indexed: load from [R1], then R1 += 4)
        // P=0, U=1, B=0, W=0 (W ignored for post), L=1, Rn=R1, Rd=R0, offset=4
        // Encoding: 0xE491_0004
        let ldr_instr: u32 = 0xE491_0004; // LDR R0, [R1], #4
        let (mut cpu, mut bus) = make_cpu_bus();
        let base = 0x0300_0000u32;
        cpu.regs[1] = base;
        bus.write32(base, 0xCAFE_BABE);
        execute_arm(&mut cpu, &mut bus, ldr_instr);
        assert_eq!(cpu.regs[0], 0xCAFE_BABE);
        assert_eq!(cpu.regs[1], base + 4); // writeback
    }

    // ── Block transfer ────────────────────────────────────────────────────────

    #[test]
    fn stm_ldm_ia() {
        // STMIA R0!, {R1, R2, R3}  then  LDMIA R4!, {R5, R6, R7}
        // STMIA: P=0, U=1, S=0, W=1, L=0, Rn=R0, rlist=0b1110 (R1|R2|R3)
        // Encoding: 0xE8A0_000E
        let stm_instr: u32 = 0xE8A0_000E; // STMIA R0!, {R1, R2, R3}
        // LDMIA: 0xE8B4_00E0 (R4!, {R5, R6, R7} = bits 5,6,7 = 0xE0)
        let ldm_instr: u32 = 0xE8B4_00E0; // LDMIA R4!, {R5, R6, R7}
        let (mut cpu, mut bus) = make_cpu_bus();
        let base = 0x0300_0000u32;
        cpu.regs[0] = base;
        cpu.regs[1] = 0xAABB;
        cpu.regs[2] = 0xCCDD;
        cpu.regs[3] = 0xEEFF;
        execute_arm(&mut cpu, &mut bus, stm_instr);
        assert_eq!(cpu.regs[0], base + 12); // writeback: 3 regs * 4

        cpu.regs[4] = base;
        execute_arm(&mut cpu, &mut bus, ldm_instr);
        assert_eq!(cpu.regs[5], 0xAABB);
        assert_eq!(cpu.regs[6], 0xCCDD);
        assert_eq!(cpu.regs[7], 0xEEFF);
        assert_eq!(cpu.regs[4], base + 12);
    }

    // ── Multiply ──────────────────────────────────────────────────────────────

    #[test]
    fn mul_basic() {
        // MUL R0, R1, R2  (Rd=R0, Rm=R1, Rs=R2)
        // Encoding: cond=AL, 000000[A][S], Rd[19:16], Rn[15:12]=0, Rs[11:8], 1001, Rm[3:0]
        // MUL: bits[27:22]=000000, A=0, S=0
        // 0xE000_0291: Rd=R0(0<<16), Rs=R2(2<<8), 1001, Rm=R1(1)
        let instr: u32 = 0xE000_0291; // MUL R0, R1, R2
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[1] = 6;
        cpu.regs[2] = 7;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 42);
    }

    // ── SWP ───────────────────────────────────────────────────────────────────

    #[test]
    fn swp_exchanges() {
        // SWP R0, R1, [R2]  →  R0 = mem[R2], mem[R2] = R1
        // Encoding: cond=AL, 00010B00, Rn, Rd, 0000_1001, Rm
        // SWP (word): 0xE102_0091  (Rn=R2<<16, Rd=R0<<12, Rm=R1)
        let instr: u32 = 0xE102_0091; // SWP R0, R1, [R2]
        let (mut cpu, mut bus) = make_cpu_bus();
        let addr = 0x0300_0008u32;
        bus.write32(addr, 0x1111_1111);
        cpu.regs[1] = 0x2222_2222;
        cpu.regs[2] = addr;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0x1111_1111); // loaded from memory
        assert_eq!(bus.read32(addr), 0x2222_2222); // stored to memory
    }

    // ── MRS ───────────────────────────────────────────────────────────────────

    #[test]
    fn mrs_reads_cpsr() {
        // MRS R0, CPSR
        // Encoding: 0xE10F_0000
        let instr: u32 = 0xE10F_0000;
        let (mut cpu, mut bus) = make_cpu_bus();
        let expected = cpu.cpsr;
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], expected);
    }

    // ── Condition skip ────────────────────────────────────────────────────────

    #[test]
    fn condition_false_skips() {
        // MOV R0, #99 with NE condition (0x1) when Z=1 → should be skipped
        // NE = condition 0x1 → 0x1_0000000 | dp_imm body
        let instr: u32 = 0x13A0_0063; // NE MOV R0, #99
        let (mut cpu, mut bus) = make_cpu_bus();
        cpu.regs[0] = 0;
        cpu.set_flag(Z_FLAG, true); // Z set → NE fails
        execute_arm(&mut cpu, &mut bus, instr);
        assert_eq!(cpu.regs[0], 0, "Instruction should have been skipped");
    }
}
