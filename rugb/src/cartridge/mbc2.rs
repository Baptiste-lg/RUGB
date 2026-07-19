use super::Cartridge;
use crate::savestate::*;

/// MBC2 — simple mapper with built-in 512×4-bit RAM.
///
/// ROM banking via writes to 0x0000-0x3FFF:
///   - Bit 8 of address = 0: RAM enable (low nibble 0x0A enables)
///   - Bit 8 of address = 1: ROM bank select (4 bits, 0 maps to 1)
///
/// RAM is 512 bytes at 0xA000-0xA1FF, only lower 4 bits of each byte are used.
pub struct Mbc2 {
    rom: Vec<u8>,
    ram: [u8; 512],
    title: String,
    ram_enabled: bool,
    rom_bank: u8,
    battery: bool,
}

impl Mbc2 {
    pub fn new(data: &[u8], title: String, battery: bool) -> Self {
        Mbc2 {
            rom: data.to_vec(),
            ram: [0; 512],
            title,
            ram_enabled: false,
            rom_bank: 1,
            battery,
        }
    }
}

impl Cartridge for Mbc2 {
    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_u8(d, self.rom_bank);
        d.extend_from_slice(&self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.rom_bank = pop_u8(d);
        self.ram.copy_from_slice(&d[..512]);
        *d = &d[512..];
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
                // Only 512 bytes, mirrored; only lower 4 bits valid
                let offset = (addr as usize - 0xA000) & 0x1FF;
                self.ram[offset] | 0xF0
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x3FFF => {
                if addr & 0x0100 == 0 {
                    // RAM enable: bit 8 of address is 0
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                } else {
                    // ROM bank select: bit 8 of address is 1
                    self.rom_bank = val & 0x0F;
                    if self.rom_bank == 0 {
                        self.rom_bank = 1;
                    }
                }
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return;
                }
                let offset = (addr as usize - 0xA000) & 0x1FF;
                self.ram[offset] = val & 0x0F; // only lower 4 bits
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
