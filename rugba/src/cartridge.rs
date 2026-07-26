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
            _ => 0xFF,
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
    let rom_str = rom
        .iter()
        .map(|&b| if b.is_ascii() { b as char } else { ' ' })
        .collect::<String>();

    if rom_str.contains("FLASH1M_V") {
        BackupType::Flash128
    } else if rom_str.contains("FLASH_V") || rom_str.contains("FLASH512_V") {
        BackupType::Flash64
    } else if rom_str.contains("SRAM_V") || rom_str.contains("SRAM_F_V") {
        BackupType::Sram
    } else if rom_str.contains("EEPROM_V") {
        // Guess size from ROM size (>16MB usually means 8KB EEPROM)
        if rom.len() > 0x100_0000 {
            BackupType::Eeprom8K
        } else {
            BackupType::Eeprom512
        }
    } else {
        BackupType::None
    }
}
