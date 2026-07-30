use crate::cartridge::Cartridge;
use crate::savestate::*;

/// HuC1 mapper (cart type 0xFF) — similar to MBC1 with infrared LED stub.
/// Used by some Hudson Soft games.
pub struct Huc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    ir_mode: bool,
    rom_bank: u8,
    ram_bank: u8,
    banking_mode: u8,
    battery: bool,
}

impl Huc1 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        Huc1 {
            rom: data.to_vec(),
            ram: vec![0; ram_size],
            title,
            ram_enabled: false,
            ir_mode: false,
            rom_bank: 1,
            ram_bank: 0,
            banking_mode: 0,
            battery,
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
}

impl Cartridge for Huc1 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let offset = self.effective_rom_bank() * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                if self.ir_mode {
                    return 0xC0; // IR receive stub: no signal
                }
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
                if val == 0x0E {
                    self.ir_mode = true;
                    self.ram_enabled = false;
                } else if val & 0x0F == 0x0A {
                    self.ir_mode = false;
                    self.ram_enabled = true;
                } else {
                    self.ir_mode = false;
                    self.ram_enabled = false;
                }
            }
            0x2000..=0x3FFF => {
                let mut bank = val & 0x3F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = val & 0x03;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = val & 0x01;
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() || self.ir_mode {
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

    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_bool(d, self.ir_mode);
        push_u8(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_u8(d, self.banking_mode);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.ir_mode = pop_bool(d);
        self.rom_bank = pop_u8(d);
        self.ram_bank = pop_u8(d);
        self.banking_mode = pop_u8(d);
        self.ram = pop_vec(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    fn make_rom(num_banks: usize) -> Vec<u8> {
        let mut rom = vec![0u8; num_banks * 0x4000];
        for bank in 0..num_banks {
            let start = bank * 0x4000;
            for byte in &mut rom[start..start + 0x4000] {
                *byte = bank as u8;
            }
        }
        rom
    }

    fn cart(num_banks: usize) -> Huc1 {
        let rom = make_rom(num_banks);
        Huc1::new(&rom, 0x2000, String::from("TEST"), true)
    }

    #[test]
    fn rom_bank_0_maps_to_1() {
        let mut c = cart(4);
        c.write(0x2000, 0x00);
        assert_eq!(c.rom_bank, 1);
    }

    #[test]
    fn rom_bank_6bit_mask() {
        let mut c = cart(4);
        c.write(0x2000, 0x42); // 0x42 & 0x3F = 0x02
        assert_eq!(c.rom_bank, 2);
    }

    #[test]
    fn ram_enabled_by_0x0a() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A);
        assert!(c.ram_enabled);
        assert!(!c.ir_mode);
        assert_eq!(c.read(0xA000), 0x00);
    }

    #[test]
    fn ir_mode_returns_0xc0() {
        let mut c = cart(2);
        c.write(0x0000, 0x0E);
        assert!(c.ir_mode);
        assert!(!c.ram_enabled);
        assert_eq!(c.read(0xA000), 0xC0);
    }

    #[test]
    fn save_load_roundtrip() {
        let mut c = cart(4);
        c.write(0x0000, 0x0A);
        c.write(0x2000, 0x03);
        c.write(0xA000, 0xBB);

        let mut buf = Vec::new();
        c.save_state(&mut buf);

        let mut c2 = cart(4);
        let mut slice: &[u8] = &buf;
        c2.load_state(&mut slice);

        assert_eq!(c2.rom_bank, 3);
        assert!(c2.ram_enabled);
        assert_eq!(c2.read(0xA000), 0xBB);
    }
}
