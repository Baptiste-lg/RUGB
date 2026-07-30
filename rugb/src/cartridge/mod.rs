pub mod huc1;
pub mod huc3;
pub mod mbc1;
pub mod mbc2;
pub mod mbc3;
pub mod mbc5;
pub mod mbc7;
pub mod mmm01;
pub mod no_mbc;

pub trait Cartridge {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn title(&self) -> &str;
    fn save_state(&self, data: &mut Vec<u8>);
    fn load_state(&mut self, data: &mut &[u8]);
    /// Whether this cartridge has battery-backed SRAM.
    fn has_battery(&self) -> bool {
        false
    }
    /// Return the current cartridge RAM contents (for battery save persistence).
    fn ram_data(&self) -> &[u8] {
        &[]
    }
    /// Restore cartridge RAM from previously saved data.
    fn load_ram(&mut self, _data: &[u8]) {}
    /// Whether the rumble motor is currently active (MBC5 rumble carts only).
    fn rumble(&self) -> bool {
        false
    }
    /// Advance internal RTC by the given T-cycles (MBC3 only).
    fn tick_rtc(&mut self, _cycles: u32) {}
    /// Set RTC time offset in seconds (MBC3 only, for user override).
    fn set_rtc_offset(&mut self, _seconds: i32) {}
    /// Feed accelerometer data from host (MBC7 only). Center = 0x81D0.
    fn set_tilt(&mut self, _x: i16, _y: i16) {}
}

/// Detect if ROM is a Game Boy Color title (header byte 0x0143).
pub fn is_cgb(data: &[u8]) -> bool {
    if data.len() <= 0x0143 {
        return false;
    }
    data[0x0143] == 0x80 || data[0x0143] == 0xC0
}

/// Parse ROM header byte 0x0147 and return the appropriate mapper.
pub fn from_rom(data: &[u8]) -> Box<dyn Cartridge> {
    if data.len() < 0x150 {
        return Box::new(no_mbc::NoMbc::new(data));
    }

    let cart_type = data[0x0147];
    let rom_title = parse_title(data);
    let ram_size = parse_ram_size(data[0x0149]);
    let has_battery = matches!(
        cart_type,
        0x03 | 0x06 | 0x09 | 0x0D | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E | 0x22 | 0xFE | 0xFF
    );

    let has_rumble = matches!(cart_type, 0x1C..=0x1E);

    match cart_type {
        0x00 => Box::new(no_mbc::NoMbc::new(data)),
        0x01..=0x03 => Box::new(mbc1::Mbc1::new(data, ram_size, rom_title, has_battery)),
        0x05..=0x06 => Box::new(mbc2::Mbc2::new(data, rom_title, has_battery)),
        0x0B..=0x0D => Box::new(mmm01::Mmm01::new(data, ram_size, rom_title, has_battery)),
        0x0F..=0x13 => Box::new(mbc3::Mbc3::new(data, ram_size, rom_title, has_battery)),
        0x19..=0x1E => Box::new(mbc5::Mbc5::new(
            data,
            ram_size,
            rom_title,
            has_battery,
            has_rumble,
        )),
        0x22 => Box::new(mbc7::Mbc7::new(data, rom_title, has_battery)),
        0xFE => Box::new(huc3::Huc3::new(data, ram_size, rom_title, has_battery)),
        0xFF => Box::new(huc1::Huc1::new(data, ram_size, rom_title, has_battery)),
        _ => {
            // Fall back to NoMBC for unsupported mappers
            #[cfg(debug_assertions)]
            eprintln!(
                "Unsupported cartridge type 0x{:02X}, falling back to NoMBC",
                cart_type
            );
            Box::new(no_mbc::NoMbc::new(data))
        }
    }
}

fn parse_title(data: &[u8]) -> String {
    let title_bytes = &data[0x0134..0x0144];
    title_bytes
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect()
}

fn parse_ram_size(code: u8) -> usize {
    match code {
        0x00 => 0,
        0x01 => 2 * 1024,
        0x02 => 8 * 1024,
        0x03 => 32 * 1024,
        0x04 => 128 * 1024,
        0x05 => 64 * 1024,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal ROM of the correct size for the given header fields.
    /// Byte 0x0147 = cart_type, 0x0148 = rom_size_code, 0x0149 = ram_size_code.
    /// The ROM size code 0x00 = 2 banks = 32 KB, 0x01 = 4 banks = 64 KB, etc.
    fn make_rom(cart_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        // Minimum size so all header indices are valid (0x150 bytes minimum).
        // ROM size: 32 KB << rom_size_code, clamped to at least 0x150 bytes.
        let num_banks = 2usize << rom_size_code as usize;
        let rom_len = (num_banks * 0x4000).max(0x150);
        let mut rom = vec![0u8; rom_len];
        rom[0x0147] = cart_type;
        rom[0x0148] = rom_size_code;
        rom[0x0149] = ram_size_code;
        rom
    }

    // ---------- is_cgb ----------

    #[test]
    fn is_cgb_false_for_dmg() {
        let mut rom = vec![0u8; 0x0144];
        rom[0x0143] = 0x00; // plain DMG byte
        assert!(!is_cgb(&rom));
    }

    #[test]
    fn is_cgb_true_for_0x80() {
        let mut rom = vec![0u8; 0x0144];
        rom[0x0143] = 0x80;
        assert!(is_cgb(&rom));
    }

    #[test]
    fn is_cgb_true_for_0xc0() {
        let mut rom = vec![0u8; 0x0144];
        rom[0x0143] = 0xC0;
        assert!(is_cgb(&rom));
    }

    #[test]
    fn is_cgb_short_rom() {
        // ROM that ends before index 0x0143 — must return false, not panic.
        let rom = vec![0u8; 0x0100];
        assert!(!is_cgb(&rom));
    }

    // ---------- from_rom ----------

    #[test]
    fn from_rom_short_returns_no_mbc() {
        // A ROM shorter than 0x150 should silently create a NoMbc.
        let short = vec![0u8; 0x100];
        let cart = from_rom(&short);
        // NoMbc::read returns the byte stored in the ROM (here 0x00) for in-range addresses
        // and 0xFF for out-of-range. Verifying it doesn't panic is sufficient, but we also
        // confirm in-range reads work.
        assert_eq!(cart.read(0x0000), 0x00);
    }

    #[test]
    fn from_rom_type_0x00_no_mbc() {
        let rom = make_rom(0x00, 0x00, 0x00);
        let cart = from_rom(&rom);
        // Should not panic and should read as ROM-only.
        assert_eq!(cart.read(0x0000), 0x00);
    }

    #[test]
    fn from_rom_type_0x01_mbc1() {
        // Two-bank ROM: bank 0 at 0x0000-0x3FFF, bank 1 at 0x4000-0x7FFF.
        // We fill bank 1 with a sentinel byte to confirm bank switching works.
        let mut rom = make_rom(0x01, 0x00, 0x00); // rom_size_code 0x00 = 32 KB / 2 banks
                                                  // Bank 1 starts at offset 0x4000; write a sentinel at the very first byte.
        rom[0x4000] = 0xAB;
        let mut cart = from_rom(&rom);
        // Default bank is 1, so reading 0x4000 should return the sentinel.
        assert_eq!(cart.read(0x4000), 0xAB);
        // Write bank register to select bank 2 (wraps to bank 1 on a 2-bank ROM).
        cart.write(0x2000, 0x02);
        // For a 2-bank ROM bank 2 % 2 = 0, but effective_rom_bank clamps to max(1) so
        // the read should still return a valid byte (not panic).
        let _ = cart.read(0x4000);
    }

    #[test]
    fn from_rom_type_0x05_mbc2() {
        let rom = make_rom(0x05, 0x00, 0x00);
        let cart = from_rom(&rom);
        assert_eq!(cart.read(0x0000), 0x00);
    }

    #[test]
    fn from_rom_type_0x19_mbc5() {
        let rom = make_rom(0x19, 0x00, 0x00);
        let cart = from_rom(&rom);
        assert_eq!(cart.read(0x0000), 0x00);
    }

    #[test]
    fn from_rom_unsupported_falls_back() {
        // Type 0x14 is not handled — must fall back gracefully without panicking.
        let rom = make_rom(0x14, 0x00, 0x00);
        let cart = from_rom(&rom);
        // Falls back to NoMbc; first byte of ROM is 0x00.
        assert_eq!(cart.read(0x0000), 0x00);
    }
}
