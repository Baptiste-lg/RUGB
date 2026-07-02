mod apu;
mod cartridge;
mod cpu;
mod interrupt;
mod joypad;
mod mmu;
mod ppu;
pub mod savestate;
mod timer;

use cpu::Cpu;
use interrupt::handle_interrupts;
use mmu::Mmu;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

pub struct Emulator {
    cpu: Cpu,
    mmu: Mmu,
}

impl Emulator {
    pub fn new(rom: &[u8]) -> Self {
        let mut mmu = Mmu::new();
        mmu.load_rom(rom);
        Emulator {
            cpu: Cpu::new(),
            mmu,
        }
    }

    pub fn step(&mut self) -> u32 {
        let interrupt_cycles = handle_interrupts(&mut self.cpu, &mut self.mmu);
        if interrupt_cycles > 0 {
            self.mmu
                .ppu
                .tick(interrupt_cycles, &mut self.mmu.interrupt_flag);
            self.mmu
                .timer
                .tick(interrupt_cycles, &mut self.mmu.interrupt_flag);
            self.mmu.apu.tick(interrupt_cycles);
            return interrupt_cycles;
        }

        let cycles = self.cpu.step(&mut self.mmu);
        self.mmu.ppu.tick(cycles, &mut self.mmu.interrupt_flag);
        self.mmu.timer.tick(cycles, &mut self.mmu.interrupt_flag);
        self.mmu.apu.tick(cycles);
        cycles
    }

    /// Run until one full frame is rendered (70224 T-cycles)
    pub fn run_frame(&mut self) {
        let mut cycles_this_frame: u32 = 0;
        while cycles_this_frame < 70224 {
            cycles_this_frame += self.step();
        }
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.mmu.ppu.framebuffer
    }

    pub fn save_state(&self) -> Vec<u8> {
        let mut data = Vec::new();
        self.cpu.save_state(&mut data);
        self.mmu.save_state(&mut data);
        data
    }

    pub fn load_state(&mut self, data: &[u8]) {
        let mut cursor: &[u8] = data;
        self.cpu.load_state(&mut cursor);
        self.mmu.load_state(&mut cursor);
    }
}

#[wasm_bindgen]
pub struct WasmEmulator {
    emu: Emulator,
}

#[wasm_bindgen]
impl WasmEmulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> WasmEmulator {
        console_error_panic_hook::set_once();
        WasmEmulator {
            emu: Emulator::new(rom),
        }
    }

    pub fn run_frame(&mut self) {
        self.emu.run_frame();
    }

    pub fn framebuffer_ptr(&self) -> *const u8 {
        self.emu.mmu.ppu.framebuffer.as_ptr()
    }

    pub fn set_button(&mut self, button: u8, pressed: bool) {
        self.emu
            .mmu
            .joypad
            .set_button(button, pressed, &mut self.emu.mmu.interrupt_flag);
    }

    pub fn title(&self) -> String {
        self.emu.mmu.cartridge_title()
    }

    /// Returns channel state as [freq_hz, volume, enabled] for Web Audio.
    /// ch: 1-4
    pub fn channel_freq(&self, ch: u8) -> f64 {
        match ch {
            1 if self.emu.mmu.apu.ch1.enabled => self.emu.mmu.apu.ch1.frequency_hz(),
            2 if self.emu.mmu.apu.ch2.enabled => self.emu.mmu.apu.ch2.frequency_hz(),
            3 if self.emu.mmu.apu.ch3.enabled => self.emu.mmu.apu.ch3.frequency_hz(),
            _ => 0.0,
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        self.emu.save_state()
    }

    pub fn load_state(&mut self, data: &[u8]) {
        self.emu.load_state(data);
    }

    pub fn channel_volume(&self, ch: u8) -> f64 {
        match ch {
            1 if self.emu.mmu.apu.ch1.enabled => self.emu.mmu.apu.ch1.volume as f64 / 15.0,
            2 if self.emu.mmu.apu.ch2.enabled => self.emu.mmu.apu.ch2.volume as f64 / 15.0,
            3 if self.emu.mmu.apu.ch3.enabled => match self.emu.mmu.apu.ch3.volume_shift {
                1 => 1.0,
                2 => 0.5,
                3 => 0.25,
                _ => 0.0,
            },
            4 if self.emu.mmu.apu.ch4.enabled => self.emu.mmu.apu.ch4.volume as f64 / 15.0,
            _ => 0.0,
        }
    }
}
