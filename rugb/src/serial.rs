//! Serial link cable subsystem — SB (0xFF01) and SC (0xFF02).
//!
//! Supports master mode (internal clock, bit 0 = 1) and slave mode (external clock, bit 0 = 0).
//! A completed transfer raises interrupt bit 3 (serial interrupt).

use crate::savestate::*;

pub struct Serial {
    pub sb: u8,                      // 0xFF01 — shift register
    pub sc: u8,                      // 0xFF02 — control register
    cycles_remaining: u32,           // countdown to transfer completion (0 = idle)
    pending_remote_byte: Option<u8>, // byte from remote peer
}

impl Serial {
    const TRANSFER_CYCLES: u32 = 512;

    pub fn new() -> Self {
        Serial {
            sb: 0,
            sc: 0x7E,
            cycles_remaining: 0,
            pending_remote_byte: None,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF01 => self.sb,
            0xFF02 => self.sc | 0x7E, // bits 1-6 always read as 1
            _ => 0xFF,
        }
    }

    /// Write to serial registers.
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF01 => self.sb = val,
            0xFF02 => {
                self.sc = val;
                // Master mode: bit 7 (start) + bit 0 (internal clock) both set
                if val & 0x81 == 0x81 {
                    self.cycles_remaining = Self::TRANSFER_CYCLES;
                }
                // Slave mode (0x80): waits for receive_remote_byte to trigger transfer
            }
            _ => {}
        }
    }

    /// Advance by `cycles`. Returns Some(sent_byte) when a transfer completes.
    pub fn tick(&mut self, cycles: u32, interrupt_flag: &mut u8) -> Option<u8> {
        if self.cycles_remaining == 0 {
            return None;
        }

        if cycles >= self.cycles_remaining {
            self.cycles_remaining = 0;
            let sent = self.sb;
            self.sb = self.pending_remote_byte.take().unwrap_or(0xFF);
            self.sc &= !0x80; // clear transfer flag
            *interrupt_flag |= 0x08; // serial interrupt
            Some(sent)
        } else {
            self.cycles_remaining -= cycles;
            None
        }
    }

    /// Inject a byte from the remote peer. If we're in slave mode (waiting for
    /// external clock), this triggers the transfer.
    pub fn receive_remote_byte(&mut self, byte: u8) {
        self.pending_remote_byte = Some(byte);
        // If slave mode is armed (SC = 0x80: start bit set, external clock)
        if self.sc & 0x81 == 0x80 && self.cycles_remaining == 0 {
            self.cycles_remaining = 4; // near-instant completion for slave
        }
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_u8(d, self.sb);
        push_u8(d, self.sc);
        push_u32(d, self.cycles_remaining);
        push_bool(d, self.pending_remote_byte.is_some());
        push_u8(d, self.pending_remote_byte.unwrap_or(0));
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.sb = pop_u8(d);
        self.sc = pop_u8(d);
        self.cycles_remaining = pop_u32(d);
        let has_pending = pop_bool(d);
        let pending_val = pop_u8(d);
        self.pending_remote_byte = if has_pending { Some(pending_val) } else { None };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_transfer_starts() {
        let mut s = Serial::new();
        s.write(0xFF02, 0x81);
        assert_eq!(
            s.cycles_remaining, 512,
            "master transfer should arm 512 cycles"
        );
    }

    #[test]
    fn tick_does_not_complete_early() {
        let mut s = Serial::new();
        s.write(0xFF02, 0x81);
        let mut iflag = 0u8;
        let result = s.tick(511, &mut iflag);
        assert!(result.is_none(), "should not complete before 512 cycles");
        assert_eq!(iflag & 0x08, 0, "no serial interrupt before completion");
    }

    #[test]
    fn tick_completes_at_512() {
        let mut s = Serial::new();
        s.write(0xFF02, 0x81);
        let mut iflag = 0u8;
        let result = s.tick(512, &mut iflag);
        assert!(result.is_some(), "should complete at 512 cycles");
        assert_eq!(iflag & 0x08, 0x08, "serial interrupt bit 3 must be set");
        assert_eq!(s.sc & 0x80, 0, "SC bit 7 (transfer flag) must be cleared");
    }

    #[test]
    fn completed_transfer_swaps_bytes() {
        let mut s = Serial::new();
        s.write(0xFF01, 0xAB); // SB = outgoing byte
        s.receive_remote_byte(0x42);
        s.write(0xFF02, 0x81); // start master transfer
        let mut iflag = 0u8;
        let sent = s.tick(512, &mut iflag);
        assert_eq!(sent, Some(0xAB), "returned byte should be the sent SB");
        assert_eq!(s.sb, 0x42, "SB should be replaced by remote byte");
    }

    #[test]
    fn no_remote_gives_0xff() {
        let mut s = Serial::new();
        s.write(0xFF01, 0xAB);
        s.write(0xFF02, 0x81);
        let mut iflag = 0u8;
        s.tick(512, &mut iflag);
        assert_eq!(
            s.sb, 0xFF,
            "no remote byte → SB should be 0xFF after transfer"
        );
    }

    #[test]
    fn slave_fires_on_receive() {
        let mut s = Serial::new();
        s.write(0xFF02, 0x80); // slave mode: start bit set, external clock
        s.receive_remote_byte(0x37);
        assert!(
            s.cycles_remaining > 0,
            "slave should arm countdown on receive"
        );
        let mut iflag = 0u8;
        let result = s.tick(4, &mut iflag);
        assert!(
            result.is_some(),
            "slave transfer should complete after 4 cycles"
        );
        assert_eq!(s.sb, 0x37, "SB should contain received byte");
        assert_eq!(iflag & 0x08, 0x08, "serial interrupt must fire");
    }

    #[test]
    fn save_load_roundtrip() {
        let mut s = Serial::new();
        s.write(0xFF01, 0xCD);
        s.write(0xFF02, 0x81);
        s.receive_remote_byte(0x55);
        let mut iflag = 0u8;
        s.tick(100, &mut iflag); // partial progress

        let mut buf: Vec<u8> = Vec::new();
        s.save_state(&mut buf);

        let mut s2 = Serial::new();
        let mut slice: &[u8] = &buf;
        s2.load_state(&mut slice);

        assert_eq!(s.sb, s2.sb, "SB mismatch");
        assert_eq!(s.sc, s2.sc, "SC mismatch");
        assert_eq!(
            s.cycles_remaining, s2.cycles_remaining,
            "cycles_remaining mismatch"
        );
        assert_eq!(
            s.pending_remote_byte, s2.pending_remote_byte,
            "pending_remote_byte mismatch"
        );
    }
}
