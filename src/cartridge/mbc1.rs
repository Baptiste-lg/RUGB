use super::Cartridge;

/// MBC1 — the most common Game Boy mapper.
///
/// Bank switching via writes to ROM address space:
///   0x0000-0x1FFF: RAM enable (0x0A enables, anything else disables)
///   0x2000-0x3FFF: ROM bank number (lower 5 bits, 0 maps to 1)
///   0x4000-0x5FFF: RAM bank OR upper ROM bank bits (2 bits)
///   0x6000-0x7FFF: Banking mode (0 = ROM, 1 = RAM)
pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    banking_mode: u8,
}

impl Mbc1 {
    pub fn new(data: &[u8], ram_size: usize, title: String) -> Self {
        Mbc1 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x2000 }],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            banking_mode: 0,
        }
    }

    fn effective_rom_bank(&self) -> usize {
        let bank = if self.banking_mode == 0 {
            (self.ram_bank as usize) << 5 | self.rom_bank as usize
        } else {
            self.rom_bank as usize
        };
        bank % (self.rom.len() / 0x4000).max(1)
    }

    fn effective_bank0(&self) -> usize {
        if self.banking_mode == 1 {
            ((self.ram_bank as usize) << 5) % (self.rom.len() / 0x4000).max(1)
        } else {
            0
        }
    }
}

impl Cartridge for Mbc1 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                let offset = self.effective_bank0() * 0x4000 + addr as usize;
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0x4000..=0x7FFF => {
                let offset = self.effective_rom_bank() * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return 0xFF;
                }
                let bank = if self.banking_mode == 1 {
                    self.ram_bank as usize
                } else {
                    0
                };
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                *self.ram.get(offset).unwrap_or(&0xFF)
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (val & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                let mut bank = val & 0x1F;
                if bank == 0 {
                    bank = 1;
                } // Bank 0 is never mapped to 0x4000-0x7FFF
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = val & 0x03;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = val & 0x01;
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return;
                }
                let bank = if self.banking_mode == 1 {
                    self.ram_bank as usize
                } else {
                    0
                };
                let offset = bank * 0x2000 + (addr as usize - 0xA000);
                if offset < self.ram.len() {
                    self.ram[offset] = val;
                }
            }
            _ => {}
        }
    }

    fn title(&self) -> &str {
        &self.title
    }
}
