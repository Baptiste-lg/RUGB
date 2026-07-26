//! Timer subsystem — DIV, TIMA, TMA, TAC registers.
//!
//! DIV is the upper 8 bits of a free-running 16-bit counter.
//! TIMA increments at a rate selected by TAC and triggers an interrupt on overflow.

use crate::savestate::*;

pub struct Timer {
    /// Internal 16-bit counter. DIV register = upper 8 bits (bits 8-15).
    div_counter: u16,
    /// Timer counter — increments at TAC-selected rate
    tima: u8,
    /// Timer modulo — TIMA reloads from this on overflow
    tma: u8,
    /// Timer control: bit 2 = enable, bits 1-0 = clock select
    tac: u8,
    /// Cached bit mask for the selected TAC frequency (avoids match per cycle)
    bit_mask: u16,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div_counter: 0xAB00, // Post-boot value makes DIV read 0xAB
            tima: 0,
            tma: 0,
            tac: 0,
            bit_mask: 1 << 9, // Default: TAC=00 → bit 9
        }
    }

    /// Advance the timer by `cycles` T-cycles.
    /// The timer interrupt fires when TIMA overflows (bit 2 of interrupt_flag).
    pub fn tick(&mut self, cycles: u32, interrupt_flag: &mut u8) {
        if self.tac & 0x04 == 0 {
            // Timer disabled — just advance DIV, no falling-edge checks needed
            self.div_counter = self.div_counter.wrapping_add(cycles as u16);
            return;
        }

        let mask = self.bit_mask;
        for _ in 0..cycles {
            let old_bit = self.div_counter & mask != 0;
            self.div_counter = self.div_counter.wrapping_add(1);
            let new_bit = self.div_counter & mask != 0;

            if old_bit && !new_bit {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0 {
                    self.tima = self.tma;
                    *interrupt_flag |= 0x04;
                }
            }
        }
    }

    fn update_bit_mask(&mut self) {
        self.bit_mask = match self.tac & 0x03 {
            0 => 1 << 9,
            1 => 1 << 3,
            2 => 1 << 5,
            3 => 1 << 7,
            _ => unreachable!(),
        };
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_u16(d, self.div_counter);
        push_u8(d, self.tima);
        push_u8(d, self.tma);
        push_u8(d, self.tac);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.div_counter = pop_u16(d);
        self.tima = pop_u8(d);
        self.tma = pop_u8(d);
        self.tac = pop_u8(d);
        self.update_bit_mask();
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div_counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper 5 bits read as 1
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8, interrupt_flag: &mut u8) {
        match addr {
            0xFF04 => {
                // If timer is enabled and selected bit was 1, resetting creates a falling edge
                if self.tac & 0x04 != 0 && self.div_counter & self.bit_mask != 0 {
                    self.tima = self.tima.wrapping_add(1);
                    if self.tima == 0 {
                        self.tima = self.tma;
                        *interrupt_flag |= 0x04;
                    }
                }
                self.div_counter = 0;
            }
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => {
                self.tac = val & 0x07;
                self.update_bit_mask();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Return a Timer whose div_counter is forced to 0 (simplifies cycle maths).
    fn timer_zero_div() -> Timer {
        let mut t = Timer::new();
        let mut irq = 0u8;
        // Writing to 0xFF04 resets div_counter to 0.
        t.write(0xFF04, 0, &mut irq);
        t
    }

    // -----------------------------------------------------------------------
    // Initialisation
    // -----------------------------------------------------------------------

    #[test]
    fn new_div_reads_initial() {
        // Post-boot div_counter = 0xAB00 → upper byte = 0xAB.
        let t = Timer::new();
        assert_eq!(t.read(0xFF04), 0xAB);
    }

    // -----------------------------------------------------------------------
    // DIV always advances
    // -----------------------------------------------------------------------

    #[test]
    fn div_advances_always() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        // Timer disabled (TAC = 0x00).
        // Tick 256 cycles → div_counter = 256 = 0x0100 → upper byte = 0x01.
        t.tick(256, &mut irq);
        assert_eq!(
            t.read(0xFF04),
            0x01,
            "DIV must advance even when timer is disabled"
        );
    }

    // -----------------------------------------------------------------------
    // TIMA disabled
    // -----------------------------------------------------------------------

    #[test]
    fn tima_not_incremented_when_disabled() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        // TAC bit 2 = 0 → timer disabled.
        t.write(0xFF07, 0x00, &mut irq);
        t.tick(100_000, &mut irq);
        assert_eq!(t.read(0xFF05), 0, "TIMA must stay 0 when timer is disabled");
    }

    // -----------------------------------------------------------------------
    // TAC clock frequencies
    // -----------------------------------------------------------------------

    #[test]
    fn tac_clock_00_1024_cycles() {
        // freq=0 → bit 9 of div_counter → falling edge every 1024 T-cycles.
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x04, &mut irq); // enable, freq 0
                                         // After 1023 cycles TIMA must still be 0.
        t.tick(1023, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            0,
            "TIMA should not increment before 1024 cycles"
        );
        // The 1024th cycle crosses the falling edge.
        t.tick(1, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            1,
            "TIMA should be 1 after exactly 1024 cycles"
        );
    }

    #[test]
    fn tac_clock_01_16_cycles() {
        // freq=1 → bit 3 → falling edge every 16 T-cycles.
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x05, &mut irq); // enable, freq 1
        t.tick(15, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            0,
            "TIMA should not increment before 16 cycles"
        );
        t.tick(1, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            1,
            "TIMA should be 1 after exactly 16 cycles"
        );
    }

    #[test]
    fn tac_clock_10_64_cycles() {
        // freq=2 → bit 5 → falling edge every 64 T-cycles.
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x06, &mut irq); // enable, freq 2
        t.tick(63, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            0,
            "TIMA should not increment before 64 cycles"
        );
        t.tick(1, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            1,
            "TIMA should be 1 after exactly 64 cycles"
        );
    }

    #[test]
    fn tac_clock_11_256_cycles() {
        // freq=3 → bit 7 → falling edge every 256 T-cycles.
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x07, &mut irq); // enable, freq 3
        t.tick(255, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            0,
            "TIMA should not increment before 256 cycles"
        );
        t.tick(1, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            1,
            "TIMA should be 1 after exactly 256 cycles"
        );
    }

    // -----------------------------------------------------------------------
    // TIMA overflow
    // -----------------------------------------------------------------------

    #[test]
    fn tima_overflow_fires_interrupt() {
        // Set TIMA to 0xFF and TMA to 0x00, then tick one more increment.
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x05, &mut irq); // enable, freq 1 (16 cycles/tick)
        t.write(0xFF05, 0xFF, &mut irq); // TIMA = 255
        t.tick(16, &mut irq); // one more increment → overflow
        assert_eq!(
            irq & 0x04,
            0x04,
            "timer interrupt (bit 2) must fire on TIMA overflow"
        );
    }

    #[test]
    fn tima_overflow_reloads_tma() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF07, 0x05, &mut irq); // enable, freq 1
        t.write(0xFF06, 0x42, &mut irq); // TMA = 0x42
        t.write(0xFF05, 0xFF, &mut irq); // TIMA = 255
        t.tick(16, &mut irq); // overflow
        assert_eq!(
            t.read(0xFF05),
            0x42,
            "TIMA must reload from TMA after overflow"
        );
    }

    // -----------------------------------------------------------------------
    // DIV write resets counter
    // -----------------------------------------------------------------------

    #[test]
    fn div_write_resets_to_zero() {
        let mut t = Timer::new(); // div_counter starts at 0xAB00
        let mut irq = 0u8;
        assert_eq!(t.read(0xFF04), 0xAB);
        t.write(0xFF04, 0, &mut irq); // any write resets div_counter to 0
        assert_eq!(t.read(0xFF04), 0x00, "DIV must read 0 after write");
    }

    #[test]
    fn div_reset_falling_edge_increments_tima() {
        // Set up: timer enabled with freq 1 (bit_mask = bit 3).
        // Force div_counter so that bit 3 is set, then write to 0xFF04.
        // The falling edge should increment TIMA by 1.
        let mut t = timer_zero_div(); // div_counter = 0
        let mut irq = 0u8;
        t.write(0xFF07, 0x05, &mut irq); // enable, freq 1 (bit 3)
                                         // Advance until bit 3 is set: tick 8 cycles → div_counter = 8 = 0b1000.
        t.tick(8, &mut irq);
        assert_eq!(t.read(0xFF05), 0, "no overflow yet");
        let tima_before = t.read(0xFF05);
        // Writing to DIV resets div_counter to 0, creating a falling edge on bit 3.
        t.write(0xFF04, 0, &mut irq);
        assert_eq!(
            t.read(0xFF05),
            tima_before + 1,
            "falling edge on DIV reset must increment TIMA"
        );
    }

    // -----------------------------------------------------------------------
    // Direct register writes
    // -----------------------------------------------------------------------

    #[test]
    fn write_tima_directly() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF05, 0x55, &mut irq);
        assert_eq!(t.read(0xFF05), 0x55);
    }

    #[test]
    fn write_tma_directly() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        t.write(0xFF06, 0xAA, &mut irq);
        assert_eq!(t.read(0xFF06), 0xAA);
    }

    // -----------------------------------------------------------------------
    // Save / load state
    // -----------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip() {
        let mut t = timer_zero_div();
        let mut irq = 0u8;
        // Set up a distinctive state.
        t.write(0xFF07, 0x07, &mut irq); // enable, freq 3
        t.write(0xFF05, 0x12, &mut irq); // TIMA
        t.write(0xFF06, 0x34, &mut irq); // TMA
        t.tick(128, &mut irq); // advance div_counter a bit

        let mut buf: Vec<u8> = Vec::new();
        t.save_state(&mut buf);

        let mut t2 = Timer::new();
        let mut slice: &[u8] = &buf;
        t2.load_state(&mut slice);

        assert_eq!(t.read(0xFF04), t2.read(0xFF04), "DIV mismatch");
        assert_eq!(t.read(0xFF05), t2.read(0xFF05), "TIMA mismatch");
        assert_eq!(t.read(0xFF06), t2.read(0xFF06), "TMA mismatch");
        assert_eq!(t.read(0xFF07), t2.read(0xFF07), "TAC mismatch");

        // Verify that the loaded timer behaves identically (bit_mask restored).
        let mut irq1 = 0u8;
        let mut irq2 = 0u8;
        t.tick(256, &mut irq1);
        t2.tick(256, &mut irq2);
        assert_eq!(
            t.read(0xFF05),
            t2.read(0xFF05),
            "TIMA must evolve identically after load"
        );
    }
}
