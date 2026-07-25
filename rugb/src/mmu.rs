use crate::apu::Apu;
use crate::cartridge::{self, Cartridge};
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::savestate::*;
use crate::timer::Timer;

/// Game Genie cheat: intercepts ROM reads at a specific address.
pub struct GgCheat {
    pub addr: u16,
    pub new_val: u8,
    pub compare: Option<u8>,
}

pub struct Mmu {
    cartridge: Box<dyn Cartridge>,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub joypad: Joypad,
    /// Work RAM — 8 banks × 4 KB for CGB (bank 0 always at 0xC000, banks 1-7 switchable at 0xD000)
    wram: [[u8; 0x1000]; 8],
    /// Current WRAM bank (SVBK register, 0xFF70). Bank 0 always at 0xC000.
    wram_bank: u8,
    /// High RAM — 127 bytes, CPU-accessible during OAM DMA
    hram: [u8; 0x7F],
    /// Interrupt Enable register at 0xFFFF
    pub ie: u8,
    /// Interrupt Flag register at 0xFF0F
    pub interrupt_flag: u8,
    /// Serial transfer data (0xFF01) — used by Blargg test ROMs to print output
    serial_data: u8,
    /// Optional boot ROM (256 bytes), mapped at 0x0000-0x00FF until 0xFF50 is written
    boot_rom: Option<Vec<u8>>,
    pub boot_rom_active: bool,
    /// Game Genie cheats — intercept ROM reads
    pub gg_cheats: Vec<GgCheat>,
    /// CGB mode flag
    pub cgb_mode: bool,
    /// KEY1 speed switch register (0xFF4D). Bit 7 = current speed, bit 0 = prepare switch.
    pub key1: u8,
    /// True when CPU is in double-speed mode
    pub double_speed: bool,
}

impl Mmu {
    pub fn new() -> Self {
        Mmu {
            cartridge: Box::new(cartridge::no_mbc::NoMbc::empty()),
            ppu: Ppu::new(),
            apu: Apu::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            wram: [[0; 0x1000]; 8],
            wram_bank: 1,
            hram: [0; 0x7F],
            ie: 0,
            interrupt_flag: 0,
            serial_data: 0,
            boot_rom: None,
            boot_rom_active: false,
            gg_cheats: Vec::new(),
            cgb_mode: false,
            key1: 0,
            double_speed: false,
        }
    }

    pub fn set_boot_rom(&mut self, data: Vec<u8>) {
        self.boot_rom_active = true;
        self.boot_rom = Some(data);
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        self.cartridge = cartridge::from_rom(data);
    }

    pub fn cartridge_title(&self) -> String {
        self.cartridge.title().to_string()
    }

    pub fn has_battery(&self) -> bool {
        self.cartridge.has_battery()
    }

    pub fn rumble(&self) -> bool {
        self.cartridge.rumble()
    }

    pub fn tick_cartridge_rtc(&mut self, cycles: u32) {
        self.cartridge.tick_rtc(cycles);
    }

    /// Write a byte to any address (used by GameShark cheats each frame).
    pub fn poke(&mut self, addr: u16, val: u8) {
        self.write(addr, val);
    }

    pub fn add_gg_cheat(&mut self, addr: u16, new_val: u8, compare: Option<u8>) {
        self.gg_cheats.push(GgCheat {
            addr,
            new_val,
            compare,
        });
    }

    pub fn clear_cheats(&mut self) {
        self.gg_cheats.clear();
    }

    pub fn set_rtc_offset(&mut self, seconds: i32) {
        self.cartridge.set_rtc_offset(seconds);
    }

    pub fn battery_ram(&self) -> &[u8] {
        self.cartridge.ram_data()
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        self.cartridge.load_ram(data);
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        for bank in &self.wram {
            d.extend_from_slice(bank);
        }
        d.extend_from_slice(&self.hram);
        push_u8(d, self.ie);
        push_u8(d, self.interrupt_flag);
        push_u8(d, self.serial_data);
        self.ppu.save_state(d);
        self.timer.save_state(d);
        self.apu.save_state(d);
        self.cartridge.save_state(d);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        for bank in &mut self.wram {
            bank.copy_from_slice(&d[..0x1000]);
            *d = &d[0x1000..];
        }
        self.hram.copy_from_slice(&d[..0x7F]);
        *d = &d[0x7F..];
        self.ie = pop_u8(d);
        self.interrupt_flag = pop_u8(d);
        self.serial_data = pop_u8(d);
        self.ppu.load_state(d);
        self.timer.load_state(d);
        self.apu.load_state(d);
        self.cartridge.load_state(d);
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // Boot ROM overlay at 0x0000-0x00FF
            0x0000..=0x00FF if self.boot_rom_active => {
                if let Some(ref boot) = self.boot_rom {
                    return boot[addr as usize];
                }
                self.cartridge.read(addr)
            }
            // ROM banks — routed through cartridge mapper (with Game Genie interception)
            0x0000..=0x7FFF => {
                let val = self.cartridge.read(addr);
                for cheat in &self.gg_cheats {
                    if cheat.addr == addr {
                        if let Some(cmp) = cheat.compare {
                            if val == cmp {
                                return cheat.new_val;
                            }
                        } else {
                            return cheat.new_val;
                        }
                    }
                }
                val
            }

            // VRAM (banked in CGB mode)
            0x8000..=0x9FFF => {
                if self.cgb_mode {
                    self.ppu.read_vram_banked(addr)
                } else {
                    self.ppu.read_vram(addr)
                }
            }

            // External RAM — routed through cartridge mapper
            0xA000..=0xBFFF => self.cartridge.read(addr),

            // Work RAM bank 0 (always 0xC000-0xCFFF)
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize],

            // Work RAM bank 1-7 (switchable 0xD000-0xDFFF, controlled by SVBK)
            0xD000..=0xDFFF => self.wram[self.wram_bank as usize][(addr - 0xD000) as usize],

            // Echo RAM — mirrors 0xC000-0xDDFF
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize],
            0xF000..=0xFDFF => self.wram[self.wram_bank as usize][(addr - 0xF000) as usize],

            // OAM — sprite attribute table
            0xFE00..=0xFE9F => self.ppu.read_oam(addr),

            // Unusable region
            0xFEA0..=0xFEFF => 0xFF,

            // I/O registers
            0xFF00 => self.joypad.read(),
            0xFF01 => self.serial_data,
            0xFF02 => 0x7E, // Serial control — stub, no link cable
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.interrupt_flag,
            0xFF10..=0xFF3F => self.apu.read(addr),
            0xFF40..=0xFF4B => self.ppu.read_register(addr),
            // CGB registers
            0xFF4D => self.key1 | if self.double_speed { 0x80 } else { 0 },
            0xFF4F => self.ppu.vram_bank | 0xFE,
            0xFF68..=0xFF6B => self.ppu.read_register(addr),
            0xFF70 => self.wram_bank,
            // Remaining I/O returns 0xFF
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF7F => 0xFF,

            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],

            // Interrupt Enable
            0xFFFF => self.ie,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ROM area — writes go to the cartridge mapper (bank switching)
            0x0000..=0x7FFF => self.cartridge.write(addr, val),

            // VRAM (banked in CGB mode)
            0x8000..=0x9FFF => {
                if self.cgb_mode {
                    self.ppu.write_vram_banked(addr, val);
                } else {
                    self.ppu.write_vram(addr, val);
                }
            }

            // External RAM
            0xA000..=0xBFFF => self.cartridge.write(addr, val),

            // Work RAM bank 0
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize] = val,

            // Work RAM bank 1-7
            0xD000..=0xDFFF => {
                self.wram[self.wram_bank as usize][(addr - 0xD000) as usize] = val;
            }

            // Echo RAM
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize] = val,
            0xF000..=0xFDFF => {
                self.wram[self.wram_bank as usize][(addr - 0xF000) as usize] = val;
            }

            // OAM
            0xFE00..=0xFE9F => self.ppu.write_oam(addr, val),

            // Unusable
            0xFEA0..=0xFEFF => {}

            // I/O registers
            0xFF00 => self.joypad.write(val),
            0xFF01 => self.serial_data = val,
            0xFF02 => {
                // Blargg test ROMs: writing 0x81 triggers serial "send"
                if val == 0x81 {
                    #[cfg(not(target_arch = "wasm32"))]
                    eprint!("{}", self.serial_data as char);
                }
            }
            0xFF04..=0xFF07 => {
                let mut iflag = self.interrupt_flag;
                self.timer.write(addr, val, &mut iflag);
                self.interrupt_flag = iflag;
            }
            0xFF0F => self.interrupt_flag = val,
            0xFF10..=0xFF3F => self.apu.write(addr, val),
            0xFF40..=0xFF4B => {
                // OAM DMA is triggered by writing to 0xFF46
                if addr == 0xFF46 {
                    let source = (val as u16) << 8;
                    for i in 0..0xA0u16 {
                        let byte = self.read(source + i);
                        self.ppu.write_oam(0xFE00 + i, byte);
                    }
                }
                self.ppu.write_register(addr, val);
            }
            0xFF50 => {
                if val != 0 {
                    self.boot_rom_active = false;
                }
            }
            // CGB registers
            0xFF4D => self.key1 = (self.key1 & 0x80) | (val & 0x01),
            0xFF4F => self.ppu.vram_bank = val & 1,
            0xFF68..=0xFF6B => self.ppu.write_register(addr, val),
            0xFF70 => {
                self.wram_bank = val & 0x07;
                if self.wram_bank == 0 {
                    self.wram_bank = 1;
                }
            }
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF7F => {} // Unhandled I/O

            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,

            // Interrupt Enable
            0xFFFF => self.ie = val,
        }
    }
}
