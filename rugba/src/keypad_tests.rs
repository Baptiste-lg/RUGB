#[cfg(test)]
mod tests {
    use crate::keypad::Keypad;

    #[test]
    fn test_initial_state_all_released() {
        let kp = Keypad::new();
        assert_eq!(kp.read(), 0x03FF); // All 10 bits set (active-low)
    }

    #[test]
    fn test_press_a() {
        let mut kp = Keypad::new();
        kp.set_button(0, true); // A
        assert_eq!(kp.read(), 0x03FE); // Bit 0 cleared
    }

    #[test]
    fn test_press_multiple() {
        let mut kp = Keypad::new();
        kp.set_button(0, true); // A
        kp.set_button(1, true); // B
        kp.set_button(3, true); // Start
        assert_eq!(kp.read(), 0x03FF & !(1 | 2 | 8)); // Bits 0, 1, 3 cleared
    }

    #[test]
    fn test_release() {
        let mut kp = Keypad::new();
        kp.set_button(0, true);
        assert_eq!(kp.read() & 1, 0); // A pressed
        kp.set_button(0, false);
        assert_eq!(kp.read() & 1, 1); // A released
    }

    #[test]
    fn test_invalid_button_ignored() {
        let mut kp = Keypad::new();
        kp.set_button(10, true); // Invalid — should do nothing
        kp.set_button(255, true);
        assert_eq!(kp.read(), 0x03FF);
    }
}
