use super::Cartridge;

/// ROM-only cartridge — no memory bank controller.
/// Max 32 KB ROM, no external RAM.
/// Used by Tetris, Dr. Mario, etc.
pub struct NoMbc {
    rom: Vec<u8>,
    title: String,
}

impl NoMbc {
    pub fn new(data: &[u8]) -> Self {
        let title = if data.len() >= 0x0144 {
            data[0x0134..0x0144]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect()
        } else {
            String::from("Unknown")
        };
        NoMbc {
            rom: data.to_vec(),
            title,
        }
    }

    pub fn empty() -> Self {
        NoMbc {
            rom: vec![0; 0x8000],
            title: String::new(),
        }
    }
}

impl Cartridge for NoMbc {
    fn save_state(&self, _data: &mut Vec<u8>) {
        // ROM-only: no mutable state to save
    }

    fn load_state(&mut self, _data: &mut &[u8]) {
        // ROM-only: nothing to restore
    }

    fn read(&self, addr: u16) -> u8 {
        let idx = addr as usize;
        if idx < self.rom.len() {
            self.rom[idx]
        } else {
            0xFF
        }
    }

    fn write(&mut self, _addr: u16, _val: u8) {
        // ROM-only: writes are ignored
    }

    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rom_with_title(title: &[u8]) -> Vec<u8> {
        // 32 KB ROM; title lives at 0x0134-0x0143.
        let mut rom = vec![0u8; 0x8000];
        let len = title.len().min(16);
        rom[0x0134..0x0134 + len].copy_from_slice(&title[..len]);
        rom
    }

    #[test]
    fn read_within_bounds() {
        let mut data = vec![0u8; 0x8000];
        data[0x0100] = 0x42;
        let cart = NoMbc::new(&data);
        assert_eq!(cart.read(0x0100), 0x42);
    }

    #[test]
    fn read_out_of_bounds_returns_0xff() {
        let data = vec![0u8; 0x4000]; // only 16 KB
        let cart = NoMbc::new(&data);
        // Address 0x7FFF is beyond the 16 KB ROM.
        assert_eq!(cart.read(0x7FFF), 0xFF);
    }

    #[test]
    fn write_is_ignored() {
        let mut data = vec![0u8; 0x8000];
        data[0x0200] = 0x55;
        let mut cart = NoMbc::new(&data);
        cart.write(0x0200, 0xAA);
        // Write should be a no-op; original byte must be unchanged.
        assert_eq!(cart.read(0x0200), 0x55);
    }

    #[test]
    fn title_parsed() {
        let title_bytes = b"TETRIS\0\0\0\0\0\0\0\0\0\0";
        let rom = make_rom_with_title(title_bytes);
        let cart = NoMbc::new(&rom);
        assert_eq!(cart.title(), "TETRIS");
    }

    #[test]
    fn empty_constructor() {
        let cart = NoMbc::empty();
        // empty() creates a 32 KB (0x8000 byte) ROM.
        // Last valid address is 0x7FFF; 0x8000 should return 0xFF.
        assert_eq!(cart.read(0x7FFF), 0x00); // zeroed memory
        assert_eq!(cart.read(0x8000), 0xFF); // out of bounds
    }
}
