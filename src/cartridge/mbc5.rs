use super::Cartridge;
use crate::savestate::*;

/// MBC5 — the most common mapper for later Game Boy and Game Boy Color titles.
///
/// Supports up to 8 MB ROM (9-bit bank number) and 128 KB RAM (4-bit bank number).
/// Unlike MBC1, bank 0 IS valid for the switchable region.
///
/// Bank switching via writes to ROM address space:
///   0x0000-0x1FFF: RAM enable (0x0A enables)
///   0x2000-0x2FFF: ROM bank low 8 bits
///   0x3000-0x3FFF: ROM bank bit 8 (1 bit)
///   0x4000-0x5FFF: RAM bank (4 bits, 0x00-0x0F)
pub struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    rom_bank: u16, // 9-bit (0-511)
    ram_bank: u8,  // 4-bit (0-15)
    battery: bool,
}

impl Mbc5 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        Mbc5 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x2000 }],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            battery,
        }
    }
}

impl Cartridge for Mbc5 {
    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_u16(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.rom_bank = pop_u16(d);
        self.ram_bank = pop_u8(d);
        self.ram = pop_vec(d);
    }

    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let bank = self.rom_bank as usize;
                let offset = bank * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return 0xFF;
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
            0x2000..=0x2FFF => {
                // Low 8 bits of ROM bank
                self.rom_bank = (self.rom_bank & 0x100) | val as u16;
            }
            0x3000..=0x3FFF => {
                // Bit 8 of ROM bank
                self.rom_bank = (self.rom_bank & 0xFF) | ((val as u16 & 0x01) << 8);
            }
            0x4000..=0x5FFF => {
                self.ram_bank = val & 0x0F;
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
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

    fn has_battery(&self) -> bool {
        self.battery
    }

    fn ram_data(&self) -> &[u8] {
        &self.ram
    }

    fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }
}
