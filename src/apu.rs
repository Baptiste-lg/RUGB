//! APU — Audio Processing Unit with 4 channels.
//!
//! Channel 1: Square wave with frequency sweep
//! Channel 2: Square wave (no sweep)
//! Channel 3: Programmable wave
//! Channel 4: Noise (LFSR-based)
//!
//! The frame sequencer runs at 512 Hz and clocks:
//!   Steps 0,2,4,6 — length counters
//!   Steps 2,6     — frequency sweep (CH1 only)
//!   Step 7        — volume envelope (CH1, CH2, CH4)

use crate::savestate::*;

const CPU_CLOCK: u32 = 4_194_304;
const SAMPLE_RATE: u32 = 48_000;
const MAX_BUFFER_SAMPLES: usize = 4096;

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75%
];

const NOISE_DIVISORS: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

/// Capacitor high-pass filter matching the Game Boy's DC-blocking output stage.
/// Removes DC offset to prevent pops on channel enable/disable and envelope changes.
const HIGHPASS_ALPHA: f32 = 0.999;

pub struct Apu {
    pub enabled: bool,
    pub ch1: SquareChannel,
    pub ch2: SquareChannel,
    pub ch3: WaveChannel,
    pub ch4: NoiseChannel,
    /// NR50 — master volume / VIN panning (bits 0-2 = right vol, 4-6 = left vol)
    nr50: u8,
    /// NR51 — channel panning (which channels go to left/right)
    nr51: u8,
    /// Frame sequencer step (0-7), clocked at 512 Hz
    frame_step: u8,
    /// T-cycle accumulator for the 512 Hz frame sequencer (8192 T-cycles per step)
    frame_cycles: u32,
    /// Wave RAM (16 bytes at 0xFF30-0xFF3F), holds 32 4-bit samples
    wave_ram: [u8; 16],
    /// Audio sample buffer (interleaved L/R f32 pairs)
    pub sample_buffer: Vec<f32>,
    /// Accumulator for sample timing
    sample_clock: u32,
    /// Per-channel mute flags (controlled from frontend, not saved in state)
    pub ch_mute: [bool; 4],
    // High-pass filter state (emulates the hardware coupling capacitor)
    hp_left_in: f32,
    hp_left_out: f32,
    hp_right_in: f32,
    hp_right_out: f32,
}

pub struct SquareChannel {
    pub enabled: bool,
    /// NRx1 bits 6-7: duty cycle (0=12.5%, 1=25%, 2=50%, 3=75%)
    pub duty: u8,
    /// Current volume (0-15)
    pub volume: u8,
    /// 11-bit frequency register value. Actual freq = 131072 / (2048 - freq_raw) Hz.
    pub freq_raw: u16,
    // Length
    length_counter: u16,
    length_enabled: bool,
    // Volume envelope
    env_initial: u8,
    env_direction: i8, // +1 or -1
    env_period: u8,
    env_timer: u8,
    // Sweep (CH1 only, ignored for CH2)
    sweep_period: u8,
    sweep_direction: i8,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,
    has_sweep: bool,
    /// Frequency timer — counts down each T-cycle, reloads from (2048 - freq_raw) * 4
    freq_timer: u32,
    /// Current position in the 8-step duty cycle pattern (0-7)
    duty_position: u8,
}

pub struct WaveChannel {
    pub enabled: bool,
    pub volume_shift: u8, // 0=mute, 1=100%, 2=50%, 3=25%
    pub freq_raw: u16,
    length_counter: u16,
    length_enabled: bool,
    dac_enabled: bool,
    /// Frequency timer — counts down each T-cycle, reloads from (2048 - freq_raw) * 2
    freq_timer: u32,
    /// Current position in the 32-sample wave (0-31)
    wave_position: u8,
}

pub struct NoiseChannel {
    pub enabled: bool,
    pub volume: u8,
    length_counter: u16,
    length_enabled: bool,
    env_initial: u8,
    env_direction: i8,
    env_period: u8,
    env_timer: u8,
    /// Clock shift and divisor control the noise frequency
    pub clock_shift: u8,
    pub divisor_code: u8,
    pub width_mode: bool, // false=15-bit LFSR, true=7-bit
    /// Frequency timer — counts down each T-cycle
    freq_timer: u32,
    /// 15-bit Linear Feedback Shift Register
    lfsr: u16,
}

impl SquareChannel {
    fn new(has_sweep: bool) -> Self {
        SquareChannel {
            enabled: false,
            duty: 0,
            volume: 0,
            freq_raw: 0,
            length_counter: 0,
            length_enabled: false,
            env_initial: 0,
            env_direction: 0,
            env_period: 0,
            env_timer: 0,
            sweep_period: 0,
            sweep_direction: 0,
            sweep_shift: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            sweep_shadow: 0,
            has_sweep,
            freq_timer: 0,
            duty_position: 0,
        }
    }

    fn dac_enabled(&self) -> bool {
        self.env_initial > 0 || self.env_direction > 0
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled();
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.volume = self.env_initial;
        self.env_timer = self.env_period;
        self.freq_timer = (2048 - self.freq_raw as u32) * 4;
        self.duty_position = 0;
        if self.has_sweep {
            self.sweep_shadow = self.freq_raw;
            self.sweep_timer = if self.sweep_period > 0 {
                self.sweep_period
            } else {
                8
            };
            self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
            // Overflow check on trigger
            if self.sweep_shift > 0 && self.sweep_calc_freq() > 2047 {
                self.enabled = false;
            }
        }
    }

    fn sweep_calc_freq(&self) -> u16 {
        let delta = self.sweep_shadow >> self.sweep_shift;
        if self.sweep_direction < 0 {
            self.sweep_shadow.wrapping_sub(delta)
        } else {
            self.sweep_shadow.wrapping_add(delta)
        }
    }

    fn clock_sweep(&mut self) {
        if !self.sweep_enabled || self.sweep_period == 0 {
            return;
        }
        self.sweep_timer = self.sweep_timer.saturating_sub(1);
        if self.sweep_timer > 0 {
            return;
        }
        self.sweep_timer = if self.sweep_period > 0 {
            self.sweep_period
        } else {
            8
        };
        let new_freq = self.sweep_calc_freq();
        if new_freq > 2047 {
            self.enabled = false;
        } else if self.sweep_shift > 0 {
            self.sweep_shadow = new_freq;
            self.freq_raw = new_freq;
            // Second overflow check after writing
            if self.sweep_calc_freq() > 2047 {
                self.enabled = false;
            }
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.env_period == 0 {
            return;
        }
        self.env_timer = self.env_timer.saturating_sub(1);
        if self.env_timer > 0 {
            return;
        }
        self.env_timer = self.env_period;
        let new_vol = self.volume as i8 + self.env_direction;
        if (0..=15).contains(&new_vol) {
            self.volume = new_vol as u8;
        }
    }

    fn tick_freq(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = (2048 - self.freq_raw as u32) * 4;
            self.duty_position = (self.duty_position + 1) & 7;
        }
    }

    fn output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled() {
            return 0.0;
        }
        let dac_input = DUTY_TABLE[self.duty as usize][self.duty_position as usize] * self.volume;
        dac_input as f32 / 7.5 - 1.0
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.enabled);
        push_u8(d, self.duty);
        push_u8(d, self.volume);
        push_u16(d, self.freq_raw);
        push_u16(d, self.length_counter);
        push_bool(d, self.length_enabled);
        push_u8(d, self.env_initial);
        push_i8(d, self.env_direction);
        push_u8(d, self.env_period);
        push_u8(d, self.env_timer);
        push_u8(d, self.sweep_period);
        push_i8(d, self.sweep_direction);
        push_u8(d, self.sweep_shift);
        push_u8(d, self.sweep_timer);
        push_bool(d, self.sweep_enabled);
        push_u16(d, self.sweep_shadow);
        push_bool(d, self.has_sweep);
        push_u32(d, self.freq_timer);
        push_u8(d, self.duty_position);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.enabled = pop_bool(d);
        self.duty = pop_u8(d);
        self.volume = pop_u8(d);
        self.freq_raw = pop_u16(d);
        self.length_counter = pop_u16(d);
        self.length_enabled = pop_bool(d);
        self.env_initial = pop_u8(d);
        self.env_direction = pop_i8(d);
        self.env_period = pop_u8(d);
        self.env_timer = pop_u8(d);
        self.sweep_period = pop_u8(d);
        self.sweep_direction = pop_i8(d);
        self.sweep_shift = pop_u8(d);
        self.sweep_timer = pop_u8(d);
        self.sweep_enabled = pop_bool(d);
        self.sweep_shadow = pop_u16(d);
        self.has_sweep = pop_bool(d);
        self.freq_timer = pop_u32(d);
        self.duty_position = pop_u8(d);
    }
}

impl WaveChannel {
    fn new() -> Self {
        WaveChannel {
            enabled: false,
            volume_shift: 0,
            freq_raw: 0,
            length_counter: 0,
            length_enabled: false,
            dac_enabled: false,
            freq_timer: 0,
            wave_position: 0,
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
        }
        self.freq_timer = (2048 - self.freq_raw as u32) * 2;
        self.wave_position = 0;
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn tick_freq(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = (2048 - self.freq_raw as u32) * 2;
            self.wave_position = (self.wave_position + 1) & 31;
        }
    }

    fn output(&self, wave_ram: &[u8; 16]) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let byte = wave_ram[self.wave_position as usize / 2];
        let sample = if self.wave_position & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };
        let shifted = match self.volume_shift {
            0 => 0,
            1 => sample,
            2 => sample >> 1,
            3 => sample >> 2,
            _ => 0,
        };
        shifted as f32 / 7.5 - 1.0
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.enabled);
        push_u8(d, self.volume_shift);
        push_u16(d, self.freq_raw);
        push_u16(d, self.length_counter);
        push_bool(d, self.length_enabled);
        push_bool(d, self.dac_enabled);
        push_u32(d, self.freq_timer);
        push_u8(d, self.wave_position);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.enabled = pop_bool(d);
        self.volume_shift = pop_u8(d);
        self.freq_raw = pop_u16(d);
        self.length_counter = pop_u16(d);
        self.length_enabled = pop_bool(d);
        self.dac_enabled = pop_bool(d);
        self.freq_timer = pop_u32(d);
        self.wave_position = pop_u8(d);
    }
}

impl NoiseChannel {
    fn new() -> Self {
        NoiseChannel {
            enabled: false,
            volume: 0,
            length_counter: 0,
            length_enabled: false,
            env_initial: 0,
            env_direction: 0,
            env_period: 0,
            env_timer: 0,
            clock_shift: 0,
            divisor_code: 0,
            width_mode: false,
            freq_timer: 0,
            lfsr: 0x7FFF,
        }
    }

    fn dac_enabled(&self) -> bool {
        self.env_initial > 0 || self.env_direction > 0
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled();
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.volume = self.env_initial;
        self.env_timer = self.env_period;
        self.lfsr = 0x7FFF;
        self.freq_timer = NOISE_DIVISORS[self.divisor_code as usize] << self.clock_shift;
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.env_period == 0 {
            return;
        }
        self.env_timer = self.env_timer.saturating_sub(1);
        if self.env_timer > 0 {
            return;
        }
        self.env_timer = self.env_period;
        let new_vol = self.volume as i8 + self.env_direction;
        if (0..=15).contains(&new_vol) {
            self.volume = new_vol as u8;
        }
    }

    fn tick_freq(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = NOISE_DIVISORS[self.divisor_code as usize] << self.clock_shift;
            // Clock the LFSR
            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.width_mode {
                self.lfsr = (self.lfsr & !(1 << 6)) | (xor_bit << 6);
            }
        }
    }

    fn output(&self) -> f32 {
        if !self.enabled || !self.dac_enabled() {
            return 0.0;
        }
        let bit = (self.lfsr & 1) ^ 1; // inverted bit 0
        let dac_input = bit as u8 * self.volume;
        dac_input as f32 / 7.5 - 1.0
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.enabled);
        push_u8(d, self.volume);
        push_u16(d, self.length_counter);
        push_bool(d, self.length_enabled);
        push_u8(d, self.env_initial);
        push_i8(d, self.env_direction);
        push_u8(d, self.env_period);
        push_u8(d, self.env_timer);
        push_u8(d, self.clock_shift);
        push_u8(d, self.divisor_code);
        push_bool(d, self.width_mode);
        push_u32(d, self.freq_timer);
        push_u16(d, self.lfsr);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.enabled = pop_bool(d);
        self.volume = pop_u8(d);
        self.length_counter = pop_u16(d);
        self.length_enabled = pop_bool(d);
        self.env_initial = pop_u8(d);
        self.env_direction = pop_i8(d);
        self.env_period = pop_u8(d);
        self.env_timer = pop_u8(d);
        self.clock_shift = pop_u8(d);
        self.divisor_code = pop_u8(d);
        self.width_mode = pop_bool(d);
        self.freq_timer = pop_u32(d);
        self.lfsr = pop_u16(d);
    }
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            enabled: true,
            ch1: SquareChannel::new(true),
            ch2: SquareChannel::new(false),
            ch3: WaveChannel::new(),
            ch4: NoiseChannel::new(),
            nr50: 0x77,
            nr51: 0xF3,
            frame_step: 0,
            frame_cycles: 0,
            wave_ram: [0; 16],
            sample_buffer: Vec::with_capacity(MAX_BUFFER_SAMPLES * 2),
            sample_clock: 0,
            ch_mute: [false; 4],
            hp_left_in: 0.0,
            hp_left_out: 0.0,
            hp_right_in: 0.0,
            hp_right_out: 0.0,
        }
    }

    /// Advance APU by the given number of T-cycles. Generates audio samples.
    pub fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            if self.enabled {
                // Frame sequencer ticks every 8192 T-cycles (512 Hz)
                self.frame_cycles += 1;
                if self.frame_cycles >= 8192 {
                    self.frame_cycles = 0;
                    self.clock_frame_sequencer();
                }

                // Tick channel frequency timers
                self.ch1.tick_freq();
                self.ch2.tick_freq();
                self.ch3.tick_freq();
                self.ch4.tick_freq();
            }

            // Always generate samples at the target rate
            self.sample_clock += SAMPLE_RATE;
            if self.sample_clock >= CPU_CLOCK {
                self.sample_clock -= CPU_CLOCK;
                if self.sample_buffer.len() < MAX_BUFFER_SAMPLES * 2 {
                    if self.enabled {
                        self.generate_sample();
                    } else {
                        self.sample_buffer.push(0.0);
                        self.sample_buffer.push(0.0);
                    }
                }
            }
        }
    }

    fn clock_frame_sequencer(&mut self) {
        match self.frame_step {
            0 | 4 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            2 | 6 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
                self.ch1.clock_sweep();
            }
            7 => {
                self.ch1.clock_envelope();
                self.ch2.clock_envelope();
                self.ch4.clock_envelope();
            }
            _ => {}
        }
        self.frame_step = (self.frame_step + 1) & 7;
    }

    fn generate_sample(&mut self) {
        let ch1_out = if self.ch_mute[0] { 0.0 } else { self.ch1.output() };
        let ch2_out = if self.ch_mute[1] { 0.0 } else { self.ch2.output() };
        let ch3_out = if self.ch_mute[2] { 0.0 } else { self.ch3.output(&self.wave_ram) };
        let ch4_out = if self.ch_mute[3] { 0.0 } else { self.ch4.output() };

        let left_vol = ((self.nr50 >> 4) & 7) as f32 + 1.0;
        let right_vol = (self.nr50 & 7) as f32 + 1.0;

        let mut left = 0.0f32;
        let mut right = 0.0f32;

        // NR51 panning: bits 7-4 = left ch4-ch1, bits 3-0 = right ch4-ch1
        if self.nr51 & 0x10 != 0 {
            left += ch1_out;
        }
        if self.nr51 & 0x20 != 0 {
            left += ch2_out;
        }
        if self.nr51 & 0x40 != 0 {
            left += ch3_out;
        }
        if self.nr51 & 0x80 != 0 {
            left += ch4_out;
        }

        if self.nr51 & 0x01 != 0 {
            right += ch1_out;
        }
        if self.nr51 & 0x02 != 0 {
            right += ch2_out;
        }
        if self.nr51 & 0x04 != 0 {
            right += ch3_out;
        }
        if self.nr51 & 0x08 != 0 {
            right += ch4_out;
        }

        // Apply master volume, normalize to roughly -1.0..1.0
        // Max per side: 4 channels * 1.0 amplitude * 8.0 master = 32.0
        left *= left_vol / 32.0;
        right *= right_vol / 32.0;

        // High-pass filter (DC-blocking capacitor emulation)
        let hp_left = left - self.hp_left_in + HIGHPASS_ALPHA * self.hp_left_out;
        self.hp_left_in = left;
        self.hp_left_out = hp_left;

        let hp_right = right - self.hp_right_in + HIGHPASS_ALPHA * self.hp_right_out;
        self.hp_right_in = right;
        self.hp_right_out = hp_right;

        self.sample_buffer.push(hp_left);
        self.sample_buffer.push(hp_right);
    }

    pub fn sample_buffer_ptr(&self) -> *const f32 {
        self.sample_buffer.as_ptr()
    }

    pub fn sample_buffer_len(&self) -> usize {
        self.sample_buffer.len()
    }

    pub fn drain_samples(&mut self) {
        self.sample_buffer.clear();
    }

    /// Remove the first `count` f32 values from the sample buffer.
    /// Used by the JS audio callback to consume only the samples it actually read.
    pub fn consume_samples(&mut self, count: usize) {
        let count = count.min(self.sample_buffer.len());
        self.sample_buffer.drain(..count);
    }

    pub fn save_state(&self, d: &mut Vec<u8>) {
        push_bool(d, self.enabled);
        self.ch1.save_state(d);
        self.ch2.save_state(d);
        self.ch3.save_state(d);
        self.ch4.save_state(d);
        push_u8(d, self.nr50);
        push_u8(d, self.nr51);
        push_u8(d, self.frame_step);
        push_u32(d, self.frame_cycles);
        d.extend_from_slice(&self.wave_ram);
        push_u32(d, self.sample_clock);
    }

    pub fn load_state(&mut self, d: &mut &[u8]) {
        self.enabled = pop_bool(d);
        self.ch1.load_state(d);
        self.ch2.load_state(d);
        self.ch3.load_state(d);
        self.ch4.load_state(d);
        self.nr50 = pop_u8(d);
        self.nr51 = pop_u8(d);
        self.frame_step = pop_u8(d);
        self.frame_cycles = pop_u32(d);
        self.wave_ram.copy_from_slice(&d[..16]);
        *d = &d[16..];
        self.sample_clock = pop_u32(d);
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // NR10 — CH1 sweep
            0xFF10 => {
                0x80 | (self.ch1.sweep_period << 4)
                    | if self.ch1.sweep_direction < 0 {
                        0x08
                    } else {
                        0
                    }
                    | self.ch1.sweep_shift
            }
            // NR11 — CH1 duty + length (only duty readable)
            0xFF11 => (self.ch1.duty << 6) | 0x3F,
            // NR12 — CH1 volume envelope
            0xFF12 => {
                (self.ch1.env_initial << 4)
                    | if self.ch1.env_direction > 0 { 0x08 } else { 0 }
                    | self.ch1.env_period
            }
            // NR13 — CH1 freq low (write-only)
            0xFF13 => 0xFF,
            // NR14 — CH1 freq high + trigger + length enable
            0xFF14 => 0xBF | if self.ch1.length_enabled { 0x40 } else { 0 },

            // NR21-NR24 — CH2
            0xFF16 => (self.ch2.duty << 6) | 0x3F,
            0xFF17 => {
                (self.ch2.env_initial << 4)
                    | if self.ch2.env_direction > 0 { 0x08 } else { 0 }
                    | self.ch2.env_period
            }
            0xFF18 => 0xFF,
            0xFF19 => 0xBF | if self.ch2.length_enabled { 0x40 } else { 0 },

            // NR30-NR34 — CH3
            0xFF1A => {
                if self.ch3.dac_enabled {
                    0xFF
                } else {
                    0x7F
                }
            }
            0xFF1B => 0xFF,
            0xFF1C => (self.ch3.volume_shift << 5) | 0x9F,
            0xFF1D => 0xFF,
            0xFF1E => 0xBF | if self.ch3.length_enabled { 0x40 } else { 0 },

            // NR41-NR44 — CH4
            0xFF20 => 0xFF,
            0xFF21 => {
                (self.ch4.env_initial << 4)
                    | if self.ch4.env_direction > 0 { 0x08 } else { 0 }
                    | self.ch4.env_period
            }
            0xFF22 => {
                (self.ch4.clock_shift << 4)
                    | if self.ch4.width_mode { 0x08 } else { 0 }
                    | self.ch4.divisor_code
            }
            0xFF23 => 0xBF | if self.ch4.length_enabled { 0x40 } else { 0 },

            // NR50/NR51/NR52
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let mut val = 0x70; // bits 4-6 always 1
                if self.enabled {
                    val |= 0x80;
                }
                if self.ch1.enabled {
                    val |= 0x01;
                }
                if self.ch2.enabled {
                    val |= 0x02;
                }
                if self.ch3.enabled {
                    val |= 0x04;
                }
                if self.ch4.enabled {
                    val |= 0x08;
                }
                val
            }

            // Wave RAM
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize],

            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        // NR52 power control — writing with bit 7 clear disables APU
        if addr == 0xFF26 {
            self.enabled = val & 0x80 != 0;
            if !self.enabled {
                self.ch1 = SquareChannel::new(true);
                self.ch2 = SquareChannel::new(false);
                self.ch3 = WaveChannel::new();
                self.ch4 = NoiseChannel::new();
                self.nr50 = 0;
                self.nr51 = 0;
            }
            return;
        }

        // Ignore all writes except NR52 and wave RAM when APU is off
        if !self.enabled && !(0xFF30..=0xFF3F).contains(&addr) {
            return;
        }

        match addr {
            // CH1 — Square with sweep
            0xFF10 => {
                self.ch1.sweep_period = (val >> 4) & 0x07;
                self.ch1.sweep_direction = if val & 0x08 != 0 { -1 } else { 1 };
                self.ch1.sweep_shift = val & 0x07;
            }
            0xFF11 => {
                self.ch1.duty = (val >> 6) & 0x03;
                self.ch1.length_counter = 64 - (val & 0x3F) as u16;
            }
            0xFF12 => {
                self.ch1.env_initial = val >> 4;
                self.ch1.env_direction = if val & 0x08 != 0 { 1 } else { -1 };
                self.ch1.env_period = val & 0x07;
                if !self.ch1.dac_enabled() {
                    self.ch1.enabled = false;
                }
            }
            0xFF13 => {
                self.ch1.freq_raw = (self.ch1.freq_raw & 0x700) | val as u16;
            }
            0xFF14 => {
                self.ch1.freq_raw = (self.ch1.freq_raw & 0xFF) | ((val as u16 & 0x07) << 8);
                self.ch1.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch1.trigger();
                }
            }

            // CH2 — Square (no sweep)
            0xFF15 => {} // NR20 doesn't exist
            0xFF16 => {
                self.ch2.duty = (val >> 6) & 0x03;
                self.ch2.length_counter = 64 - (val & 0x3F) as u16;
            }
            0xFF17 => {
                self.ch2.env_initial = val >> 4;
                self.ch2.env_direction = if val & 0x08 != 0 { 1 } else { -1 };
                self.ch2.env_period = val & 0x07;
                if !self.ch2.dac_enabled() {
                    self.ch2.enabled = false;
                }
            }
            0xFF18 => {
                self.ch2.freq_raw = (self.ch2.freq_raw & 0x700) | val as u16;
            }
            0xFF19 => {
                self.ch2.freq_raw = (self.ch2.freq_raw & 0xFF) | ((val as u16 & 0x07) << 8);
                self.ch2.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch2.trigger();
                }
            }

            // CH3 — Wave
            0xFF1A => {
                self.ch3.dac_enabled = val & 0x80 != 0;
                if !self.ch3.dac_enabled {
                    self.ch3.enabled = false;
                }
            }
            0xFF1B => {
                self.ch3.length_counter = 256 - val as u16;
            }
            0xFF1C => {
                self.ch3.volume_shift = (val >> 5) & 0x03;
            }
            0xFF1D => {
                self.ch3.freq_raw = (self.ch3.freq_raw & 0x700) | val as u16;
            }
            0xFF1E => {
                self.ch3.freq_raw = (self.ch3.freq_raw & 0xFF) | ((val as u16 & 0x07) << 8);
                self.ch3.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch3.trigger();
                }
            }

            // CH4 — Noise
            0xFF20 => {
                self.ch4.length_counter = 64 - (val & 0x3F) as u16;
            }
            0xFF21 => {
                self.ch4.env_initial = val >> 4;
                self.ch4.env_direction = if val & 0x08 != 0 { 1 } else { -1 };
                self.ch4.env_period = val & 0x07;
                if !self.ch4.dac_enabled() {
                    self.ch4.enabled = false;
                }
            }
            0xFF22 => {
                self.ch4.clock_shift = val >> 4;
                self.ch4.width_mode = val & 0x08 != 0;
                self.ch4.divisor_code = val & 0x07;
            }
            0xFF23 => {
                self.ch4.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch4.trigger();
                }
            }

            // Master controls
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,

            // Wave RAM
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize] = val,

            _ => {}
        }
    }
}
