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
    has_rumble: bool,
    pub rumble_active: bool,
}

impl Mbc5 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool, rumble: bool) -> Self {
        Mbc5 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x2000 }],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            battery,
            has_rumble: rumble,
            rumble_active: false,
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
                if self.has_rumble {
                    self.rumble_active = val & 0x08 != 0;
                    self.ram_bank = val & 0x07;
                } else {
                    self.ram_bank = val & 0x0F;
                }
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

    fn rumble(&self) -> bool {
        self.rumble_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    /// Build a multi-bank ROM where each 16 KB bank is filled with its bank index byte.
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

    fn cart(num_banks: usize) -> Mbc5 {
        let rom = make_rom(num_banks);
        Mbc5::new(&rom, 0x2000, String::from("TEST"), false, false)
    }

    fn cart_rumble(num_banks: usize) -> Mbc5 {
        let rom = make_rom(num_banks);
        Mbc5::new(&rom, 0x8000, String::from("TEST"), false, true)
    }

    // ---------- Bank 0 is valid for MBC5 ----------

    #[test]
    fn bank_0_is_valid() {
        let mut c = cart(4);
        // Write bank 0 explicitly (MBC5 allows it, unlike MBC1).
        c.write(0x2000, 0x00);
        // Bank 0 is filled with 0x00.
        assert_eq!(c.read(0x4000), 0x00);
    }

    // ---------- ROM bank low byte ----------

    #[test]
    fn rom_bank_low_byte() {
        let mut c = cart(4);
        c.write(0x2000, 0x02); // low 8 bits = 2
        assert_eq!(c.read(0x4000), 0x02);
    }

    // ---------- ROM bank high bit ----------

    #[test]
    fn rom_bank_high_bit() {
        // Need a ROM large enough that bank 0x101 (257) exists.
        // 258 banks × 0x4000 = 1 032 192 bytes.
        let num_banks = 258usize;
        let mut rom = vec![0u8; num_banks * 0x4000];
        // Fill bank 257 (0x101) with sentinel 0xEE.
        let start = 257 * 0x4000;
        for byte in &mut rom[start..start + 0x4000] {
            *byte = 0xEE;
        }
        let mut c = Mbc5::new(&rom, 0x2000, String::from("TEST"), false, false);
        c.write(0x2000, 0x01); // low byte = 1
        c.write(0x3000, 0x01); // set bit 8
                               // rom_bank = 0x101 = 257
        assert_eq!(c.read(0x4000), 0xEE);
    }

    // ---------- RAM enable ----------

    #[test]
    fn ram_enable() {
        let mut c = cart(2);
        // RAM disabled by default.
        assert_eq!(c.read(0xA000), 0xFF);
        c.write(0x0000, 0x0A);
        // RAM is zeroed; read returns 0x00.
        assert_eq!(c.read(0xA000), 0x00);
    }

    // ---------- Rumble ----------

    #[test]
    fn rumble_bit3() {
        let mut c = cart_rumble(4);
        assert!(!c.rumble());
        // Bit 3 of the value written to 0x4000-0x5FFF controls rumble on rumble carts.
        c.write(0x4000, 0x08); // set bit 3
        assert!(c.rumble());
        c.write(0x4000, 0x00); // clear bit 3
        assert!(!c.rumble());
    }

    #[test]
    fn rumble_ram_bank_3bits() {
        // On a rumble cart, only bits 0-2 are used for the RAM bank.
        let mut c = cart_rumble(4);
        c.write(0x0000, 0x0A); // enable RAM
                               // Writing 0x0F: bit 3 = rumble, bits 0-2 = bank 7 — but bank 7 is out of range for
                               // a 32 KB (4-bank) RAM, so the read falls back to 0xFF via get().unwrap_or.
                               // More usefully: write bank 1 (0x01, no rumble bit).
        c.write(0x4000, 0x01); // bit 3 = 0 (no rumble), bits 0-2 = 1
        c.write(0xA000, 0xBB); // write to RAM bank 1
        c.write(0x4000, 0x00);
        assert_eq!(c.read(0xA000), 0x00); // bank 0 clean
        c.write(0x4000, 0x01);
        assert_eq!(c.read(0xA000), 0xBB);
    }

    #[test]
    fn non_rumble_ram_bank_4bits() {
        // On a non-rumble cart 4 bits are used for RAM bank (0x0F mask).
        let rom = make_rom(4);
        // 4 RAM banks × 0x2000 = 32 KB.
        let mut c = Mbc5::new(&rom, 0x8000, String::from("TEST"), false, false);
        c.write(0x0000, 0x0A);
        c.write(0x4000, 0x01); // RAM bank 1
        c.write(0xA000, 0xCC);
        c.write(0x4000, 0x00);
        assert_eq!(c.read(0xA000), 0x00);
        c.write(0x4000, 0x01);
        assert_eq!(c.read(0xA000), 0xCC);
    }

    // ---------- Save / load roundtrip ----------

    #[test]
    fn save_load_roundtrip() {
        let mut c = cart(4);
        c.write(0x0000, 0x0A);
        c.write(0x2000, 0x03); // low byte of rom_bank
        c.write(0xA000, 0xAB);

        let mut state: Vec<u8> = Vec::new();
        c.save_state(&mut state);

        let mut c2 = cart(4);
        let mut slice: &[u8] = &state;
        c2.load_state(&mut slice);

        assert_eq!(c2.read(0x4000), c.read(0x4000));
        assert_eq!(c2.read(0xA000), 0xAB);
    }
}
