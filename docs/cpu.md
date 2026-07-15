# CPU (SM83)

**Files:** `src/cpu/mod.rs`, `src/cpu/registers.rs`, `src/cpu/opcodes.rs`, `src/cpu/cb_opcodes.rs`

The Game Boy CPU is a Sharp SM83 — often described as "a hybrid between Z80 and 8080." It has the Z80's register layout but uses an 8080-like instruction set with a few Z80 extras (CB prefix, JR, etc.).

## Registers

```
 15        8  7        0
┌──────────┬──────────┐
│    A     │    F     │  AF — Accumulator + Flags
├──────────┼──────────┤
│    B     │    C     │  BC
├──────────┼──────────┤
│    D     │    E     │  DE
├──────────┼──────────┤
│    H     │    L     │  HL — often used as memory pointer
├──────────┴──────────┤
│        SP           │  Stack Pointer
├─────────────────────┤
│        PC           │  Program Counter
└─────────────────────┘
```

### Flags (F register)

| Bit | Flag | Meaning |
|-----|------|---------|
| 7 | Z | Zero — result is 0 |
| 6 | N | Subtract — last op was subtraction (used by DAA) |
| 5 | H | Half-carry — carry from bit 3 to bit 4 |
| 4 | C | Carry — carry from bit 7 (or borrow) |
| 3–0 | — | Always 0 (hardwired) |

`set_af()` masks the lower nibble to enforce this.

### Post-Boot State

The emulator skips the boot ROM and initializes registers to the DMG-01 post-boot state:

```
A=0x01  F=0xB0  B=0x00  C=0x13
D=0x00  E=0xD8  H=0x01  L=0x4D
SP=0xFFFE  PC=0x0100
```

## Instruction Execution

`cpu.step()` does:

1. Check `ime_scheduled` — if set, enable IME (delayed EI effect)
2. If halted and no pending interrupts, return 4 cycles (idle)
3. Fetch opcode at PC (with halt bug: don't advance PC if `halt_bug` is set)
4. Execute the opcode → returns T-cycle count
5. Return total cycles consumed

### Halt Bug

When HALT is executed with IME=0 and an interrupt is pending (IF & IE & 0x1F != 0):
- CPU does **not** halt
- The byte after HALT is read twice (PC fails to increment for the next fetch)
- This is tracked by the `halt_bug` flag

### EI Timing

EI sets `ime_scheduled = true`. IME becomes true at the **start of the next step()**, meaning the instruction immediately after EI still executes with interrupts disabled.

## Opcode Decoding

All 256 base opcodes are decoded in a single `match` statement in `opcodes.rs` (~1,300 lines). CB-prefixed opcodes (another 256) are in `cb_opcodes.rs` (~94 lines).

### Cycle Counts

Some instructions have variable timing:

| Instruction | Taken | Not taken |
|---|---|---|
| JP cc, nn | 16 | 12 |
| JR cc, n | 12 | 8 |
| CALL cc, nn | 24 | 12 |
| RET cc | 20 | 8 |

### CB-Prefixed Opcodes

The CB prefix adds 256 rotate/shift/bit operations. They are decoded by structure:

```
Bits 7-6: Operation group
  00 = rotate/shift (RLC, RRC, RL, RR, SLA, SRA, SWAP, SRL)
  01 = BIT (test bit)
  10 = RES (reset bit)
  11 = SET (set bit)

Bits 5-3: Bit number (for BIT/RES/SET) or operation (for group 00)
Bits 2-0: Register (B=0, C=1, D=2, E=3, H=4, L=5, (HL)=6, A=7)
```

Timing: 8 cycles for register ops, 12 for BIT (HL), 16 for RES/SET (HL).

## ALU Helpers

Defined in `cpu/mod.rs`, these compute results and set flags:

- `alu_add/adc/sub/sbc` — 8-bit arithmetic with flag computation
- `alu_and/xor/or` — bitwise ops (AND always sets H)
- `alu_inc/dec` — 8-bit increment/decrement (doesn't touch C flag)
- `alu_add_hl` — 16-bit add to HL (only sets N, H, C — Z untouched)
- `alu_cp` — compare (same as SUB but doesn't store result)

### Half-Carry

Half-carry is the carry from bit 3 to bit 4. For addition:
```rust
(a & 0xF) + (b & 0xF) > 0xF
```
For subtraction (borrow from bit 4):
```rust
(a & 0xF) < (b & 0xF)
```

### DAA

The most commonly mis-implemented opcode. Adjusts A for BCD after addition or subtraction:

- After ADD (N=0): if H set or lower nibble > 9, add 0x06; if C set or A > 0x99, add 0x60 and set C
- After SUB (N=1): if H set, subtract 0x06; if C set, subtract 0x60

Z is set from result. H is always cleared. N is unchanged. C is set on correction, never cleared.

## Stack Operations

- **PUSH**: SP -= 1, write high byte; SP -= 1, write low byte (high byte at higher address)
- **POP**: read low byte, SP += 1; read high byte, SP += 1
- **POP AF**: masks F bits 3–0 to 0 (enforced by `set_af()`)
