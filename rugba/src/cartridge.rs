/// GBA cartridge backup detection and Flash/EEPROM/SRAM handling.
///
/// Backup type is detected by scanning the ROM for identification strings:
/// - "SRAM_V" → 32 KB SRAM (byte-addressed at 0x0E000000)
/// - "FLASH_V" / "FLASH512_V" → 64 KB Flash
/// - "FLASH1M_V" → 128 KB Flash
/// - "EEPROM_V" → 512B or 8KB EEPROM (serial access)

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackupType {
    None,
    Sram,      // 32 KB, byte access
    Flash64,   // 64 KB, command protocol
    Flash128,  // 128 KB, command protocol
    Eeprom512, // 512 bytes, serial
    Eeprom8K,  // 8 KB, serial
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EepromState {
    Idle,
    ReadingCommand,
    ReadReady,
    WritingData,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FlashState {
    Ready,
    Cmd1,    // Received 0xAA at 0x5555
    Cmd2,    // Received 0x55 at 0x2AAA
    IdMode,  // Chip identification mode
    Erase,   // Waiting for erase command
    Write,   // Single byte write mode
    BankSel, // Bank select (128 KB only)
}

pub struct Cartridge {
    pub backup_type: BackupType,
    pub sram: Vec<u8>,
    flash_state: FlashState,
    flash_bank: usize, // 0 or 1 (for 128 KB)
    pub dirty: bool,   // Set when save data is modified
    // EEPROM serial state
    eeprom_bits_in: u64,  // Shift register for incoming bits
    eeprom_bit_count: u8, // How many bits received so far
    eeprom_state: EepromState,
    eeprom_read_data: u64, // Data being clocked out on reads
    eeprom_read_pos: u8,   // Current read bit position (counts down from 67)
}

impl Cartridge {
    pub fn new(rom: &[u8]) -> Self {
        let backup_type = detect_backup_type(rom);
        let sram_size = match backup_type {
            BackupType::None => 0,
            BackupType::Sram => 0x8000,      // 32 KB
            BackupType::Flash64 => 0x10000,  // 64 KB
            BackupType::Flash128 => 0x20000, // 128 KB
            BackupType::Eeprom512 => 0x200,  // 512 bytes
            BackupType::Eeprom8K => 0x2000,  // 8 KB
        };

        Cartridge {
            backup_type,
            sram: vec![0xFF; sram_size],
            flash_state: FlashState::Ready,
            flash_bank: 0,
            dirty: false,
            eeprom_bits_in: 0,
            eeprom_bit_count: 0,
            eeprom_state: EepromState::Idle,
            eeprom_read_data: 0,
            eeprom_read_pos: 0,
        }
    }

    /// Read a byte from the backup memory region (0x0E000000).
    pub fn read(&self, addr: u32) -> u8 {
        let offset = (addr & 0xFFFF) as usize;
        match self.backup_type {
            BackupType::Sram => *self.sram.get(offset & 0x7FFF).unwrap_or(&0xFF),
            BackupType::Flash64 => {
                if self.flash_state == FlashState::IdMode {
                    return match offset {
                        0 => 0xBF, // Manufacturer ID (SST)
                        1 => 0xD4, // Device ID (64 KB)
                        _ => 0,
                    };
                }
                *self.sram.get(offset).unwrap_or(&0xFF)
            }
            BackupType::Flash128 => {
                if self.flash_state == FlashState::IdMode {
                    return match offset {
                        0 => 0x62, // Manufacturer ID (Sanyo)
                        1 => 0x13, // Device ID (128 KB)
                        _ => 0,
                    };
                }
                let real_offset = self.flash_bank * 0x10000 + offset;
                *self.sram.get(real_offset).unwrap_or(&0xFF)
            }
            BackupType::Eeprom512 | BackupType::Eeprom8K => {
                // EEPROM is accessed at 0x0D000000 via eeprom_read_bit() / eeprom_write_bit().
                // This path (0x0E) is not used for EEPROM serial access.
                1
            }
            _ => 0xFF,
        }
    }

    /// Read a bit from the EEPROM data shift register (called when clocking out data bits).
    /// Returns the next bit of eeprom_read_data (MSB first, 64 bits).
    ///
    /// Protocol: 4 dummy zero bits, then 64 data bits (MSB first).
    /// eeprom_read_pos starts at 68, counts down to 0.
    /// Positions 68..5 (exclusive) = dummy zeros (4 bits: 68,67,66,65)
    /// Positions 64..1 = data bits 63..0 (bit = data >> (pos-1) & 1)
    /// Position 0 = sequence complete, return to idle.
    pub fn eeprom_read_bit(&mut self) -> u8 {
        if self.eeprom_state != EepromState::ReadReady {
            return 1;
        }
        if self.eeprom_read_pos > 64 {
            // Dummy bits (positions 68, 67, 66, 65)
            self.eeprom_read_pos -= 1;
            0
        } else if self.eeprom_read_pos > 0 {
            // Data bits: pos 64 → bit63, pos 63 → bit62, ..., pos 1 → bit0
            let bit = ((self.eeprom_read_data >> (self.eeprom_read_pos - 1)) & 1) as u8;
            self.eeprom_read_pos -= 1;
            bit
        } else {
            // All bits clocked out — go back to idle
            self.eeprom_state = EepromState::Idle;
            self.eeprom_bit_count = 0;
            self.eeprom_bits_in = 0;
            1
        }
    }

    /// Write a bit to the EEPROM (called when the bus writes to address 0x0D000000).
    pub fn eeprom_write_bit(&mut self, bit: u8) {
        let bit = (bit & 1) as u64;
        let addr_bits: u8 = if self.backup_type == BackupType::Eeprom8K {
            14
        } else {
            6
        };

        match self.eeprom_state {
            EepromState::Idle => {
                // Wait for start bit (1)
                if bit == 1 {
                    self.eeprom_bits_in = 1;
                    self.eeprom_bit_count = 1;
                    self.eeprom_state = EepromState::ReadingCommand;
                }
            }
            EepromState::ReadingCommand => {
                self.eeprom_bits_in = (self.eeprom_bits_in << 1) | bit;
                self.eeprom_bit_count += 1;

                // After start(1) + opcode(2) + address(addr_bits) bits
                let cmd_bits = 1u8 + 2 + addr_bits;
                if self.eeprom_bit_count == cmd_bits {
                    let opcode = (self.eeprom_bits_in >> addr_bits) & 0x3;
                    let addr_mask = (1u64 << addr_bits) - 1;
                    let word_addr = (self.eeprom_bits_in & addr_mask) as usize;
                    let byte_addr = word_addr * 8; // Each word is 8 bytes (64 bits)

                    match opcode {
                        0b10 => {
                            // Read command — load data and prepare to clock out
                            let mut data = 0u64;
                            for i in 0..8 {
                                let b = *self.sram.get(byte_addr + i).unwrap_or(&0xFF) as u64;
                                data = (data << 8) | b;
                            }
                            self.eeprom_read_data = data;
                            // 4 dummy bits + 64 data bits = 68 total; read_pos starts at 68
                            self.eeprom_read_pos = 68;
                            self.eeprom_state = EepromState::ReadReady;
                            self.eeprom_bit_count = 0;
                            self.eeprom_bits_in = 0;
                        }
                        0b01 => {
                            // Write command — store address and prepare to receive 64 data bits
                            // Re-use bits_in to store the target byte address
                            self.eeprom_bits_in = byte_addr as u64;
                            self.eeprom_bit_count = 0;
                            self.eeprom_state = EepromState::WritingData;
                        }
                        _ => {
                            // Special (EWEN/EWDS) or unknown — ignore, go idle
                            self.eeprom_state = EepromState::Idle;
                            self.eeprom_bit_count = 0;
                            self.eeprom_bits_in = 0;
                        }
                    }
                }
            }
            EepromState::WritingData => {
                // Collect 64 data bits (MSB first)
                // We use eeprom_read_data as a temporary write buffer here
                self.eeprom_read_data = (self.eeprom_read_data << 1) | bit;
                self.eeprom_bit_count += 1;

                if self.eeprom_bit_count == 64 {
                    // Write the 64-bit word to SRAM
                    let byte_addr = self.eeprom_bits_in as usize;
                    let data = self.eeprom_read_data;
                    for i in 0..8 {
                        let b = ((data >> (56 - i * 8)) & 0xFF) as u8;
                        let idx = byte_addr + i;
                        if idx < self.sram.len() {
                            self.sram[idx] = b;
                        }
                    }
                    self.dirty = true;
                    self.eeprom_state = EepromState::Idle;
                    self.eeprom_bit_count = 0;
                    self.eeprom_bits_in = 0;
                    self.eeprom_read_data = 0;
                }
            }
            EepromState::ReadReady => {
                // Unexpected write while in read state; reset
                self.eeprom_state = EepromState::Idle;
                self.eeprom_bit_count = 0;
                self.eeprom_bits_in = 0;
            }
        }
    }

    /// Write a byte to the backup memory region (0x0E000000).
    pub fn write(&mut self, addr: u32, val: u8) {
        let offset = (addr & 0xFFFF) as usize;
        match self.backup_type {
            BackupType::Sram => {
                let idx = offset & 0x7FFF;
                if idx < self.sram.len() {
                    self.sram[idx] = val;
                    self.dirty = true;
                }
            }
            BackupType::Flash64 | BackupType::Flash128 => {
                self.write_flash(offset, val);
            }
            _ => {}
        }
    }

    fn write_flash(&mut self, offset: usize, val: u8) {
        match self.flash_state {
            FlashState::Ready => {
                if offset == 0x5555 && val == 0xAA {
                    self.flash_state = FlashState::Cmd1;
                }
            }
            FlashState::Cmd1 => {
                if offset == 0x2AAA && val == 0x55 {
                    self.flash_state = FlashState::Cmd2;
                } else {
                    self.flash_state = FlashState::Ready;
                }
            }
            FlashState::Cmd2 => {
                if offset == 0x5555 {
                    match val {
                        0x90 => self.flash_state = FlashState::IdMode,
                        0xF0 => self.flash_state = FlashState::Ready,
                        0x80 => self.flash_state = FlashState::Erase,
                        0xA0 => self.flash_state = FlashState::Write,
                        0xB0 => {
                            if self.backup_type == BackupType::Flash128 {
                                self.flash_state = FlashState::BankSel;
                            } else {
                                self.flash_state = FlashState::Ready;
                            }
                        }
                        _ => self.flash_state = FlashState::Ready,
                    }
                } else {
                    self.flash_state = FlashState::Ready;
                }
            }
            FlashState::IdMode => {
                if offset == 0x5555 && val == 0xAA {
                    self.flash_state = FlashState::Cmd1;
                } else if val == 0xF0 {
                    self.flash_state = FlashState::Ready;
                }
            }
            FlashState::Erase => {
                if offset == 0x5555 && val == 0xAA {
                    self.flash_state = FlashState::Cmd1;
                } else if val == 0x30 {
                    // Sector erase (4 KB)
                    let sector = (offset & 0xF000) + self.flash_bank * 0x10000;
                    let end = (sector + 0x1000).min(self.sram.len());
                    if sector < self.sram.len() {
                        self.sram[sector..end].fill(0xFF);
                        self.dirty = true;
                    }
                    self.flash_state = FlashState::Ready;
                } else if offset == 0x5555 && val == 0x10 {
                    // Full chip erase
                    self.sram.fill(0xFF);
                    self.dirty = true;
                    self.flash_state = FlashState::Ready;
                } else {
                    self.flash_state = FlashState::Ready;
                }
            }
            FlashState::Write => {
                let real_offset = self.flash_bank * 0x10000 + offset;
                if real_offset < self.sram.len() {
                    self.sram[real_offset] = val;
                    self.dirty = true;
                }
                self.flash_state = FlashState::Ready;
            }
            FlashState::BankSel => {
                if offset == 0 {
                    self.flash_bank = (val & 1) as usize;
                }
                self.flash_state = FlashState::Ready;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- detect_none ----

    #[test]
    fn detect_none() {
        let rom = vec![0u8; 0x200];
        let cart = Cartridge::new(&rom);
        assert_eq!(cart.backup_type, BackupType::None);
        assert!(cart.sram.is_empty());
    }

    // ---- detect_sram ----

    #[test]
    fn detect_sram() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x106].copy_from_slice(b"SRAM_V");
        let cart = Cartridge::new(&rom);
        assert_eq!(cart.backup_type, BackupType::Sram);
        assert_eq!(cart.sram.len(), 0x8000);
    }

    // ---- detect_flash64 ----

    #[test]
    fn detect_flash64() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let cart = Cartridge::new(&rom);
        assert_eq!(cart.backup_type, BackupType::Flash64);
        assert_eq!(cart.sram.len(), 0x10000);
    }

    // ---- detect_flash128 ----

    #[test]
    fn detect_flash128() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x109].copy_from_slice(b"FLASH1M_V");
        let cart = Cartridge::new(&rom);
        assert_eq!(cart.backup_type, BackupType::Flash128);
        assert_eq!(cart.sram.len(), 0x20000);
    }

    // ---- detect_eeprom_small ----

    #[test]
    fn detect_eeprom_small() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x108].copy_from_slice(b"EEPROM_V");
        let cart = Cartridge::new(&rom);
        // ROM is small (<= 16 MB), so Eeprom512
        assert_eq!(cart.backup_type, BackupType::Eeprom512);
        assert_eq!(cart.sram.len(), 0x200);
    }

    // ---- sram_read_write ----

    #[test]
    fn sram_read_write() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x106].copy_from_slice(b"SRAM_V");
        let mut cart = Cartridge::new(&rom);

        cart.write(0x0E00_0042, 0xAB);
        assert_eq!(cart.read(0x0E00_0042), 0xAB);
    }

    // ---- sram_dirty_flag ----

    #[test]
    fn sram_dirty_flag() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x106].copy_from_slice(b"SRAM_V");
        let mut cart = Cartridge::new(&rom);

        assert!(!cart.dirty);
        cart.write(0x0E00_0000, 0x55);
        assert!(cart.dirty);
    }

    // Helper: create a Flash64 cart in IdMode by issuing the unlock sequence + 0x90
    fn flash64_enter_id_mode() -> Cartridge {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let mut cart = Cartridge::new(&rom);
        // Unlock sequence
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0x90);
        cart
    }

    // ---- flash_id_mode ----

    #[test]
    fn flash_id_mode() {
        let cart = flash64_enter_id_mode();
        // Manufacturer ID at offset 0 → 0xBF (SST)
        assert_eq!(cart.read(0x0E00_0000), 0xBF);
        // Device ID at offset 1 → 0xD4 (64 KB)
        assert_eq!(cart.read(0x0E00_0001), 0xD4);
    }

    // ---- flash_write_byte ----

    #[test]
    fn flash_write_byte() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let mut cart = Cartridge::new(&rom);

        // Unlock + Write mode
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xA0);
        // Write one byte to offset 0x0010
        cart.write(0x0E00_0010, 0x42);
        assert_eq!(cart.read(0x0E00_0010), 0x42);
        assert!(cart.dirty);
    }

    // ---- flash_sector_erase ----

    #[test]
    fn flash_sector_erase() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let mut cart = Cartridge::new(&rom);

        // Write a byte first (using write mode)
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xA0);
        cart.write(0x0E00_0000, 0x55);
        assert_eq!(cart.read(0x0E00_0000), 0x55);

        // Sector erase: unlock, 0x80, then 0x30 at sector address (in Erase state)
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0x80);
        cart.write(0x0E00_0000, 0x30); // erase sector containing offset 0

        // After erase the sector should read 0xFF
        assert_eq!(cart.read(0x0E00_0000), 0xFF);
    }

    // ---- flash_chip_erase ----

    #[test]
    fn flash_chip_erase() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let mut cart = Cartridge::new(&rom);

        // Write a byte
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xA0);
        cart.write(0x0E00_1234, 0x77);

        // Chip erase: unlock, 0x80, then 0x10 at 0x5555 (in Erase state)
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0x80);
        cart.write(0x0E00_5555, 0x10);

        // All bytes should be 0xFF
        assert_eq!(cart.read(0x0E00_1234), 0xFF);
        assert!(cart.sram.iter().all(|&b| b == 0xFF));
    }

    // ---- flash128_bank_select ----

    #[test]
    fn flash128_bank_select() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x109].copy_from_slice(b"FLASH1M_V");
        let mut cart = Cartridge::new(&rom);

        // Write a byte to bank 0 offset 0
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xA0);
        cart.write(0x0E00_0000, 0x11);

        // Write a byte to bank 1 offset 0: first select bank 1
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xB0); // bank select command
        cart.write(0x0E00_0000, 0x01); // select bank 1
                                       // Now write to offset 0 (goes to bank 1 → real offset 0x10000)
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xA0);
        cart.write(0x0E00_0000, 0x22);

        // Reading offset 0 while on bank 1 should return bank 1 data
        assert_eq!(cart.read(0x0E00_0000), 0x22);

        // Switch back to bank 0 and read
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0xB0);
        cart.write(0x0E00_0000, 0x00);
        assert_eq!(cart.read(0x0E00_0000), 0x11);
    }

    // ---- flash_wrong_sequence_resets ----

    #[test]
    fn flash_wrong_sequence_resets() {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x107].copy_from_slice(b"FLASH_V");
        let mut cart = Cartridge::new(&rom);

        // Start unlock: write 0xAA at 0x5555 (Cmd1)
        cart.write(0x0E00_5555, 0xAA);
        // Wrong second step: wrong address → resets to Ready
        cart.write(0x0E00_1234, 0x55);

        // The state is back to Ready: a subsequent ID-mode entry from scratch
        // should still work, proving the previous state was cleaned up.
        cart.write(0x0E00_5555, 0xAA);
        cart.write(0x0E00_2AAA, 0x55);
        cart.write(0x0E00_5555, 0x90);
        // Now in IdMode — manufacturer reads correctly
        assert_eq!(cart.read(0x0E00_0000), 0xBF);
    }
}

/// Detect backup type by scanning ROM for identification strings.
fn detect_backup_type(rom: &[u8]) -> BackupType {
    fn contains(rom: &[u8], needle: &[u8]) -> bool {
        rom.windows(needle.len()).any(|w| w == needle)
    }

    if contains(rom, b"FLASH1M_V") {
        BackupType::Flash128
    } else if contains(rom, b"FLASH_V") || contains(rom, b"FLASH512_V") {
        BackupType::Flash64
    } else if contains(rom, b"SRAM_V") || contains(rom, b"SRAM_F_V") {
        BackupType::Sram
    } else if contains(rom, b"EEPROM_V") {
        if rom.len() > 0x100_0000 {
            BackupType::Eeprom8K
        } else {
            BackupType::Eeprom512
        }
    } else {
        BackupType::None
    }
}
