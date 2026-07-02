//! Joypad input register (0xFF00).
//!
//! Games select which button group to read by writing to bits 4-5:
//!   bit 5 = 0 → select action buttons (A, B, Select, Start)
//!   bit 4 = 0 → select D-pad (Right, Left, Up, Down)
//! Reading returns the selected group in bits 0-3 (active LOW: 0 = pressed).

pub struct Joypad {
    // Button states — true means pressed
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    /// Which group the game selected (bits 4-5 of last write)
    select_bits: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            right: false,
            left: false,
            up: false,
            down: false,
            a: false,
            b: false,
            select: false,
            start: false,
            select_bits: 0x30,
        }
    }

    /// Set button state from the JS side.
    /// Button mapping: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Start, 7=Select
    pub fn set_button(&mut self, button: u8, pressed: bool, interrupt_flag: &mut u8) {
        let was_released = !self.is_pressed(button);
        match button {
            0 => self.right = pressed,
            1 => self.left = pressed,
            2 => self.up = pressed,
            3 => self.down = pressed,
            4 => self.a = pressed,
            5 => self.b = pressed,
            6 => self.start = pressed,
            7 => self.select = pressed,
            _ => {}
        }
        // Trigger joypad interrupt on any high-to-low transition (button press)
        if pressed && was_released {
            *interrupt_flag |= 0x10;
        }
    }

    fn is_pressed(&self, button: u8) -> bool {
        match button {
            0 => self.right,
            1 => self.left,
            2 => self.up,
            3 => self.down,
            4 => self.a,
            5 => self.b,
            6 => self.start,
            7 => self.select,
            _ => false,
        }
    }

    pub fn read(&self) -> u8 {
        let mut result = self.select_bits | 0xC0; // Bits 6-7 always 1

        if self.select_bits & 0x10 == 0 {
            // D-pad selected
            if self.right {
                result &= !0x01;
            }
            if self.left {
                result &= !0x02;
            }
            if self.up {
                result &= !0x04;
            }
            if self.down {
                result &= !0x08;
            }
        }

        if self.select_bits & 0x20 == 0 {
            // Action buttons selected
            if self.a {
                result &= !0x01;
            }
            if self.b {
                result &= !0x02;
            }
            if self.select {
                result &= !0x04;
            }
            if self.start {
                result &= !0x08;
            }
        }

        result
    }

    pub fn write(&mut self, val: u8) {
        // Only bits 4-5 are writable
        self.select_bits = val & 0x30;
    }
}
