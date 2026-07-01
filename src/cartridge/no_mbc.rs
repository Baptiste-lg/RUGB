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
