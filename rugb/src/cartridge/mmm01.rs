use crate::cartridge::Cartridge;
use crate::savestate::*;

/// MMM01 mapper (cart types 0x0B-0x0D) — multi-cart mapper.
/// On power-up, the last 32KB of ROM is mapped (menu). Writing to 0x0000-0x1FFF
/// activates "inner" MBC1-like mode with a configurable ROM base offset.
pub struct Mmm01 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    battery: bool,
    total_banks: usize,
    // Outer (menu) state
    mapping_active: bool,
    outer_rom_bank: u8,
    #[allow(dead_code)]
    outer_ram_bank: u8,
    #[allow(dead_code)]
    outer_mode: u8,
    // Inner (MBC1-like) state
    inner_rom_bank: u8,
    inner_ram_bank: u8,
    inner_banking_mode: u8,
    ram_enabled: bool,
}

impl Mmm01 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        let total_banks = (data.len() / 0x4000).max(2);
        Mmm01 {
            rom: data.to_vec(),
            ram: vec![0; ram_size],
            title,
            battery,
            total_banks,
            mapping_active: false,
            outer_rom_bank: 0,
            outer_ram_bank: 0,
            outer_mode: 0,
            inner_rom_bank: 1,
            inner_ram_bank: 0,
            inner_banking_mode: 0,
            ram_enabled: false,
        }
    }

    fn outer_bank0_offset(&self) -> usize {
        // Menu ROM: second-to-last bank
        (self.total_banks - 2) * 0x4000
    }

    fn outer_bank1_offset(&self) -> usize {
        // Menu ROM: last bank
        (self.total_banks - 1) * 0x4000
    }

    fn inner_base(&self) -> usize {
        (self.outer_rom_bank as usize) << 5
    }

    fn inner_effective_bank(&self) -> usize {
        let base = self.inner_base();
        let bank = if self.inner_banking_mode == 0 {
            base | self.inner_rom_bank as usize
        } else {
            self.inner_rom_bank as usize
        };
        bank % self.total_banks
    }

    fn inner_bank0(&self) -> usize {
        if self.inner_banking_mode == 1 {
            self.inner_base() % self.total_banks
        } else {
            0
        }
    }
}

impl Cartridge for Mmm01 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                if !self.mapping_active {
                    let offset = self.outer_bank0_offset() + addr as usize;
                    *self.rom.get(offset).unwrap_or(&0xFF)
                } else {
                    let offset = self.inner_bank0() * 0x4000 + addr as usize;
                    *self.rom.get(offset).unwrap_or(&0xFF)
                }
            }
            0x4000..=0x7FFF => {
                if !self.mapping_active {
                    let offset = self.outer_bank1_offset() + (addr as usize - 0x4000);
                    *self.rom.get(offset).unwrap_or(&0xFF)
                } else {
                    let offset = self.inner_effective_bank() * 0x4000 + (addr as usize - 0x4000);
                    *self.rom.get(offset).unwrap_or(&0xFF)
                }
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled || self.ram.is_empty() {
                    return 0xFF;
                }
                let bank = if self.inner_banking_mode == 1 {
                    self.inner_ram_bank as usize
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
        if !self.mapping_active {
            // Outer mode: configure mapping, then activate
            match addr {
                0x0000..=0x1FFF => {
                    // Any write with bit 0 set activates inner mode
                    if val & 0x01 != 0 {
                        self.mapping_active = true;
                    }
                }
                0x2000..=0x3FFF => {
                    self.outer_rom_bank = val & 0x3F;
                }
                0x4000..=0x5FFF => {
                    self.outer_ram_bank = val & 0x03;
                }
                0x6000..=0x7FFF => {
                    self.outer_mode = val & 0x01;
                }
                _ => {}
            }
        } else {
            // Inner mode: MBC1-like
            match addr {
                0x0000..=0x1FFF => {
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                }
                0x2000..=0x3FFF => {
                    let mut bank = val & 0x1F;
                    if bank == 0 {
                        bank = 1;
                    }
                    self.inner_rom_bank = bank;
                }
                0x4000..=0x5FFF => {
                    self.inner_ram_bank = val & 0x03;
                }
                0x6000..=0x7FFF => {
                    self.inner_banking_mode = val & 0x01;
                }
                0xA000..=0xBFFF => {
                    if !self.ram_enabled || self.ram.is_empty() {
                        return;
                    }
                    let bank = if self.inner_banking_mode == 1 {
                        self.inner_ram_bank as usize
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
        push_bool(d, self.mapping_active);
        push_u8(d, self.outer_rom_bank);
        push_u8(d, self.outer_ram_bank);
        push_u8(d, self.outer_mode);
        push_u8(d, self.inner_rom_bank);
        push_u8(d, self.inner_ram_bank);
        push_u8(d, self.inner_banking_mode);
        push_bool(d, self.ram_enabled);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.mapping_active = pop_bool(d);
        self.outer_rom_bank = pop_u8(d);
        self.outer_ram_bank = pop_u8(d);
        self.outer_mode = pop_u8(d);
        self.inner_rom_bank = pop_u8(d);
        self.inner_ram_bank = pop_u8(d);
        self.inner_banking_mode = pop_u8(d);
        self.ram_enabled = pop_bool(d);
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

    #[test]
    fn outer_mode_reads_last_banks() {
        let rom = make_rom(8);
        let c = Mmm01::new(&rom, 0x2000, String::from("TEST"), false);
        // Bank 0 should read from second-to-last bank (6)
        assert_eq!(c.read(0x0000), 6);
        // Bank 1 should read from last bank (7)
        assert_eq!(c.read(0x4000), 7);
    }

    #[test]
    fn mapping_activate() {
        let rom = make_rom(8);
        let mut c = Mmm01::new(&rom, 0x2000, String::from("TEST"), false);
        assert!(!c.mapping_active);
        c.write(0x0000, 0x01);
        assert!(c.mapping_active);
    }

    #[test]
    fn inner_mode_bank_switching() {
        let rom = make_rom(8);
        let mut c = Mmm01::new(&rom, 0x2000, String::from("TEST"), false);
        c.write(0x0000, 0x01); // activate
        c.write(0x2000, 0x03); // select inner bank 3
        assert_eq!(c.read(0x4000), 3);
    }

    #[test]
    fn save_load_roundtrip() {
        let rom = make_rom(8);
        let mut c = Mmm01::new(&rom, 0x2000, String::from("TEST"), false);
        c.write(0x0000, 0x01);
        c.write(0x2000, 0x05);

        let mut buf = Vec::new();
        c.save_state(&mut buf);

        let mut c2 = Mmm01::new(&rom, 0x2000, String::from("TEST"), false);
        let mut slice: &[u8] = &buf;
        c2.load_state(&mut slice);

        assert!(c2.mapping_active);
        assert_eq!(c2.inner_rom_bank, 5);
    }
}
