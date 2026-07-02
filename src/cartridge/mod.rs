pub mod mbc1;
pub mod mbc3;
pub mod no_mbc;

pub trait Cartridge {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn title(&self) -> &str;
}

/// Parse ROM header byte 0x0147 and return the appropriate mapper.
pub fn from_rom(data: &[u8]) -> Box<dyn Cartridge> {
    if data.len() < 0x150 {
        return Box::new(no_mbc::NoMbc::new(data));
    }

    let cart_type = data[0x0147];
    let rom_title = parse_title(data);
    let ram_size = parse_ram_size(data[0x0149]);

    match cart_type {
        0x00 => Box::new(no_mbc::NoMbc::new(data)),
        0x01..=0x03 => Box::new(mbc1::Mbc1::new(data, ram_size, rom_title)),
        0x0F..=0x13 => Box::new(mbc3::Mbc3::new(data, ram_size, rom_title)),
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
