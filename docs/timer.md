# Timer

**File:** `src/timer.rs` (111 lines)

The timer subsystem provides a free-running counter (DIV) and a configurable interrupt timer (TIMA).

## Architecture

```
                  16-bit internal counter (div_counter)
                  ┌──────────────────────────────────┐
                  │ bit15 ◄──── bit9 ◄── bit3 ◄── bit0 │
                  └────────────────┬─────────────────┘
                                   │ (selected bit)
                  DIV = upper 8 bits │
                  (0xFF04)           ▼ falling edge?
                                ┌────────┐
                                │  TIMA  │ ──overflow──► TMA reload + interrupt
                                │(0xFF05)│
                                └────────┘
```

## Registers

| Address | Register | Description |
|---------|----------|-------------|
| 0xFF04 | DIV | Upper 8 bits of internal 16-bit counter. Writing any value resets the whole counter to 0. |
| 0xFF05 | TIMA | Timer counter. Incremented at the rate selected by TAC. Overflows → reloads from TMA + fires interrupt. |
| 0xFF06 | TMA | Timer modulo. Value loaded into TIMA on overflow. |
| 0xFF07 | TAC | Timer control. Bit 2 = enable. Bits 1–0 = frequency select. |

## Frequency Selection

TAC bits 1–0 select which bit of the internal counter triggers TIMA increments:

| TAC | Bit monitored | Frequency | TIMA period |
|-----|---------------|-----------|-------------|
| 00 | Bit 9 | 4,096 Hz | 1,024 T-cycles |
| 01 | Bit 3 | 262,144 Hz | 16 T-cycles |
| 10 | Bit 5 | 65,536 Hz | 64 T-cycles |
| 11 | Bit 7 | 16,384 Hz | 256 T-cycles |

Note the non-sequential order: 00 is the slowest, 01 is the fastest.

## Falling-Edge Detection

TIMA does **not** increment on a simple clock divider. Instead, it increments when the selected bit of the internal counter transitions from 1 to 0. This is implemented by comparing the bit before and after each counter increment:

```rust
let old_bit = (old_counter >> bit_pos) & 1 != 0;
let new_bit = (new_counter >> bit_pos) & 1 != 0;
if old_bit && !new_bit {
    // TIMA increment
}
```

This accurately emulates a hardware glitch: resetting DIV while the selected bit is 1 creates a falling edge, causing TIMA to increment unexpectedly.

## DIV Reset Glitch

Writing **any value** to 0xFF04 resets `div_counter` to 0. If the timer is enabled and the monitored bit was 1, this reset creates a 1→0 transition (falling edge) that increments TIMA.

This is a documented hardware behavior that some games depend on.

## TIMA Overflow

When TIMA overflows (0xFF → 0x00):
1. TIMA is reloaded with the value in TMA
2. Timer interrupt is requested (IF bit 2)

On real hardware there is a 1 M-cycle delay between overflow and reload. This is **not** currently emulated (no known games depend on it).

## Initial State

After boot, `div_counter` is initialized to 0xAB00 (so DIV reads 0xAB), matching DMG hardware measurements.
