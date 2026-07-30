use crate::cartridge::Cartridge;
use crate::savestate::*;

/// HuC3 mapper (cart type 0xFE) — MBC3-like with RTC, IR, and tone generator.
/// Used by Robopon and some Hudson Soft games.
pub struct Huc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    mode: u8, // 0x00/0x0A = RAM, 0x0B = RTC, 0x0C = IR, 0x0D = tone
    battery: bool,
    // RTC
    rtc_minutes: u32,
    rtc_cycles: u32,
    // Command interface
    cmd_reg: u8,
    read_nibble: u8,
}

const CYCLES_PER_MINUTE: u32 = 4_194_304 * 60;

impl Huc3 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        Huc3 {
            rom: data.to_vec(),
            ram: vec![0; ram_size],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            mode: 0,
            battery,
            rtc_minutes: 0,
            rtc_cycles: 0,
            cmd_reg: 0,
            read_nibble: 0,
        }
    }

    fn effective_rom_bank(&self) -> usize {
        let bank = self.rom_bank as usize;
        bank % (self.rom.len() / 0x4000).max(1)
    }
}

impl Cartridge for Huc3 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let offset = self.effective_rom_bank() * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                match self.mode {
                    0x0B => {
                        // RTC command read — return status nibble
                        0x01 | ((self.read_nibble & 0x0F) << 1)
                    }
                    0x0C => 0x00, // IR receive stub
                    0x0D => 0x00, // Tone stub
                    _ => {
                        if !self.ram_enabled || self.ram.is_empty() {
                            return 0xFF;
                        }
                        let offset =
                            self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
                        *self.ram.get(offset).unwrap_or(&0xFF)
                    }
                }
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.mode = val;
                self.ram_enabled = val == 0x0A;
            }
            0x2000..=0x3FFF => {
                let mut bank = val & 0x7F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = val & 0x0F;
            }
            0xA000..=0xBFFF => {
                if self.mode == 0x0B {
                    // RTC command write — nibble protocol
                    let cmd = (val >> 4) & 0x0F;
                    let data = val & 0x0F;
                    match cmd {
                        0x01 => {
                            // Read minutes low nibble
                            self.read_nibble = (self.rtc_minutes & 0x0F) as u8;
                        }
                        0x02 => {
                            // Read minutes mid nibble
                            self.read_nibble = ((self.rtc_minutes >> 4) & 0x0F) as u8;
                        }
                        0x03 => {
                            // Read minutes high nibble
                            self.read_nibble = ((self.rtc_minutes >> 8) & 0x0F) as u8;
                        }
                        0x04 => {
                            // Read days low nibble
                            let days = self.rtc_minutes / (24 * 60);
                            self.read_nibble = (days & 0x0F) as u8;
                        }
                        0x05 => {
                            let days = self.rtc_minutes / (24 * 60);
                            self.read_nibble = ((days >> 4) & 0x0F) as u8;
                        }
                        0x06 => {
                            let days = self.rtc_minutes / (24 * 60);
                            self.read_nibble = ((days >> 8) & 0x0F) as u8;
                        }
                        _ => {
                            self.cmd_reg = data;
                        }
                    }
                } else if self.mode == 0x0A || self.mode == 0x00 {
                    if !self.ram_enabled || self.ram.is_empty() {
                        return;
                    }
                    let offset =
                        self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
                    if offset < self.ram.len() {
                        self.ram[offset] = val;
                    }
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

    fn tick_rtc(&mut self, cycles: u32) {
        self.rtc_cycles += cycles;
        while self.rtc_cycles >= CYCLES_PER_MINUTE {
            self.rtc_cycles -= CYCLES_PER_MINUTE;
            self.rtc_minutes += 1;
        }
    }

    fn save_state(&self, d: &mut Vec<u8>) {
        push_u8(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_bool(d, self.ram_enabled);
        push_u8(d, self.mode);
        push_u32(d, self.rtc_minutes);
        push_u32(d, self.rtc_cycles);
        push_slice(d, &self.ram);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.rom_bank = pop_u8(d);
        self.ram_bank = pop_u8(d);
        self.ram_enabled = pop_bool(d);
        self.mode = pop_u8(d);
        self.rtc_minutes = pop_u32(d);
        self.rtc_cycles = pop_u32(d);
        self.ram = pop_vec(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Cartridge;

    fn cart(num_banks: usize) -> Huc3 {
        let rom = vec![0u8; num_banks * 0x4000];
        Huc3::new(&rom, 0x2000, String::from("TEST"), true)
    }

    #[test]
    fn rom_bank_0_maps_to_1() {
        let mut c = cart(4);
        c.write(0x2000, 0x00);
        assert_eq!(c.rom_bank, 1);
    }

    #[test]
    fn ram_access_when_enabled() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A);
        c.write(0xA000, 0x42);
        assert_eq!(c.read(0xA000), 0x42);
    }

    #[test]
    fn ram_disabled_returns_0xff() {
        let c = cart(2);
        assert_eq!(c.read(0xA000), 0xFF);
    }

    #[test]
    fn rtc_ticks_advance_minutes() {
        let mut c = cart(2);
        assert_eq!(c.rtc_minutes, 0);
        c.tick_rtc(CYCLES_PER_MINUTE);
        assert_eq!(c.rtc_minutes, 1);
        c.tick_rtc(CYCLES_PER_MINUTE * 5);
        assert_eq!(c.rtc_minutes, 6);
    }

    #[test]
    fn save_load_roundtrip() {
        let mut c = cart(4);
        c.write(0x0000, 0x0A);
        c.write(0x2000, 0x03);
        c.write(0xA000, 0xBB);
        c.tick_rtc(CYCLES_PER_MINUTE * 10);

        let mut buf = Vec::new();
        c.save_state(&mut buf);

        let mut c2 = cart(4);
        let mut slice: &[u8] = &buf;
        c2.load_state(&mut slice);

        assert_eq!(c2.rom_bank, 3);
        assert_eq!(c2.rtc_minutes, 10);
        assert_eq!(c2.read(0xA000), 0xBB);
    }
}
