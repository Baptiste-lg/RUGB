pub mod mbc1;
pub mod mbc3;
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
}

/// Parse ROM header byte 0x0147 and return the appropriate mapper.
pub fn from_rom(data: &[u8]) -> Box<dyn Cartridge> {
    if data.len() < 0x150 {
        return Box::new(no_mbc::NoMbc::new(data));
    }

    let cart_type = data[0x0147];
    let rom_title = parse_title(data);
    let ram_size = parse_ram_size(data[0x0149]);
    let has_battery = matches!(cart_type, 0x03 | 0x06 | 0x09 | 0x0D | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E | 0x22 | 0xFF);

    match cart_type {
        0x00 => Box::new(no_mbc::NoMbc::new(data)),
        0x01..=0x03 => Box::new(mbc1::Mbc1::new(data, ram_size, rom_title, has_battery)),
        0x0F..=0x13 => Box::new(mbc3::Mbc3::new(data, ram_size, rom_title, has_battery)),
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
