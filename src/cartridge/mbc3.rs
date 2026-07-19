use super::Cartridge;
use crate::savestate::*;

/// MBC3 — used by Pokemon Gold/Silver/Crystal and other later titles.
///
/// 7-bit ROM bank select, 4 RAM banks, Real-Time Clock with latch mechanism.
pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    title: String,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    battery: bool,
    // RTC registers (latched values visible to the game)
    rtc_s: u8,
    rtc_m: u8,
    rtc_h: u8,
    rtc_dl: u8,
    rtc_dh: u8,
    // RTC internal running counters
    rtc_seconds: u32, // total seconds elapsed
    rtc_halt: bool,
    rtc_day_overflow: bool,
    // Latch state: write 0x00 then 0x01 to latch
    rtc_latch_ready: bool,
    // Sub-second accumulator (T-cycles)
    rtc_cycles: u32,
    // User-configurable offset in seconds (for RTC override)
    pub rtc_offset: i64,
}

impl Mbc3 {
    pub fn new(data: &[u8], ram_size: usize, title: String, battery: bool) -> Self {
        Mbc3 {
            rom: data.to_vec(),
            ram: vec![0; if ram_size > 0 { ram_size } else { 0x8000 }],
            title,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            battery,
            rtc_s: 0,
            rtc_m: 0,
            rtc_h: 0,
            rtc_dl: 0,
            rtc_dh: 0,
            rtc_seconds: 0,
            rtc_halt: false,
            rtc_day_overflow: false,
            rtc_latch_ready: false,
            rtc_cycles: 0,
            rtc_offset: 0,
        }
    }

    /// Advance RTC by the given number of T-cycles.
    fn advance_rtc(&mut self, cycles: u32) {
        if self.rtc_halt {
            return;
        }
        self.rtc_cycles += cycles;
        // 4,194,304 T-cycles = 1 second
        while self.rtc_cycles >= 4_194_304 {
            self.rtc_cycles -= 4_194_304;
            self.rtc_seconds += 1;
            // Day counter overflow at 512 days
            if self.rtc_seconds >= 512 * 86400 {
                self.rtc_seconds -= 512 * 86400;
                self.rtc_day_overflow = true;
            }
        }
    }

    fn latch_rtc(&mut self) {
        let total = (self.rtc_seconds as i64 + self.rtc_offset).max(0) as u32;
        let days = total / 86400;
        let remainder = total % 86400;
        self.rtc_h = (remainder / 3600) as u8;
        self.rtc_m = ((remainder % 3600) / 60) as u8;
        self.rtc_s = (remainder % 60) as u8;
        self.rtc_dl = (days & 0xFF) as u8;
        self.rtc_dh = ((days >> 8) & 1) as u8
            | if self.rtc_halt { 0x40 } else { 0 }
            | if self.rtc_day_overflow { 0x80 } else { 0 };
    }
}

impl Cartridge for Mbc3 {
    fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.ram_enabled);
        push_u8(d, self.rom_bank);
        push_u8(d, self.ram_bank);
        push_slice(d, &self.ram);
        // Save RTC state
        push_u32(d, self.rtc_seconds);
        push_u32(d, self.rtc_cycles);
        push_bool(d, self.rtc_halt);
        push_bool(d, self.rtc_day_overflow);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.ram_enabled = pop_bool(d);
        self.rom_bank = pop_u8(d);
        self.ram_bank = pop_u8(d);
        self.ram = pop_vec(d);
        // Load RTC state
        self.rtc_seconds = pop_u32(d);
        self.rtc_cycles = pop_u32(d);
        self.rtc_halt = pop_bool(d);
        self.rtc_day_overflow = pop_bool(d);
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
                match self.ram_bank {
                    0x00..=0x03 => {
                        let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
                        *self.ram.get(offset).unwrap_or(&0xFF)
                    }
                    0x08 => self.rtc_s,
                    0x09 => self.rtc_m,
                    0x0A => self.rtc_h,
                    0x0B => self.rtc_dl,
                    0x0C => self.rtc_dh,
                    _ => 0xFF,
                }
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
                // RTC latch: write 0x00 then 0x01
                if val == 0x00 {
                    self.rtc_latch_ready = true;
                } else if val == 0x01 && self.rtc_latch_ready {
                    self.latch_rtc();
                    self.rtc_latch_ready = false;
                }
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return;
                }
                match self.ram_bank {
                    0x00..=0x03 => {
                        let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
                        if offset < self.ram.len() {
                            self.ram[offset] = val;
                        }
                    }
                    0x08 => {
                        self.rtc_s = val & 0x3F;
                        // Sync internal counter
                        let days = self.rtc_seconds / 86400;
                        let old_hm = (self.rtc_seconds % 86400) - (self.rtc_seconds % 60);
                        self.rtc_seconds = days * 86400 + old_hm + val as u32;
                    }
                    0x09 => {
                        self.rtc_m = val & 0x3F;
                        let days = self.rtc_seconds / 86400;
                        let s = self.rtc_seconds % 60;
                        let h = (self.rtc_seconds % 86400) / 3600;
                        self.rtc_seconds = days * 86400 + h * 3600 + (val as u32) * 60 + s;
                    }
                    0x0A => {
                        self.rtc_h = val & 0x1F;
                        let days = self.rtc_seconds / 86400;
                        let m = (self.rtc_seconds % 3600) / 60;
                        let s = self.rtc_seconds % 60;
                        self.rtc_seconds = days * 86400 + (val as u32) * 3600 + m * 60 + s;
                    }
                    0x0B => {
                        self.rtc_dl = val;
                        let old_days = self.rtc_seconds / 86400;
                        let time_of_day = self.rtc_seconds % 86400;
                        let new_days = ((old_days & 0x100) | val as u32) as u32;
                        self.rtc_seconds = new_days * 86400 + time_of_day;
                    }
                    0x0C => {
                        self.rtc_dh = val;
                        self.rtc_halt = val & 0x40 != 0;
                        self.rtc_day_overflow = val & 0x80 != 0;
                        let time_of_day = self.rtc_seconds % 86400;
                        let day_bit8 = ((val & 1) as u32) << 8;
                        let low_days = (self.rtc_seconds / 86400) & 0xFF;
                        self.rtc_seconds = (day_bit8 | low_days) * 86400 + time_of_day;
                    }
                    _ => {}
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
        self.advance_rtc(cycles);
    }

    fn set_rtc_offset(&mut self, seconds: i32) {
        self.rtc_offset = seconds as i64;
    }
}
