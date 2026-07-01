use crate::cartridge::{self, Cartridge};
use crate::ppu::Ppu;
use crate::timer::Timer;
use crate::joypad::Joypad;
use crate::apu::Apu;

pub struct Mmu {
    cartridge: Box<dyn Cartridge>,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub joypad: Joypad,
    /// Work RAM — 8 KB
    wram: [u8; 0x2000],
    /// High RAM — 127 bytes, CPU-accessible during OAM DMA
    hram: [u8; 0x7F],
    /// Interrupt Enable register at 0xFFFF
    pub ie: u8,
    /// Interrupt Flag register at 0xFF0F
    pub interrupt_flag: u8,
    /// Serial transfer data (0xFF01) — used by Blargg test ROMs to print output
    serial_data: u8,
}

impl Mmu {
    pub fn new() -> Self {
        Mmu {
            cartridge: Box::new(cartridge::no_mbc::NoMbc::empty()),
            ppu: Ppu::new(),
            apu: Apu::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            ie: 0,
            interrupt_flag: 0,
            serial_data: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        self.cartridge = cartridge::from_rom(data);
    }

    pub fn cartridge_title(&self) -> String {
        self.cartridge.title().to_string()
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM banks — routed through cartridge mapper
            0x0000..=0x7FFF => self.cartridge.read(addr),

            // VRAM
            0x8000..=0x9FFF => self.ppu.read_vram(addr),

            // External RAM — routed through cartridge mapper
            0xA000..=0xBFFF => self.cartridge.read(addr),

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],

            // Echo RAM — mirrors 0xC000-0xDDFF
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],

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
            // Remaining I/O returns 0xFF
            0xFF00..=0xFF7F => 0xFF,

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

            // VRAM
            0x8000..=0x9FFF => self.ppu.write_vram(addr, val),

            // External RAM
            0xA000..=0xBFFF => self.cartridge.write(addr, val),

            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,

            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,

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
            0xFF04..=0xFF07 => self.timer.write(addr, val),
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
            0xFF00..=0xFF7F => {} // Unhandled I/O — ignore

            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,

            // Interrupt Enable
            0xFFFF => self.ie = val,
        }
    }
}
