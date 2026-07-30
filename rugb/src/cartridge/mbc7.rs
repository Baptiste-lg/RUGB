use crate::cartridge::Cartridge;
use crate::savestate::*;

/// MBC7 mapper (cart type 0x22) — used by Kirby Tilt 'n' Tumble.
/// Features: MBC5-style ROM banking, accelerometer, 93C56 EEPROM (256 bytes).
pub struct Mbc7 {
    rom: Vec<u8>,
    title: String,
    battery: bool,
    rom_bank: u16,
    // Two-step RAM/sensor enable
    ram_enable_1: bool,
    ram_enable_2: bool,
    // Accelerometer
    accel_x: u16, // latched 16-bit value (center = 0x81D0)
    accel_y: u16,
    accel_latched: bool,
    // Raw tilt input from host
    tilt_x: i16,
    tilt_y: i16,
    // EEPROM
    eeprom: Eeprom93c56,
}

/// 93C56-compatible EEPROM (128 × 16-bit = 256 bytes), SPI bit-bang interface.
struct Eeprom93c56 {
    data: [u8; 256],
    cs: bool,
    sk: bool,
    di: bool,
    do_out: bool,
    write_enabled: bool,
    // Command shift register
    bits_in: u32,
    bit_count: u8,
    // Read state
    reading: bool,
    read_data: u16,
    read_bit: u8,
}

impl Eeprom93c56 {
    fn new() -> Self {
        Eeprom93c56 {
            data: [0xFF; 256],
            cs: false,
            sk: false,
            di: false,
            do_out: true,
            write_enabled: false,
            bits_in: 0,
            bit_count: 0,
            reading: false,
            read_data: 0,
            read_bit: 0,
        }
    }

    fn read_byte(&self) -> u8 {
        let mut val = 0u8;
        if self.do_out {
            val |= 0x01;
        }
        if self.di {
            val |= 0x02;
        }
        if self.sk {
            val |= 0x40;
        }
        if self.cs {
            val |= 0x80;
        }
        val
    }

    fn write_byte(&mut self, val: u8) {
        let new_cs = val & 0x80 != 0;
        let new_sk = val & 0x40 != 0;
        self.di = val & 0x02 != 0;

        if !new_cs {
            // CS deasserted — reset
            self.cs = false;
            self.sk = false;
            self.bit_count = 0;
            self.bits_in = 0;
            self.reading = false;
            self.do_out = true;
            return;
        }

        let rising_edge = new_sk && !self.sk;
        self.cs = new_cs;
        self.sk = new_sk;

        if !rising_edge {
            return;
        }

        if self.reading {
            // Output one bit of the read data
            self.do_out = (self.read_data >> (15 - self.read_bit)) & 1 != 0;
            self.read_bit += 1;
            if self.read_bit >= 16 {
                self.reading = false;
                self.do_out = true;
            }
            return;
        }

        // Shift in a bit
        self.bits_in = (self.bits_in << 1) | self.di as u32;
        self.bit_count += 1;

        // Need start bit (1) + 2-bit opcode + 7-bit address = 10 bits
        if self.bit_count < 10 {
            return;
        }

        let start = (self.bits_in >> 9) & 1;
        if start != 1 {
            self.bit_count = 0;
            self.bits_in = 0;
            return;
        }

        let opcode = (self.bits_in >> 7) & 0x03;
        let addr = (self.bits_in & 0x7F) as u8;

        match opcode {
            0b10 => {
                // READ: output 16 bits from addr
                let byte_addr = (addr as usize) * 2;
                let hi = *self.data.get(byte_addr).unwrap_or(&0xFF);
                let lo = *self.data.get(byte_addr + 1).unwrap_or(&0xFF);
                self.read_data = (hi as u16) << 8 | lo as u16;
                self.read_bit = 0;
                self.reading = true;
                self.do_out = false; // dummy 0 bit before data
            }
            0b01 => {
                // WRITE: need 16 more data bits
                if self.bit_count < 26 {
                    return; // wait for all 16 data bits
                }
                if self.write_enabled {
                    let word = self.bits_in & 0xFFFF; // last 16 bits
                                                             // Actually the data comes after the 10-bit command
                                                             // bits_in has 26 bits: [start(1)][op(2)][addr(7)][data(16)]
                    let data_word = self.bits_in & 0xFFFF;
                    let byte_addr = (addr as usize) * 2;
                    if byte_addr + 1 < self.data.len() {
                        self.data[byte_addr] = (data_word >> 8) as u8;
                        self.data[byte_addr + 1] = (data_word & 0xFF) as u8;
                    }
                    let _ = word;
                }
                self.bit_count = 0;
                self.bits_in = 0;
            }
            0b11 => {
                // ERASE: fill address with 0xFFFF
                if self.write_enabled {
                    let byte_addr = (addr as usize) * 2;
                    if byte_addr + 1 < self.data.len() {
                        self.data[byte_addr] = 0xFF;
                        self.data[byte_addr + 1] = 0xFF;
                    }
                }
                self.bit_count = 0;
                self.bits_in = 0;
            }
            0b00 => {
                // Special: EWEN/EWDS/ERAL/WRAL based on addr bits 6-5
                let sub = (addr >> 5) & 0x03;
                match sub {
                    0b11 => self.write_enabled = true,  // EWEN
                    0b00 => self.write_enabled = false, // EWDS
                    0b10 => {
                        // ERAL: erase all
                        if self.write_enabled {
                            self.data.fill(0xFF);
                        }
                    }
                    0b01 => {
                        // WRAL: write all — need 16 more data bits
                        if self.bit_count < 26 {
                            return;
                        }
                        if self.write_enabled {
                            let data_word = self.bits_in & 0xFFFF;
                            let hi = (data_word >> 8) as u8;
                            let lo = (data_word & 0xFF) as u8;
                            for i in (0..256).step_by(2) {
                                self.data[i] = hi;
                                self.data[i + 1] = lo;
                            }
                        }
                        // fall through to reset
                    }
                    _ => {}
                }
                self.bit_count = 0;
                self.bits_in = 0;
            }
            _ => unreachable!(),
        }
    }
}

const ACCEL_CENTER: u16 = 0x81D0;

impl Mbc7 {
    pub fn new(data: &[u8], title: String, battery: bool) -> Self {
        Mbc7 {
            rom: data.to_vec(),
            title,
            battery,
            rom_bank: 1,
            ram_enable_1: false,
            ram_enable_2: false,
            accel_x: ACCEL_CENTER,
            accel_y: ACCEL_CENTER,
            accel_latched: false,
            tilt_x: 0,
            tilt_y: 0,
            eeprom: Eeprom93c56::new(),
        }
    }

    fn effective_rom_bank(&self) -> usize {
        let bank = self.rom_bank as usize;
        bank % (self.rom.len() / 0x4000).max(1)
    }

    fn is_enabled(&self) -> bool {
        self.ram_enable_1 && self.ram_enable_2
    }

    fn latch_accel(&mut self) {
        self.accel_x = (ACCEL_CENTER as i32 + self.tilt_x as i32).clamp(0, 0xFFFF) as u16;
        self.accel_y = (ACCEL_CENTER as i32 + self.tilt_y as i32).clamp(0, 0xFFFF) as u16;
        self.accel_latched = true;
    }
}

impl Cartridge for Mbc7 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let offset = self.effective_rom_bank() * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.is_enabled() {
                    return 0xFF;
                }
                match addr & 0xF0 {
                    0x00 => 0x00, // latch control (write-only, reads 0)
                    0x20 => (self.accel_x & 0xFF) as u8,
                    0x30 => (self.accel_x >> 8) as u8,
                    0x40 => (self.accel_y & 0xFF) as u8,
                    0x50 => (self.accel_y >> 8) as u8,
                    0x60 => 0x00, // Z-axis stub
                    0x80 => self.eeprom.read_byte(),
                    _ => 0xFF,
                }
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enable_1 = val == 0x0A;
                if !self.ram_enable_1 {
                    self.ram_enable_2 = false;
                }
            }
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | val as u16;
            }
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0xFF) | ((val as u16 & 0x01) << 8);
            }
            0x4000..=0x5FFF => {
                self.ram_enable_2 = val == 0x40;
            }
            0xA000..=0xBFFF => {
                if !self.is_enabled() {
                    return;
                }
                match addr & 0xF0 {
                    0x00 => {
                        if val == 0x55 {
                            self.latch_accel();
                        }
                    }
                    0x80 => {
                        self.eeprom.write_byte(val);
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
        &self.eeprom.data
    }

    fn load_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.eeprom.data.len());
        self.eeprom.data[..len].copy_from_slice(&data[..len]);
    }

    fn set_tilt(&mut self, x: i16, y: i16) {
        self.tilt_x = x;
        self.tilt_y = y;
    }

    fn save_state(&self, d: &mut Vec<u8>) {
        push_u16(d, self.rom_bank);
        push_bool(d, self.ram_enable_1);
        push_bool(d, self.ram_enable_2);
        push_i16(d, self.tilt_x);
        push_i16(d, self.tilt_y);
        push_u16(d, self.accel_x);
        push_u16(d, self.accel_y);
        push_bool(d, self.accel_latched);
        push_bool(d, self.eeprom.write_enabled);
        push_slice(d, &self.eeprom.data);
    }

    fn load_state(&mut self, d: &mut &[u8]) {
        self.rom_bank = pop_u16(d);
        self.ram_enable_1 = pop_bool(d);
        self.ram_enable_2 = pop_bool(d);
        self.tilt_x = pop_i16(d);
        self.tilt_y = pop_i16(d);
        self.accel_x = pop_u16(d);
        self.accel_y = pop_u16(d);
        self.accel_latched = pop_bool(d);
        self.eeprom.write_enabled = pop_bool(d);
        let eeprom_data = pop_vec(d);
        let len = eeprom_data.len().min(self.eeprom.data.len());
        self.eeprom.data[..len].copy_from_slice(&eeprom_data[..len]);
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

    fn cart(num_banks: usize) -> Mbc7 {
        let rom = make_rom(num_banks);
        Mbc7::new(&rom, String::from("TEST"), true)
    }

    fn enable_sensors(c: &mut Mbc7) {
        c.write(0x0000, 0x0A);
        c.write(0x4000, 0x40);
    }

    #[test]
    fn rom_bank_9bit() {
        let mut c = cart(4);
        c.write(0x2000, 0x03);
        assert_eq!(c.rom_bank, 3);
        assert_eq!(c.read(0x4000), 3);
    }

    #[test]
    fn not_readable_without_both_enables() {
        let mut c = cart(2);
        c.write(0x0000, 0x0A); // step 1 only
        assert_eq!(c.read(0xA020), 0xFF);
    }

    #[test]
    fn accelerometer_latch_and_read() {
        let mut c = cart(2);
        enable_sensors(&mut c);
        c.set_tilt(100, -50);
        c.write(0xA000, 0x55); // latch
        let x = c.read(0xA020) as u16 | (c.read(0xA030) as u16) << 8;
        let y = c.read(0xA040) as u16 | (c.read(0xA050) as u16) << 8;
        assert_eq!(x, ACCEL_CENTER.wrapping_add(100));
        assert_eq!(y, ACCEL_CENTER.wrapping_sub(50));
    }

    #[test]
    fn eeprom_write_enable() {
        let mut c = cart(2);
        enable_sensors(&mut c);
        // EWEN command: start(1) + opcode(00) + addr(11xxxxx) = 10011xxxxx
        // 10 bits: 1_00_1100000 = 0b1001100000 = 0x260 shifted into SPI
        // We need to clock in 10 bits via CS/SK/DI
        let eeprom_addr = 0xA080;
        // Assert CS
        c.write(eeprom_addr, 0x80); // CS=1, SK=0
                                    // Clock in: 1 0 0 1 1 0 0 0 0 0 (start=1, op=00, addr=1100000)
        let bits: [bool; 10] = [
            true, false, false, true, true, false, false, false, false, false,
        ];
        for &bit in &bits {
            let di = if bit { 0x02 } else { 0x00 };
            c.write(eeprom_addr, 0x80 | di); // SK=0
            c.write(eeprom_addr, 0xC0 | di); // SK=1 (rising edge)
        }
        assert!(c.eeprom.write_enabled);
    }

    #[test]
    fn eeprom_read() {
        let mut c = cart(2);
        enable_sensors(&mut c);
        // Pre-fill EEPROM word 0 with 0xABCD
        c.eeprom.data[0] = 0xAB;
        c.eeprom.data[1] = 0xCD;

        let eeprom_addr = 0xA080;
        // READ command: start(1) + opcode(10) + addr(0000000) = 1_10_0000000
        c.write(eeprom_addr, 0x80); // CS=1
        let bits: [bool; 10] = [
            true, true, false, false, false, false, false, false, false, false,
        ];
        for &bit in &bits {
            let di = if bit { 0x02 } else { 0x00 };
            c.write(eeprom_addr, 0x80 | di);
            c.write(eeprom_addr, 0xC0 | di);
        }
        // Now clock out 16 data bits
        let mut result: u16 = 0;
        for i in 0..16 {
            c.write(eeprom_addr, 0x80); // SK=0
            c.write(eeprom_addr, 0xC0); // SK=1 → read DO
            let do_bit = c.read(eeprom_addr) & 0x01;
            result |= (do_bit as u16) << (15 - i);
        }
        assert_eq!(result, 0xABCD);
    }

    #[test]
    fn save_load_roundtrip() {
        let mut c = cart(4);
        c.write(0x2000, 0x05);
        c.set_tilt(200, -100);
        c.eeprom.data[0] = 0x42;

        let mut buf = Vec::new();
        c.save_state(&mut buf);

        let mut c2 = cart(4);
        let mut slice: &[u8] = &buf;
        c2.load_state(&mut slice);

        assert_eq!(c2.rom_bank, 5);
        assert_eq!(c2.tilt_x, 200);
        assert_eq!(c2.tilt_y, -100);
        assert_eq!(c2.eeprom.data[0], 0x42);
    }
}
