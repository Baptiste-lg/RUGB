mod apu;
mod arm7tdmi;
mod bus;
#[cfg(test)]
mod bus_tests;
mod cartridge;
mod dma;
mod io;
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
                &mut self.bus.ewram,
                &mut self.bus.iwram,
                &mut self.bus.vram,
                &mut self.bus.palette,
                &mut self.bus.oam,
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
        Vec::new() // TODO: implement save states
    }

    pub fn load_state(&mut self, _data: &[u8]) {
        // TODO: implement save states
    }

    pub fn has_battery(&self) -> bool {
        !self.emu.bus.cart.sram.is_empty()
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
