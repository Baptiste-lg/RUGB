use crate::cartridge::Cartridge;
use crate::io::IoRegisters;
use crate::keypad::Keypad;

/// GBA memory bus — routes all CPU reads/writes to the correct subsystem.
pub struct Bus {
    /// 256 KB external work RAM (2 wait state)
    pub ewram: Box<[u8; 0x40000]>,
    /// 32 KB internal work RAM (0 wait state)
    pub iwram: Box<[u8; 0x8000]>,
    /// I/O registers
    pub io: IoRegisters,
    /// 1 KB palette RAM (BG + OBJ, 256 colors each)
    pub palette: Box<[u8; 0x400]>,
    /// 96 KB video RAM
    pub vram: Box<[u8; 0x18000]>,
    /// 1 KB OAM (128 sprites × 8 bytes)
    pub oam: Box<[u8; 0x400]>,
    /// Game Pak ROM (up to 32 MB)
    pub rom: Vec<u8>,
    /// Cartridge backup (SRAM/Flash/EEPROM)
    pub cart: Cartridge,
    /// Keypad input
    pub keypad: Keypad,
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Self {
        let cart = Cartridge::new(&rom);
        Bus {
            ewram: Box::new([0; 0x40000]),
            iwram: Box::new([0; 0x8000]),
            io: IoRegisters::new(),
            palette: Box::new([0; 0x400]),
            vram: Box::new([0; 0x18000]),
            oam: Box::new([0; 0x400]),
            rom,
            cart,
            keypad: Keypad::new(),
        }
    }

    pub fn read8(&self, addr: u32) -> u8 {
        match addr >> 24 {
            0x00 => 0, // BIOS (HLE — return 0 for reads)
            0x02 => self.ewram[(addr & 0x3FFFF) as usize],
            0x03 => self.iwram[(addr & 0x7FFF) as usize],
            0x04 => {
                let val = self.read_io16(addr & !1);
                if addr & 1 == 0 {
                    val as u8
                } else {
                    (val >> 8) as u8
                }
            }
            0x05 => self.palette[(addr & 0x3FF) as usize],
            0x06 => {
                let a = self.mirror_vram(addr);
                self.vram[a]
            }
            0x07 => self.oam[(addr & 0x3FF) as usize],
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                *self.rom.get(offset).unwrap_or(&0)
            }
            0x0E..=0x0F => self.cart.read(addr),
            _ => 0, // Open bus
        }
    }

    pub fn read16(&self, addr: u32) -> u16 {
        let addr = addr & !1; // Force alignment
        match addr >> 24 {
            0x00 => 0,
            0x02 => {
                let a = (addr & 0x3FFFF) as usize;
                u16::from_le_bytes([self.ewram[a], self.ewram[a + 1]])
            }
            0x03 => {
                let a = (addr & 0x7FFF) as usize;
                u16::from_le_bytes([self.iwram[a], self.iwram[a + 1]])
            }
            0x04 => self.read_io16(addr),
            0x05 => {
                let a = (addr & 0x3FF) as usize;
                u16::from_le_bytes([self.palette[a], self.palette[a + 1]])
            }
            0x06 => {
                let a = self.mirror_vram(addr);
                u16::from_le_bytes([self.vram[a], self.vram[a + 1]])
            }
            0x07 => {
                let a = (addr & 0x3FF) as usize;
                u16::from_le_bytes([self.oam[a], self.oam[a + 1]])
            }
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                let lo = *self.rom.get(offset).unwrap_or(&0);
                let hi = *self.rom.get(offset + 1).unwrap_or(&0);
                u16::from_le_bytes([lo, hi])
            }
            0x0E..=0x0F => {
                // SRAM/Flash is 8-bit bus — return byte duplicated
                let v = self.cart.read(addr);
                (v as u16) | ((v as u16) << 8)
            }
            _ => 0,
        }
    }

    pub fn read32(&self, addr: u32) -> u32 {
        let addr = addr & !3; // Force alignment
        match addr >> 24 {
            0x02 => {
                let a = (addr & 0x3FFFF) as usize;
                u32::from_le_bytes([
                    self.ewram[a],
                    self.ewram[a + 1],
                    self.ewram[a + 2],
                    self.ewram[a + 3],
                ])
            }
            0x03 => {
                let a = (addr & 0x7FFF) as usize;
                u32::from_le_bytes([
                    self.iwram[a],
                    self.iwram[a + 1],
                    self.iwram[a + 2],
                    self.iwram[a + 3],
                ])
            }
            0x04 => {
                let lo = self.read_io16(addr) as u32;
                let hi = self.read_io16(addr + 2) as u32;
                lo | (hi << 16)
            }
            0x05 => {
                let a = (addr & 0x3FF) as usize;
                u32::from_le_bytes([
                    self.palette[a],
                    self.palette[a + 1],
                    self.palette[a + 2],
                    self.palette[a + 3],
                ])
            }
            0x06 => {
                let a = self.mirror_vram(addr);
                u32::from_le_bytes([
                    self.vram[a],
                    self.vram[a + 1],
                    self.vram[a + 2],
                    self.vram[a + 3],
                ])
            }
            0x07 => {
                let a = (addr & 0x3FF) as usize;
                u32::from_le_bytes([
                    self.oam[a],
                    self.oam[a + 1],
                    self.oam[a + 2],
                    self.oam[a + 3],
                ])
            }
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                let b0 = *self.rom.get(offset).unwrap_or(&0);
                let b1 = *self.rom.get(offset + 1).unwrap_or(&0);
                let b2 = *self.rom.get(offset + 2).unwrap_or(&0);
                let b3 = *self.rom.get(offset + 3).unwrap_or(&0);
                u32::from_le_bytes([b0, b1, b2, b3])
            }
            _ => 0,
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        match addr >> 24 {
            0x02 => self.ewram[(addr & 0x3FFFF) as usize] = val,
            0x03 => self.iwram[(addr & 0x7FFF) as usize] = val,
            0x04 => {
                // 8-bit I/O writes: need careful handling
                let aligned = addr & !1;
                let mut current = self.read_io16(aligned);
                if addr & 1 == 0 {
                    current = (current & 0xFF00) | val as u16;
                } else {
                    current = (current & 0x00FF) | ((val as u16) << 8);
                }
                self.write_io16(aligned, current);
            }
            0x05 => {
                // Palette: 8-bit writes duplicate to both bytes of the halfword
                let a = (addr & 0x3FE) as usize;
                self.palette[a] = val;
                self.palette[a + 1] = val;
            }
            0x06 => {
                // VRAM: 8-bit writes duplicate to halfword (BG area only)
                let a = self.mirror_vram(addr) & !1;
                self.vram[a] = val;
                self.vram[a + 1] = val;
            }
            // OAM ignores 8-bit writes
            0x0E..=0x0F => {
                self.cart.write(addr, val);
            }
            _ => {}
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        let addr = addr & !1;
        let bytes = val.to_le_bytes();
        match addr >> 24 {
            0x02 => {
                let a = (addr & 0x3FFFF) as usize;
                self.ewram[a] = bytes[0];
                self.ewram[a + 1] = bytes[1];
            }
            0x03 => {
                let a = (addr & 0x7FFF) as usize;
                self.iwram[a] = bytes[0];
                self.iwram[a + 1] = bytes[1];
            }
            0x04 => self.write_io16(addr, val),
            0x05 => {
                let a = (addr & 0x3FF) as usize;
                self.palette[a] = bytes[0];
                self.palette[a + 1] = bytes[1];
            }
            0x06 => {
                let a = self.mirror_vram(addr);
                self.vram[a] = bytes[0];
                self.vram[a + 1] = bytes[1];
            }
            0x07 => {
                let a = (addr & 0x3FF) as usize;
                self.oam[a] = bytes[0];
                self.oam[a + 1] = bytes[1];
            }
            _ => {}
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        let addr = addr & !3;
        let bytes = val.to_le_bytes();
        match addr >> 24 {
            0x02 => {
                let a = (addr & 0x3FFFF) as usize;
                self.ewram[a..a + 4].copy_from_slice(&bytes);
            }
            0x03 => {
                let a = (addr & 0x7FFF) as usize;
                self.iwram[a..a + 4].copy_from_slice(&bytes);
            }
            0x04 => {
                self.write_io16(addr, val as u16);
                self.write_io16(addr + 2, (val >> 16) as u16);
            }
            0x05 => {
                let a = (addr & 0x3FF) as usize;
                self.palette[a..a + 4].copy_from_slice(&bytes);
            }
            0x06 => {
                let a = self.mirror_vram(addr);
                self.vram[a..a + 4].copy_from_slice(&bytes);
            }
            0x07 => {
                let a = (addr & 0x3FF) as usize;
                self.oam[a..a + 4].copy_from_slice(&bytes);
            }
            _ => {}
        }
    }

    fn read_io16(&self, addr: u32) -> u16 {
        let reg = addr & 0x3FF;
        if reg == 0x130 {
            return self.keypad.read();
        }
        self.io.read16(addr)
    }

    fn write_io16(&mut self, addr: u32, val: u16) {
        self.io.write16(addr, val);
    }

    /// VRAM mirroring: 96KB total. Addresses 0x10000-0x17FFF mirror 0x00000-0x07FFF.
    fn mirror_vram(&self, addr: u32) -> usize {
        let offset = addr & 0x1FFFF;
        if offset >= 0x18000 {
            (offset - 0x8000) as usize
        } else {
            offset as usize
        }
    }
}
