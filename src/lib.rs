mod cpu;
mod mmu;
mod ppu;
mod apu;
mod timer;
mod joypad;
mod cartridge;
mod interrupt;

use cpu::Cpu;
use mmu::Mmu;
use ppu::Ppu;
use timer::Timer;
use joypad::Joypad;
use apu::Apu;
use interrupt::handle_interrupts;

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
            self.mmu.ppu.tick(interrupt_cycles, &mut self.mmu.interrupt_flag);
            self.mmu.timer.tick(interrupt_cycles, &mut self.mmu.interrupt_flag);
            return interrupt_cycles;
        }

        let cycles = self.cpu.step(&mut self.mmu);
        self.mmu.ppu.tick(cycles, &mut self.mmu.interrupt_flag);
        self.mmu.timer.tick(cycles, &mut self.mmu.interrupt_flag);
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
        self.emu.mmu.joypad.set_button(button, pressed, &mut self.emu.mmu.interrupt_flag);
    }

    pub fn title(&self) -> String {
        self.emu.mmu.cartridge_title()
    }
}
