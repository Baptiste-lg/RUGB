/// GBA Timer controller — 4 cascadable 16-bit timers.
///
/// Each timer has: counter, reload value, control register.
/// Prescaler rates: F/1, F/64, F/256, F/1024 (F = 16.78 MHz).
/// Cascade mode: timer N increments when timer N-1 overflows.

const PRESCALER_SHIFTS: [u32; 4] = [0, 6, 8, 10]; // F/1, F/64, F/256, F/1024

#[derive(Clone, Copy)]
pub struct Timer {
    /// Current counter value (16-bit)
    pub counter: u16,
    /// Reload value (written to counter on overflow or start)
    pub reload: u16,
    /// Control register (prescaler, cascade, IRQ, enable)
    pub ctrl: u16,
    /// Internal cycle accumulator for prescaler
    cycles: u32,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            counter: 0,
            reload: 0,
            ctrl: 0,
            cycles: 0,
        }
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.ctrl & 0x80 != 0
    }

    #[inline]
    pub fn cascade(&self) -> bool {
        self.ctrl & 0x04 != 0
    }

    #[inline]
    pub fn irq_enabled(&self) -> bool {
        self.ctrl & 0x40 != 0
    }

    #[inline]
    pub fn prescaler_shift(&self) -> u32 {
        PRESCALER_SHIFTS[(self.ctrl & 3) as usize]
    }
}

pub struct TimerController {
    pub timers: [Timer; 4],
}

impl TimerController {
    pub fn new() -> Self {
        TimerController {
            timers: [Timer::new(); 4],
        }
    }

    /// Advance all timers by `cycles` CPU cycles. Returns IRQ flags to raise.
    pub fn tick(&mut self, cycles: u32) -> u16 {
        let mut irqs = 0u16;
        let mut overflow = [false; 4];

        for i in 0..4 {
            if !self.timers[i].enabled() {
                continue;
            }

            if self.timers[i].cascade() && i > 0 {
                // Cascade mode: increment when previous timer overflows
                if overflow[i - 1] {
                    let (new_val, did_overflow) = self.timers[i].counter.overflowing_add(1);
                    if did_overflow || new_val == 0 {
                        self.timers[i].counter = self.timers[i].reload;
                        overflow[i] = true;
                        if self.timers[i].irq_enabled() {
                            irqs |= 1 << (3 + i); // Timer IRQs are bits 3-6
                        }
                    } else {
                        self.timers[i].counter = new_val;
                    }
                }
            } else {
                // Normal mode: count based on prescaler
                let shift = self.timers[i].prescaler_shift();
                self.timers[i].cycles += cycles;
                let ticks = self.timers[i].cycles >> shift;
                self.timers[i].cycles &= (1 << shift) - 1;

                if ticks > 0 {
                    let remaining = 0x10000u32 - self.timers[i].counter as u32;
                    if ticks >= remaining {
                        // Overflow occurred
                        self.timers[i].counter = self.timers[i].reload;
                        overflow[i] = true;
                        if self.timers[i].irq_enabled() {
                            irqs |= 1 << (3 + i);
                        }
                        // Handle multiple overflows in one tick batch
                        let excess = ticks - remaining;
                        let period = 0x10000u32 - self.timers[i].reload as u32;
                        if period > 0 && excess >= period {
                            let extra_overflows = excess / period;
                            self.timers[i].counter =
                                self.timers[i].reload.wrapping_add((excess % period) as u16);
                            if self.timers[i].irq_enabled() && extra_overflows > 0 {
                                irqs |= 1 << (3 + i);
                            }
                        } else {
                            self.timers[i].counter =
                                self.timers[i].reload.wrapping_add(excess as u16);
                        }
                    } else {
                        self.timers[i].counter =
                            self.timers[i].counter.wrapping_add(ticks as u16);
                    }
                }
            }
        }

        irqs
    }
}
