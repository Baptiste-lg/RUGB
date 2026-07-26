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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    /// Build a multi-bank ROM where each 16 KB bank is filled with its bank index.
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

    fn cart(num_banks: usize) -> Mbc2 {
        let rom = make_rom(num_banks);
        Mbc2::new(&rom, String::from("TEST"), false)
    }

    // ---------- RAM enable ----------

    #[test]
    fn ram_enable_bit8_zero() {
        // Bit 8 of address must be 0 to target the RAM-enable register.
        // Address 0x0000 has bit 8 = 0.
        let mut c = cart(2);
        c.write(0x0000, 0x0A); // enable RAM
                               // RAM is all-zero; read should return 0x00 | 0xF0 = 0xF0 (upper nibble always set).
        assert_eq!(c.read(0xA000), 0xF0);
    }

    #[test]
    fn ram_disabled_returns_0xff() {
        let c = cart(2);
        assert_eq!(c.read(0xA000), 0xFF);
    }

    // ---------- ROM bank select ----------

    #[test]
    fn rom_bank_select_bit8_one() {
        // Bit 8 of address = 1 selects the ROM bank.  Address 0x0100 has bit 8 set.
        let mut c = cart(4);
        c.write(0x0100, 0x02); // select bank 2
        assert_eq!(c.read(0x4000), 0x02); // bank 2 is filled with 0x02
    }

    #[test]
    fn bank_0_maps_to_1() {
        let mut c = cart(4);
        c.write(0x0100, 0x00); // attempt to select bank 0
                               // Bank 0 is forbidden; hardware remaps it to bank 1.
        assert_eq!(c.read(0x4000), 0x01);
    }

    // ---------- RAM upper nibble ----------

    #[test]
    fn ram_4bit_upper_nibble() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A); // enable RAM
        c.write(0xA000, 0x0F); // write lower nibble 0xF
                               // On read, upper nibble must be forced to 0xF.
        assert_eq!(c.read(0xA000), 0xFF);
    }

    #[test]
    fn ram_only_lower_nibble_stored() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A);
        c.write(0xA000, 0xAB); // write 0xAB — only lower nibble 0xB stored
                               // Read back: lower nibble is 0xB, upper is forced 0xF → 0xFB.
        assert_eq!(c.read(0xA000), 0xFB);
    }

    // ---------- RAM mirroring ----------

    #[test]
    fn ram_mirrored_512_bytes() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A); // enable RAM
                               // Write to offset 0 (0xA000) and read back at offset 0x200 (which wraps to 0 via & 0x1FF).
        c.write(0xA000, 0x07);
        // 0xA200 - 0xA000 = 0x200, 0x200 & 0x1FF = 0, so this should read the same cell.
        assert_eq!(c.read(0xA200), 0xF7); // 0x07 | 0xF0
    }
}
