mod arm7tdmi;
mod bus;
#[cfg(test)]
mod bus_tests;
mod io;
mod keypad;
#[cfg(test)]
mod keypad_tests;
mod ppu;

use arm7tdmi::Arm7Tdmi;
use bus::Bus;
use ppu::Ppu;

use wasm_bindgen::prelude::*;

const RING_BUFFER_CAPACITY: usize = 8192;

pub struct GbaEmulator {
    cpu: Arm7Tdmi,
    bus: Bus,
    ppu: Ppu,
    // Audio ring buffer (silent for now — Phase 4)
    audio_ring: Box<[f32; RING_BUFFER_CAPACITY]>,
    audio_write_pos: usize,
    audio_read_pos: usize,
}

impl GbaEmulator {
    pub fn new(rom: &[u8]) -> Self {
        GbaEmulator {
            cpu: Arm7Tdmi::new(),
            bus: Bus::new(rom.to_vec()),
            ppu: Ppu::new(),
            audio_ring: Box::new([0.0; RING_BUFFER_CAPACITY]),
            audio_write_pos: 0,
            audio_read_pos: 0,
        }
    }

    pub fn run_frame(&mut self) {
        let mut cycles_this_frame: u32 = 0;
        let frame_cycles = ppu::CYCLES_PER_FRAME;

        while cycles_this_frame < frame_cycles {
            let cycles = self.cpu.step(&mut self.bus);
            let irqs = self.ppu.tick(
                cycles,
                &mut self.bus.io,
                &self.bus.vram[..],
                &self.bus.palette[..],
            );

            // Raise IRQ if pending and enabled
            if irqs != 0 {
                self.bus.io.irq_flags |= irqs;
            }
            if self.bus.io.ime != 0
                && self.bus.io.irq_flags & self.bus.io.ie != 0
                && self.cpu.cpsr & arm7tdmi::I_FLAG == 0
            {
                self.cpu
                    .enter_exception(arm7tdmi::CpuMode::Irq, 0x18);
            }

            cycles_this_frame += cycles;
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.ppu.framebuffer[..]
    }

    fn audio_available(&self) -> usize {
        (self.audio_write_pos + RING_BUFFER_CAPACITY - self.audio_read_pos) % RING_BUFFER_CAPACITY
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

    // Audio interface (silent stub for now — returns empty buffer)
    pub fn audio_ring_ptr(&self) -> *const f32 {
        self.emu.audio_ring.as_ptr()
    }

    pub fn audio_ring_available(&self) -> usize {
        self.emu.audio_available()
    }

    pub fn audio_ring_read_pos(&self) -> usize {
        self.emu.audio_read_pos
    }

    pub fn audio_ring_capacity(&self) -> usize {
        RING_BUFFER_CAPACITY
    }

    pub fn audio_ring_clear(&mut self) {
        self.emu.audio_read_pos = self.emu.audio_write_pos;
    }

    pub fn audio_ring_consume(&mut self, count: usize) {
        let avail = self.emu.audio_available();
        let count = count.min(avail);
        self.emu.audio_read_pos =
            (self.emu.audio_read_pos + count) % RING_BUFFER_CAPACITY;
    }

    pub fn save_state(&self) -> Vec<u8> {
        Vec::new() // TODO: implement save states
    }

    pub fn load_state(&mut self, _data: &[u8]) {
        // TODO: implement save states
    }

    pub fn has_battery(&self) -> bool {
        !self.emu.bus.sram.is_empty()
    }

    pub fn battery_ram_ptr(&self) -> *const u8 {
        self.emu.bus.sram.as_ptr()
    }

    pub fn battery_ram_len(&self) -> usize {
        self.emu.bus.sram.len()
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.emu.bus.sram.len());
        self.emu.bus.sram[..len].copy_from_slice(&data[..len]);
    }
}
