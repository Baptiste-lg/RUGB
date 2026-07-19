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

    pub fn new_with_boot(rom: &[u8], boot_rom: &[u8]) -> Self {
        let mut mmu = Mmu::new();
        mmu.load_rom(rom);
        mmu.set_boot_rom(boot_rom.to_vec());
        let mut cpu = Cpu::new();
        // Boot ROM starts execution at 0x0000 with zeroed registers
        cpu.regs.reset_for_boot();
        Emulator { cpu, mmu }
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
            self.mmu.tick_cartridge_rtc(interrupt_cycles);
            return interrupt_cycles;
        }

        let cycles = self.cpu.step(&mut self.mmu);
        self.mmu.ppu.tick(cycles, &mut self.mmu.interrupt_flag);
        self.mmu.timer.tick(cycles, &mut self.mmu.interrupt_flag);
        self.mmu.apu.tick(cycles);
        self.mmu.tick_cartridge_rtc(cycles);
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

    pub fn new_with_boot(rom: &[u8], boot_rom: &[u8]) -> WasmEmulator {
        console_error_panic_hook::set_once();
        WasmEmulator {
            emu: Emulator::new_with_boot(rom, boot_rom),
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

    pub fn audio_ring_ptr(&self) -> *const f32 {
        self.emu.mmu.apu.ring_buffer_ptr()
    }

    pub fn audio_ring_available(&self) -> usize {
        self.emu.mmu.apu.ring_buffer_available()
    }

    pub fn audio_ring_read_pos(&self) -> usize {
        self.emu.mmu.apu.ring_read_pos()
    }

    pub fn audio_ring_capacity(&self) -> usize {
        self.emu.mmu.apu.ring_capacity()
    }

    pub fn audio_ring_clear(&mut self) {
        self.emu.mmu.apu.ring_clear();
    }

    pub fn audio_ring_consume(&mut self, count: usize) {
        self.emu.mmu.apu.ring_consume(count);
    }

    pub fn has_battery(&self) -> bool {
        self.emu.mmu.has_battery()
    }

    pub fn battery_ram_ptr(&self) -> *const u8 {
        self.emu.mmu.battery_ram().as_ptr()
    }

    pub fn battery_ram_len(&self) -> usize {
        self.emu.mmu.battery_ram().len()
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        self.emu.mmu.load_battery_ram(data);
    }

    pub fn set_channel_mute(&mut self, channel: u8, muted: bool) {
        if (channel as usize) < 4 {
            self.emu.mmu.apu.ch_mute[channel as usize] = muted;
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        self.emu.save_state()
    }

    pub fn load_state(&mut self, data: &[u8]) {
        self.emu.load_state(data);
    }

    pub fn rumble(&self) -> bool {
        self.emu.mmu.rumble()
    }

    /// Add a Game Genie cheat (intercepts ROM reads).
    /// compare = 0xFF means no compare byte (6-char code).
    pub fn add_gg_cheat(&mut self, addr: u16, new_val: u8, compare: u8) {
        let cmp = if compare == 0xFF { None } else { Some(compare) };
        self.emu.mmu.add_gg_cheat(addr, new_val, cmp);
    }

    /// Write a value to any address (GameShark poke).
    pub fn poke_byte(&mut self, addr: u16, val: u8) {
        self.emu.mmu.poke(addr, val);
    }

    /// Remove all active cheats.
    pub fn clear_cheats(&mut self) {
        self.emu.mmu.clear_cheats();
    }

    /// Set RTC offset in seconds (positive = advance, negative = rewind clock).
    pub fn set_rtc_offset(&mut self, seconds: i32) {
        self.emu.mmu.set_rtc_offset(seconds);
    }
}
