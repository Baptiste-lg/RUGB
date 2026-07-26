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
    pub cycles: u32,
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
                        self.timers[i].counter = self.timers[i].counter.wrapping_add(ticks as u16);
                    }
                }
            }
        }

        irqs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a timer with given counter, reload, ctrl, cycles.
    fn make_timer(counter: u16, reload: u16, ctrl: u16) -> Timer {
        Timer {
            counter,
            reload,
            ctrl,
            cycles: 0,
        }
    }

    // --- timer_disabled_no_count ---

    #[test]
    fn timer_disabled_no_count() {
        let mut tc = TimerController::new();
        // ctrl bit 7 = 0 → disabled
        tc.timers[0] = make_timer(0x0010, 0, 0x00);
        tc.tick(1000);
        assert_eq!(
            tc.timers[0].counter, 0x0010,
            "disabled timer must not change"
        );
    }

    // --- prescaler tests ---

    #[test]
    fn prescaler_f1() {
        // shift=0 → each CPU cycle is one counter tick
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0, 0, 0x80); // enabled, prescaler bits=0b00
        tc.tick(5);
        assert_eq!(tc.timers[0].counter, 5);
    }

    #[test]
    fn prescaler_f64() {
        // ctrl bits 0:1 = 0b01 → shift=6, one tick per 64 cycles
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0, 0, 0x81); // enabled | prescaler=1
        tc.tick(128); // 128 / 64 = 2 ticks
        assert_eq!(tc.timers[0].counter, 2);
    }

    #[test]
    fn prescaler_f256() {
        // ctrl bits 0:1 = 0b10 → shift=8, one tick per 256 cycles
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0, 0, 0x82); // enabled | prescaler=2
        tc.tick(512); // 512 / 256 = 2 ticks
        assert_eq!(tc.timers[0].counter, 2);
    }

    #[test]
    fn prescaler_f1024() {
        // ctrl bits 0:1 = 0b11 → shift=10, one tick per 1024 cycles
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0, 0, 0x83); // enabled | prescaler=3
        tc.tick(2048); // 2048 / 1024 = 2 ticks
        assert_eq!(tc.timers[0].counter, 2);
    }

    // --- overflow tests ---

    #[test]
    fn overflow_raises_irq() {
        // counter=0xFFFF, one tick → overflow; IRQ enabled (bit 6)
        let mut tc = TimerController::new();
        // ctrl: enabled(0x80) | irq(0x40) | prescaler F/1(0x00)
        tc.timers[0] = make_timer(0xFFFF, 0, 0xC0);
        let irqs = tc.tick(1);
        // Timer 0 IRQ is bit 3
        assert!(
            irqs & (1 << 3) != 0,
            "timer 0 IRQ should be raised on overflow"
        );
    }

    #[test]
    fn overflow_reloads_value() {
        // counter=0xFFFE, reload=0x1000, 2 ticks → overflow at 0xFFFF+1, resets to 0x1000
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0xFFFE, 0x1000, 0x80);
        tc.tick(2); // tick to 0xFFFF then overflow
                    // After overflow counter = reload + excess = 0x1000 + 0 = 0x1000
        assert_eq!(
            tc.timers[0].counter, 0x1000,
            "counter should reload after overflow"
        );
    }

    // --- cascade ---

    #[test]
    fn cascade_increments_on_overflow() {
        let mut tc = TimerController::new();
        // timer0: enabled, F/1, will overflow with 1 tick from 0xFFFF
        tc.timers[0] = make_timer(0xFFFF, 0, 0x80);
        // timer1: enabled, cascade (bit 2)
        tc.timers[1] = make_timer(5, 0, 0x84); // 0x80 | 0x04
        tc.tick(1); // timer0 overflows → timer1 increments
        assert_eq!(
            tc.timers[1].counter, 6,
            "cascade timer should increment on overflow"
        );
    }

    #[test]
    fn cascade_ignores_own_cycles() {
        let mut tc = TimerController::new();
        // timer1 in cascade mode but timer0 never overflows
        tc.timers[0] = make_timer(0, 0, 0x80); // enabled, won't overflow
        tc.timers[1] = make_timer(10, 0, 0x84); // cascade
        tc.tick(1000); // lots of cycles, but timer0 doesn't overflow (counter = 1000 mod 65536, no OVF)
                       // Actually timer0 counter=1000, no overflow → timer1 stays at 10
        assert_eq!(
            tc.timers[1].counter, 10,
            "cascade timer must not count its own cycles"
        );
    }

    // --- multiple overflows in batch ---

    #[test]
    fn multiple_overflows_in_batch() {
        // reload=0xFFFE → period = 0x10000 - 0xFFFE = 2
        // start from 0xFFFE, tick 6: first overflow after 2 ticks, then 2 more periods = 2 extra overflows
        // excess after first overflow = 6 - 2 = 4; 4/2 = 2 extra; remainder=0
        // final counter = reload + (excess % period) = 0xFFFE + 0 = 0xFFFE
        let mut tc = TimerController::new();
        tc.timers[0] = make_timer(0xFFFE, 0xFFFE, 0x80);
        tc.tick(6);
        assert_eq!(tc.timers[0].counter, 0xFFFE);
    }

    // --- IRQ bit positions ---

    #[test]
    fn irq_bit_positions() {
        for i in 0..4usize {
            let mut tc = TimerController::new();
            // counter at 0xFFFF, enabled + IRQ enabled, F/1
            tc.timers[i] = make_timer(0xFFFF, 0, 0xC0);
            let irqs = tc.tick(1);
            assert!(
                irqs & (1 << (3 + i)) != 0,
                "timer {} IRQ should be bit {}",
                i,
                3 + i
            );
        }
    }
}
