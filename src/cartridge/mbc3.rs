use super::Cartridge;
use crate::savestate::*;

/// MBC3 — used by Pokemon Gold/Silver/Crystal and other later titles.
///
/// 7-bit ROM bank select, 4 RAM banks, optional RTC (stubbed here).
pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
}

impl Mbc3 {
    pub fn new(data: &[u8], ram_size: usize, title: String) -> Self {
        Mbc3 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x8000 }],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
        }
    }
}

impl Cartridge for Mbc3 {
    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_u8(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.rom_bank = pop_u8(d);
        self.ram_bank = pop_u8(d);
        self.ram = pop_vec(d);
    }

    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let bank = self.rom_bank.max(1) as usize;
                let offset = bank * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                // Banks 0x08-0x0C would be RTC registers — return 0 for now
                if self.ram_bank >= 0x08 {
                    return 0;
                }
                let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
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
                self.rom_bank = val & 0x7F;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5FFF => {
                self.ram_bank = val;
            }
            0x6000..=0x7FFF => {
                // RTC latch — stubbed
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram_bank >= 0x08 {
                    return;
                }
                let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
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
