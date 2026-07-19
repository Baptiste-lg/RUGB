/// GBA keypad — 10 buttons, active-low at KEYINPUT (0x04000130).
///
/// Bit layout: 0=A, 1=B, 2=Select, 3=Start, 4=Right, 5=Left, 6=Up, 7=Down, 8=R, 9=L
pub struct Keypad {
    /// Active-low state (0 = pressed). Bits 0-9 used, bits 10-15 unused (read as 0).
    state: u16,
}

impl Keypad {
    pub fn new() -> Self {
        Keypad { state: 0x03FF } // All released
    }

    /// Set a button state. `button` is the bit index (0=A, 1=B, ..., 9=L).
    pub fn set_button(&mut self, button: u8, pressed: bool) {
        if button > 9 {
            return;
        }
        if pressed {
            self.state &= !(1 << button); // Active-low: 0 = pressed
        } else {
            self.state |= 1 << button;
        }
    }

    /// Read KEYINPUT register value.
    pub fn read(&self) -> u16 {
        self.state
    }
}
