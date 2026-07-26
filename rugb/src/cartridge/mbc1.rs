use super::Cartridge;
use crate::savestate::*;

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
    battery: bool,
}

impl Mbc1 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        Mbc1 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x2000 }],
            title,
            ram_enabled: false,
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

    fn effective_bank0(&self) -> usize {
        if self.banking_mode == 1 {
            ((self.ram_bank as usize) << 5) % (self.rom.len() / 0x4000).max(1)
        } else {
            0
        }
    }
}

impl Cartridge for Mbc1 {
    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_u8(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_u8(d, self.banking_mode);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.rom_bank = pop_u8(d);
        self.ram_bank = pop_u8(d);
        self.banking_mode = pop_u8(d);
        self.ram = pop_vec(d);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    /// Build an N-bank ROM (each bank = 0x4000 bytes).
    /// Bank `b` is filled with byte value `b as u8` so tests can identify which bank
    /// is mapped.
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

    fn cart(num_banks: usize) -> Mbc1 {
        let rom = make_rom(num_banks);
        Mbc1::new(&rom, 0x2000, String::from("TEST"), false)
    }

    fn cart_with_battery(num_banks: usize) -> Mbc1 {
        let rom = make_rom(num_banks);
        Mbc1::new(&rom, 0x2000, String::from("TEST"), true)
    }

    // ---------- RAM enable / disable ----------

    #[test]
    fn ram_disabled_by_default() {
        let c = cart(2);
        // RAM is disabled on construction; reading 0xA000 must return 0xFF.
        assert_eq!(c.read(0xA000), 0xFF);
    }

    #[test]
    fn ram_enable_0x0a() {
        let mut c = cart(2);
        c.write(0x1FFF, 0x0A); // write to 0x0000-0x1FFF range
                               // After enabling, RAM is zeroed so reading should return 0x00 (not 0xFF).
        assert_eq!(c.read(0xA000), 0x00);
    }

    #[test]
    fn ram_disable() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A); // enable
        c.write(0x0000, 0x00); // disable
        assert_eq!(c.read(0xA000), 0xFF);
    }

    // ---------- ROM bank switching ----------

    #[test]
    fn rom_bank_0_maps_to_1() {
        let mut c = cart(4);
        // Writing 0 to the bank register must map to bank 1.
        c.write(0x2000, 0x00);
        // Bank 1 bytes are filled with 0x01.
        assert_eq!(c.read(0x4000), 0x01);
    }

    #[test]
    fn rom_bank_select() {
        let mut c = cart(4);
        c.write(0x2000, 0x02);
        // Bank 2 bytes are filled with 0x02.
        assert_eq!(c.read(0x4000), 0x02);
    }

    // ---------- Banking mode ----------

    #[test]
    fn banking_mode_1_ram() {
        // In mode 1 the RAM bank register selects the RAM bank.
        // We need enough RAM: use a 4-bank ROM and 32 KB RAM.
        let rom = make_rom(4);
        let mut c = Mbc1::new(&rom, 0x8000, String::from("T"), false);
        c.write(0x0000, 0x0A); // enable RAM
        c.write(0x6000, 0x01); // switch to mode 1
        c.write(0x4000, 0x01); // select RAM bank 1
        c.write(0xA000, 0xBB); // write to RAM bank 1 offset 0
                               // Switch to RAM bank 0 and verify bank 1 write did not corrupt it.
        c.write(0x4000, 0x00);
        assert_eq!(c.read(0xA000), 0x00); // bank 0 is clean
                                          // Switch back to bank 1.
        c.write(0x4000, 0x01);
        assert_eq!(c.read(0xA000), 0xBB);
    }

    #[test]
    fn banking_mode_0_bank0_fixed() {
        // In mode 0 (default) the ROM bank 0 region is always physical bank 0.
        let mut c = cart(4);
        // Bank 0 is filled with 0x00.
        assert_eq!(c.read(0x0000), 0x00);
        // Changing the ram_bank register (upper bits) in mode 0 should not affect bank 0.
        c.write(0x4000, 0x01); // set upper bits to 1
        assert_eq!(c.read(0x0000), 0x00); // still bank 0
    }

    // ---------- Battery flag ----------

    #[test]
    fn has_battery_flag() {
        let no_bat = cart(2);
        assert!(!no_bat.has_battery());
        let with_bat = cart_with_battery(2);
        assert!(with_bat.has_battery());
    }

    // ---------- Save / load roundtrip ----------

    #[test]
    fn save_load_roundtrip() {
        let rom = make_rom(4);
        // Use 0x8000 RAM (4 banks) so ram_bank=1 is accessible
        let mut c = Mbc1::new(&rom, 0x8000, String::from("TEST"), false);
        c.write(0x0000, 0x0A); // enable RAM
        c.write(0x2000, 0x03); // rom_bank = 3
        c.write(0x4000, 0x01); // ram_bank = 1
        c.write(0x6000, 0x01); // banking_mode = 1
        c.write(0xA000, 0xCC); // write to RAM bank 1

        let mut state: Vec<u8> = Vec::new();
        c.save_state(&mut state);

        let mut c2 = Mbc1::new(&rom, 0x8000, String::from("TEST"), false);
        let mut slice: &[u8] = &state;
        c2.load_state(&mut slice);

        assert_eq!(c2.read(0x4000), c.read(0x4000));
        assert_eq!(c2.read(0xA000), 0xCC);
    }
}
