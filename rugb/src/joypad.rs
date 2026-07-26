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
        if button > 7 {
            return;
        }
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
        let mut result = self.select_bits | 0xCF; // Bits 6-7 always 1, bits 0-3 start high (not pressed)

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

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------
    fn new_pad() -> Joypad {
        Joypad::new()
    }

    // -----------------------------------------------------------------------
    // Initialisation
    // -----------------------------------------------------------------------

    #[test]
    fn new_default_select_bits() {
        // After construction, select_bits must be 0x30 (neither group selected).
        // read() = 0x30 | 0xCF = 0xFF when no buttons are pressed.
        let pad = new_pad();
        assert_eq!(pad.read(), 0xFF);
    }

    // -----------------------------------------------------------------------
    // Group selection
    // -----------------------------------------------------------------------

    #[test]
    fn select_dpad_group() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        // write(0x20): bit 4 = 0 → D-pad selected, bit 5 = 1 → action deselected.
        pad.write(0x20);
        // Press Right (button 0) → should clear bit 0.
        pad.set_button(0, true, &mut irq);
        let val = pad.read();
        assert_eq!(val & 0x01, 0, "bit 0 should be clear when Right is pressed");
    }

    #[test]
    fn select_action_group() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        // write(0x10): bit 5 = 0 → action selected, bit 4 = 1 → D-pad deselected.
        pad.write(0x10);
        // Press A (button 4) → maps to bit 0 of action group.
        pad.set_button(4, true, &mut irq);
        let val = pad.read();
        assert_eq!(val & 0x01, 0, "bit 0 should be clear when A is pressed");
    }

    #[test]
    fn both_groups_selected() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        // write(0x00): both bit 4 and bit 5 cleared → both groups active.
        pad.write(0x00);
        // Press Right (button 0, D-pad) and A (button 4, action) — both map to bit 0.
        pad.set_button(0, true, &mut irq);
        pad.set_button(4, true, &mut irq);
        let val = pad.read();
        assert_eq!(val & 0x01, 0, "bit 0 clear via D-pad");
        // Press Down (button 3) and Start (button 6) — map to bit 3.
        pad.set_button(3, true, &mut irq);
        pad.set_button(6, true, &mut irq);
        assert_eq!(pad.read() & 0x08, 0, "bit 3 clear via both groups");
    }

    #[test]
    fn neither_group_selected() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        // write(0x30): both select bits set → neither group active.
        pad.write(0x30);
        // Press every button.
        for b in 0..8u8 {
            pad.set_button(b, true, &mut irq);
        }
        // Bits 0-3 should still read as 1 (not pressed output) since no group is selected.
        let val = pad.read();
        assert_eq!(val & 0x0F, 0x0F, "all bits 0-3 remain 1 when no group is selected");
    }

    // -----------------------------------------------------------------------
    // D-pad bit mapping
    // -----------------------------------------------------------------------

    #[test]
    fn dpad_right_left_up_down() {
        // Buttons 0-3 map to bits 0-3 when D-pad group is selected.
        let mappings: [(u8, u8); 4] = [(0, 0x01), (1, 0x02), (2, 0x04), (3, 0x08)];
        for (button, bit) in mappings {
            let mut pad = new_pad();
            let mut irq = 0u8;
            pad.write(0x20); // select D-pad only
            pad.set_button(button, true, &mut irq);
            assert_eq!(
                pad.read() & bit,
                0,
                "button {button} should clear bit {bit:#04x}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Action button bit mapping
    // -----------------------------------------------------------------------

    #[test]
    fn action_a_b_select_start() {
        // Buttons 4-7 map to bits 0-3 when action group is selected.
        // 4=A→bit0, 5=B→bit1, 7=Select→bit2, 6=Start→bit3
        let mappings: [(u8, u8); 4] = [(4, 0x01), (5, 0x02), (7, 0x04), (6, 0x08)];
        for (button, bit) in mappings {
            let mut pad = new_pad();
            let mut irq = 0u8;
            pad.write(0x10); // select action only
            pad.set_button(button, true, &mut irq);
            assert_eq!(
                pad.read() & bit,
                0,
                "button {button} should clear bit {bit:#04x}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Release behaviour
    // -----------------------------------------------------------------------

    #[test]
    fn releasing_restores_bit() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        pad.write(0x20); // D-pad selected
        pad.set_button(0, true, &mut irq); // press Right
        assert_eq!(pad.read() & 0x01, 0, "bit 0 clear while pressed");
        pad.set_button(0, false, &mut irq); // release Right
        assert_eq!(pad.read() & 0x01, 0x01, "bit 0 restored after release");
    }

    // -----------------------------------------------------------------------
    // Interrupt behaviour
    // -----------------------------------------------------------------------

    #[test]
    fn press_sets_interrupt_flag() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        pad.set_button(0, true, &mut irq);
        assert_eq!(irq & 0x10, 0x10, "interrupt flag bit 4 must be set on press");
    }

    #[test]
    fn no_interrupt_on_release() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        pad.set_button(0, true, &mut irq);
        irq = 0; // clear flag
        pad.set_button(0, false, &mut irq);
        assert_eq!(irq & 0x10, 0, "releasing must not set interrupt flag");
    }

    #[test]
    fn no_interrupt_on_double_press() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        pad.set_button(0, true, &mut irq); // first press — sets flag
        irq = 0;
        pad.set_button(0, true, &mut irq); // already pressed — must not re-fire
        assert_eq!(irq & 0x10, 0, "pressing an already-pressed button must not fire interrupt");
    }

    // -----------------------------------------------------------------------
    // Upper-bit masking
    // -----------------------------------------------------------------------

    #[test]
    fn write_only_bits4_5() {
        let mut pad = new_pad();
        // Writing with bits 6-7 set: the write masks them out (& 0x30),
        // and read() always OR-in 0xCF which forces bits 6-7 to 1.
        pad.write(0xFF); // only bits 4-5 survive → select_bits = 0x30
        let val = pad.read();
        // Bits 6-7 must always be 1.
        assert_eq!(val & 0xC0, 0xC0, "bits 6-7 must always read as 1");
        // Bits 4-5 should reflect the masked write (0x30 → both select bits set).
        assert_eq!(val & 0x30, 0x30, "bits 4-5 should reflect the masked select bits");
        // Bits 0-3 high since neither group is active.
        assert_eq!(val & 0x0F, 0x0F, "bits 0-3 high when no group selected");
    }

    // -----------------------------------------------------------------------
    // Out-of-range button index
    // -----------------------------------------------------------------------

    #[test]
    fn invalid_button_index_ignored() {
        let mut pad = new_pad();
        let mut irq = 0u8;
        pad.write(0x00); // both groups selected so any bit change would show
        pad.set_button(8, true, &mut irq);
        // No bit should have changed and no interrupt should fire.
        assert_eq!(irq & 0x10, 0, "invalid index must not fire interrupt");
        // All bits 0-3 remain 1 (nothing pressed).
        assert_eq!(pad.read() & 0x0F, 0x0F, "invalid index must not alter button state");
    }
}
