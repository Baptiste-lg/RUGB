mod apu;
mod arm7tdmi;
mod bus;
#[cfg(test)]
mod bus_tests;
mod cartridge;
mod dma;
mod io;
#[cfg(test)]
mod io_tests;
mod keypad;
#[cfg(test)]
mod keypad_tests;
mod ppu;
mod timer;

use apu::Apu;
use arm7tdmi::Arm7Tdmi;
use bus::Bus;
use ppu::Ppu;

use wasm_bindgen::prelude::*;

pub struct GbaEmulator {
    cpu: Arm7Tdmi,
    bus: Bus,
    ppu: Ppu,
    apu: Apu,
}

impl GbaEmulator {
    pub fn new(rom: &[u8]) -> Self {
        GbaEmulator {
            cpu: Arm7Tdmi::new(),
            bus: Bus::new(rom.to_vec()),
            ppu: Ppu::new(),
            apu: Apu::new(),
        }
    }

    pub fn run_frame(&mut self) {
        let mut cycles_this_frame: u32 = 0;
        let frame_cycles = ppu::CYCLES_PER_FRAME;

        while cycles_this_frame < frame_cycles {
            let cycles = self.cpu.step(&mut self.bus);

            // Tick PPU
            let ppu_irqs = self.ppu.tick(
                cycles,
                &mut self.bus.io,
                &self.bus.vram[..],
                &self.bus.palette[..],
                &self.bus.oam[..],
            );
            if ppu_irqs != 0 {
                self.bus.io.irq_flags |= ppu_irqs;
            }

            // Flush pending APU register writes from I/O
            if self.bus.io.soundcnt_h_dirty {
                self.apu.write_soundcnt_h(self.bus.io.soundcnt_h);
                self.apu.soundcnt_l = self.bus.io.soundcnt_l;
                self.apu.soundcnt_x = self.bus.io.soundcnt_x;
                self.apu.soundbias = self.bus.io.soundbias;
                self.bus.io.soundcnt_h_dirty = false;
            }
            if let Some((offset, val)) = self.bus.io.psg_pending.take() {
                self.apu.write_psg_register(offset, val);
            }
            if let Some(data) = self.bus.io.fifo_a_pending.take() {
                self.apu.write_fifo_a(data);
            }
            if let Some(data) = self.bus.io.fifo_b_pending.take() {
                self.apu.write_fifo_b(data);
            }

            // Tick timers (check for overflow to drive audio FIFOs)
            let timer_irqs = self.bus.io.timers.tick(cycles);
            if timer_irqs != 0 {
                self.bus.io.irq_flags |= timer_irqs;
                // Timer overflow triggers FIFO sample pop
                for t in 0..2 {
                    if timer_irqs & (1 << (3 + t)) != 0 {
                        self.apu.timer_overflow(t);
                    }
                }
            }

            // Tick APU (generate audio samples)
            self.apu.tick(cycles);

            // Run immediate DMA transfers
            let (_, dma_irqs) = self.bus.io.dma.run_immediate(
                &mut *self.bus.ewram,
                &mut *self.bus.iwram,
                &mut *self.bus.vram,
                &mut *self.bus.palette,
                &mut *self.bus.oam,
                &self.bus.rom,
            );
            if dma_irqs != 0 {
                self.bus.io.irq_flags |= dma_irqs;
            }

            // Check for IRQ
            if self.bus.io.ime != 0
                && self.bus.io.irq_flags & self.bus.io.ie != 0
                && self.cpu.cpsr & arm7tdmi::I_FLAG == 0
            {
                self.cpu.enter_exception(arm7tdmi::CpuMode::Irq, 0x18);
            }

            cycles_this_frame += cycles;
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.ppu.framebuffer[..]
    }

    pub fn has_battery(&self) -> bool {
        !self.bus.cart.sram.is_empty()
    }
}

// --- WASM Bindings ---

#[wasm_bindgen]
pub struct WasmGbaEmulator {
    emu: GbaEmulator,
}

#[wasm_bindgen]
impl WasmGbaEmulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> WasmGbaEmulator {
        console_error_panic_hook::set_once();
        WasmGbaEmulator {
            emu: GbaEmulator::new(rom),
        }
    }

    pub fn run_frame(&mut self) {
        self.emu.run_frame();
    }

    pub fn framebuffer_ptr(&self) -> *const u8 {
        self.emu.ppu.framebuffer.as_ptr()
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.emu.bus.keypad.set_button(button, pressed);
    }

    pub fn title(&self) -> String {
        // GBA ROM title is at offset 0xA0, 12 bytes
        let title_bytes = &self.emu.bus.rom[0xA0..0xAC.min(self.emu.bus.rom.len())];
        title_bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect()
    }

    pub fn audio_ring_ptr(&self) -> *const f32 {
        self.emu.apu.ring_buffer_ptr()
    }

    pub fn audio_ring_available(&self) -> usize {
        self.emu.apu.ring_buffer_available()
    }

    pub fn audio_ring_read_pos(&self) -> usize {
        self.emu.apu.ring_read_pos()
    }

    pub fn audio_ring_capacity(&self) -> usize {
        self.emu.apu.ring_capacity()
    }

    pub fn audio_ring_clear(&mut self) {
        self.emu.apu.ring_clear();
    }

    pub fn audio_ring_consume(&mut self, count: usize) {
        self.emu.apu.ring_consume(count);
    }

    pub fn save_state(&self) -> Vec<u8> {
        let mut d = Vec::new();

        // CPU: current registers (R0-R15) + CPSR
        for &r in &self.emu.cpu.regs {
            d.extend_from_slice(&r.to_le_bytes());
        }
        d.extend_from_slice(&self.emu.cpu.cpsr.to_le_bytes());

        // CPU: banked registers
        for &r in &self.emu.cpu.banked.r13 {
            d.extend_from_slice(&r.to_le_bytes());
        }
        for &r in &self.emu.cpu.banked.r14 {
            d.extend_from_slice(&r.to_le_bytes());
        }
        for &r in &self.emu.cpu.banked.spsr {
            d.extend_from_slice(&r.to_le_bytes());
        }
        for &r in &self.emu.cpu.banked.fiq_r8_r12 {
            d.extend_from_slice(&r.to_le_bytes());
        }
        for &r in &self.emu.cpu.banked.usr_r8_r12 {
            d.extend_from_slice(&r.to_le_bytes());
        }

        // Memory
        d.extend_from_slice(&*self.emu.bus.ewram);
        d.extend_from_slice(&*self.emu.bus.iwram);
        d.extend_from_slice(&*self.emu.bus.palette);
        d.extend_from_slice(&*self.emu.bus.vram);
        d.extend_from_slice(&*self.emu.bus.oam);

        // Cart SRAM (length-prefixed)
        let sram_len = self.emu.bus.cart.sram.len() as u32;
        d.extend_from_slice(&sram_len.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.cart.sram);

        // Key IO registers
        d.extend_from_slice(&self.emu.bus.io.dispcnt.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.io.dispstat.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.io.vcount.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.io.ie.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.io.irq_flags.to_le_bytes());
        d.extend_from_slice(&self.emu.bus.io.ime.to_le_bytes());

        d
    }

    pub fn load_state(&mut self, data: &[u8]) {
        // Helper: read a u32 from a byte cursor
        fn read_u32(cur: &mut &[u8]) -> u32 {
            if cur.len() < 4 {
                return 0;
            }
            let val = u32::from_le_bytes([cur[0], cur[1], cur[2], cur[3]]);
            *cur = &cur[4..];
            val
        }
        fn read_u16(cur: &mut &[u8]) -> u16 {
            if cur.len() < 2 {
                return 0;
            }
            let val = u16::from_le_bytes([cur[0], cur[1]]);
            *cur = &cur[2..];
            val
        }
        fn read_bytes<'a>(cur: &mut &'a [u8], n: usize) -> &'a [u8] {
            if cur.len() < n {
                return &[];
            }
            let (head, tail) = cur.split_at(n);
            *cur = tail;
            head
        }

        let mut cur: &[u8] = data;

        // CPU registers
        for i in 0..16 {
            self.emu.cpu.regs[i] = read_u32(&mut cur);
        }
        self.emu.cpu.cpsr = read_u32(&mut cur);

        // Banked registers
        for i in 0..6 {
            self.emu.cpu.banked.r13[i] = read_u32(&mut cur);
        }
        for i in 0..6 {
            self.emu.cpu.banked.r14[i] = read_u32(&mut cur);
        }
        for i in 0..6 {
            self.emu.cpu.banked.spsr[i] = read_u32(&mut cur);
        }
        for i in 0..5 {
            self.emu.cpu.banked.fiq_r8_r12[i] = read_u32(&mut cur);
        }
        for i in 0..5 {
            self.emu.cpu.banked.usr_r8_r12[i] = read_u32(&mut cur);
        }

        // Memory
        let ewram = read_bytes(&mut cur, 0x40000);
        if ewram.len() == 0x40000 {
            self.emu.bus.ewram.copy_from_slice(ewram);
        }
        let iwram = read_bytes(&mut cur, 0x8000);
        if iwram.len() == 0x8000 {
            self.emu.bus.iwram.copy_from_slice(iwram);
        }
        let palette = read_bytes(&mut cur, 0x400);
        if palette.len() == 0x400 {
            self.emu.bus.palette.copy_from_slice(palette);
        }
        let vram = read_bytes(&mut cur, 0x18000);
        if vram.len() == 0x18000 {
            self.emu.bus.vram.copy_from_slice(vram);
        }
        let oam = read_bytes(&mut cur, 0x400);
        if oam.len() == 0x400 {
            self.emu.bus.oam.copy_from_slice(oam);
        }

        // Cart SRAM
        let sram_len = read_u32(&mut cur) as usize;
        let sram_data = read_bytes(&mut cur, sram_len);
        if sram_data.len() == sram_len {
            let copy_len = sram_len.min(self.emu.bus.cart.sram.len());
            self.emu.bus.cart.sram[..copy_len].copy_from_slice(&sram_data[..copy_len]);
        }

        // IO registers
        self.emu.bus.io.dispcnt = read_u16(&mut cur);
        self.emu.bus.io.dispstat = read_u16(&mut cur);
        self.emu.bus.io.vcount = read_u16(&mut cur);
        self.emu.bus.io.ie = read_u16(&mut cur);
        self.emu.bus.io.irq_flags = read_u16(&mut cur);
        self.emu.bus.io.ime = read_u16(&mut cur);
    }

    pub fn has_battery(&self) -> bool {
        !self.emu.bus.cart.sram.is_empty()
    }

    // --- Debug exports ---

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.emu.bus.read8(addr)
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        self.emu.bus.write8(addr, val);
    }

    pub fn cpu_reg(&self, n: u8) -> u32 {
        if (n as usize) < 16 {
            self.emu.cpu.regs[n as usize]
        } else {
            0
        }
    }

    pub fn cpu_cpsr(&self) -> u32 {
        self.emu.cpu.cpsr
    }

    pub fn step(&mut self) {
        self.emu.cpu.step(&mut self.emu.bus);
    }

    pub fn battery_ram_ptr(&self) -> *const u8 {
        self.emu.bus.cart.sram.as_ptr()
    }

    pub fn battery_ram_len(&self) -> usize {
        self.emu.bus.cart.sram.len()
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.emu.bus.cart.sram.len());
        self.emu.bus.cart.sram[..len].copy_from_slice(&data[..len]);
    }
}
