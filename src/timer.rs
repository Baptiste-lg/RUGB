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
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div_counter: 0xAB00, // Post-boot value makes DIV read 0xAB
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    /// Advance the timer by `cycles` T-cycles.
    /// The timer interrupt fires when TIMA overflows (bit 2 of interrupt_flag).
    pub fn tick(&mut self, cycles: u32, interrupt_flag: &mut u8) {
        for _ in 0..cycles {
            let old_counter = self.div_counter;
            self.div_counter = self.div_counter.wrapping_add(1);

            if self.tac & 0x04 == 0 {
                continue; // Timer disabled
            }

            // Check for falling edge on the bit selected by TAC
            let bit_mask = self.selected_bit_mask();
            let old_bit = old_counter & bit_mask != 0;
            let new_bit = self.div_counter & bit_mask != 0;

            if old_bit && !new_bit {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0 {
                    self.tima = self.tma;
                    *interrupt_flag |= 0x04; // Timer interrupt
                }
            }
        }
    }

    /// Which bit of div_counter to watch for a falling edge.
    /// TAC bits 1-0: 00=bit9 (4096Hz), 01=bit3 (262144Hz), 10=bit5 (65536Hz), 11=bit7 (16384Hz)
    fn selected_bit_mask(&self) -> u16 {
        match self.tac & 0x03 {
            0 => 1 << 9,
            1 => 1 << 3,
            2 => 1 << 5,
            3 => 1 << 7,
            _ => unreachable!(),
        }
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
                if self.tac & 0x04 != 0 && self.div_counter & self.selected_bit_mask() != 0 {
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
            0xFF07 => self.tac = val & 0x07,
            _ => {}
        }
    }
}
