use super::{Arm7Tdmi, CpuMode, Bus, N_FLAG, Z_FLAG, C_FLAG, V_FLAG, T_FLAG, I_FLAG};

/// Barrel shifter: applies shift operation and updates carry flag.
fn barrel_shift(cpu: &Arm7Tdmi, operand: u32, shift_type: u32, amount: u32, carry: &mut bool) -> u32 {
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
                if *carry { 0xFFFF_FFFF } else { 0 }
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
                    *carry = if amount == 32 { (operand & 1) != 0 } else { false };
                    0
                } else {
                    *carry = ((operand >> (32 - amount)) & 1) != 0;
                    operand << amount
                }
            }
            1 => {
                // LSR
                if amount >= 32 {
                    *carry = if amount == 32 { (operand >> 31) != 0 } else { false };
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
fn decode_rotated_imm(cpu: &Arm7Tdmi, instruction: u32, carry: &mut bool) -> u32 {
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
            if (bits_27_20 & 0xFB) == 0x12
                || (bits_27_20 & 0xFB) == 0x32
            {
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
            if s_bit { cpu.set_nz(r); cpu.set_flag(C_FLAG, carry); }
            r
        }
        0x1 => {
            // EOR
            let r = op1 ^ op2;
            if s_bit { cpu.set_nz(r); cpu.set_flag(C_FLAG, carry); }
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
            if s_bit { cpu.set_nz(r); cpu.set_flag(C_FLAG, carry); }
            r
        }
        0xD => {
            // MOV
            if s_bit { cpu.set_nz(op2); cpu.set_flag(C_FLAG, carry); }
            op2
        }
        0xE => {
            // BIC
            let r = op1 & !op2;
            if s_bit { cpu.set_nz(r); cpu.set_flag(C_FLAG, carry); }
            r
        }
        0xF => {
            // MVN
            let r = !op2;
            if s_bit { cpu.set_nz(r); cpu.set_flag(C_FLAG, carry); }
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
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        base
    };

    let mut cycles = 1u32;

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
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
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
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        base
    };

    let mut cycles;

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
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
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

    let mut base = cpu.regs[rn];
    let reg_count = reg_list.count_ones();

    // Empty register list: transfer PC only, offset 0x40
    if reg_count == 0 {
        if load {
            cpu.regs[15] = bus.read32(base);
        } else {
            bus.write32(base, cpu.regs[15].wrapping_add(8));
        }
        cpu.regs[rn] = if up { base.wrapping_add(0x40) } else { base.wrapping_sub(0x40) };
        return 3;
    }

    // Calculate start address
    let start_addr = if up {
        base
    } else {
        base.wrapping_sub(reg_count * 4)
    };

    let mut addr = start_addr;
    if !up && pre {
        // Decrement before is same as going up from (base - n*4)
    }

    // Adjust for addressing mode
    let mut current_addr = if up {
        if pre { base.wrapping_add(4) } else { base }
    } else {
        if pre {
            base.wrapping_sub(reg_count * 4)
        } else {
            base.wrapping_sub(reg_count * 4).wrapping_add(4)
        }
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
    if field_mask & 1 != 0 { mask |= 0x0000_00FF; } // control
    if field_mask & 2 != 0 { mask |= 0x0000_FF00; } // extension
    if field_mask & 4 != 0 { mask |= 0x00FF_0000; } // status
    if field_mask & 8 != 0 { mask |= 0xFF00_0000; } // flags

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
