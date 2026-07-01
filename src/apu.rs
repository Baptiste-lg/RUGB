/// APU — Audio Processing Unit with 4 channels.
///
/// Channel 1: Square wave with frequency sweep
/// Channel 2: Square wave (no sweep)
/// Channel 3: Programmable wave
/// Channel 4: Noise (LFSR-based)
///
/// The frame sequencer runs at 512 Hz and clocks:
///   Steps 0,2,4,6 — length counters
///   Steps 2,6     — frequency sweep (CH1 only)
///   Step 7        — volume envelope (CH1, CH2, CH4)

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
}

pub struct WaveChannel {
    pub enabled: bool,
    pub volume_shift: u8, // 0=mute, 1=100%, 2=50%, 3=25%
    pub freq_raw: u16,
    length_counter: u16,
    length_enabled: bool,
    dac_enabled: bool,
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
}

impl SquareChannel {
    fn new(has_sweep: bool) -> Self {
        SquareChannel {
            enabled: false, duty: 0, volume: 0, freq_raw: 0,
            length_counter: 0, length_enabled: false,
            env_initial: 0, env_direction: 0, env_period: 0, env_timer: 0,
            sweep_period: 0, sweep_direction: 0, sweep_shift: 0,
            sweep_timer: 0, sweep_enabled: false, sweep_shadow: 0,
            has_sweep,
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.volume = self.env_initial;
        self.env_timer = self.env_period;
        if self.has_sweep {
            self.sweep_shadow = self.freq_raw;
            self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
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
        self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
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
        let new_vol = self.volume as i8 + self.env_direction as i8;
        if new_vol >= 0 && new_vol <= 15 {
            self.volume = new_vol as u8;
        }
    }

    /// Frequency in Hz for use by Web Audio oscillators
    pub fn frequency_hz(&self) -> f64 {
        if self.freq_raw >= 2048 { return 0.0; }
        131072.0 / (2048.0 - self.freq_raw as f64)
    }
}

impl WaveChannel {
    fn new() -> Self {
        WaveChannel {
            enabled: false, volume_shift: 0, freq_raw: 0,
            length_counter: 0, length_enabled: false, dac_enabled: false,
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
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

    pub fn frequency_hz(&self) -> f64 {
        if self.freq_raw >= 2048 { return 0.0; }
        65536.0 / (2048.0 - self.freq_raw as f64)
    }
}

impl NoiseChannel {
    fn new() -> Self {
        NoiseChannel {
            enabled: false, volume: 0,
            length_counter: 0, length_enabled: false,
            env_initial: 0, env_direction: 0, env_period: 0, env_timer: 0,
            clock_shift: 0, divisor_code: 0, width_mode: false,
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.volume = self.env_initial;
        self.env_timer = self.env_period;
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
        if self.env_period == 0 { return; }
        self.env_timer = self.env_timer.saturating_sub(1);
        if self.env_timer > 0 { return; }
        self.env_timer = self.env_period;
        let new_vol = self.volume as i8 + self.env_direction as i8;
        if new_vol >= 0 && new_vol <= 15 {
            self.volume = new_vol as u8;
        }
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
        }
    }

    /// Advance frame sequencer. Called from the main emulation loop.
    pub fn tick(&mut self, cycles: u32) {
        if !self.enabled { return; }

        self.frame_cycles += cycles;
        // Frame sequencer ticks every 8192 T-cycles (512 Hz)
        while self.frame_cycles >= 8192 {
            self.frame_cycles -= 8192;
            self.clock_frame_sequencer();
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

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // NR10 — CH1 sweep
            0xFF10 => {
                0x80 | (self.ch1.sweep_period << 4)
                    | if self.ch1.sweep_direction < 0 { 0x08 } else { 0 }
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
            0xFF1A => if self.ch3.dac_enabled { 0xFF } else { 0x7F },
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
                if self.enabled { val |= 0x80; }
                if self.ch1.enabled { val |= 0x01; }
                if self.ch2.enabled { val |= 0x02; }
                if self.ch3.enabled { val |= 0x04; }
                if self.ch4.enabled { val |= 0x08; }
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
            }
            0xFF13 => {
                self.ch1.freq_raw = (self.ch1.freq_raw & 0x700) | val as u16;
            }
            0xFF14 => {
                self.ch1.freq_raw = (self.ch1.freq_raw & 0xFF) | ((val as u16 & 0x07) << 8);
                self.ch1.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 { self.ch1.trigger(); }
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
            }
            0xFF18 => {
                self.ch2.freq_raw = (self.ch2.freq_raw & 0x700) | val as u16;
            }
            0xFF19 => {
                self.ch2.freq_raw = (self.ch2.freq_raw & 0xFF) | ((val as u16 & 0x07) << 8);
                self.ch2.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 { self.ch2.trigger(); }
            }

            // CH3 — Wave
            0xFF1A => {
                self.ch3.dac_enabled = val & 0x80 != 0;
                if !self.ch3.dac_enabled { self.ch3.enabled = false; }
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
                if val & 0x80 != 0 { self.ch3.trigger(); }
            }

            // CH4 — Noise
            0xFF20 => {
                self.ch4.length_counter = 64 - (val & 0x3F) as u16;
            }
            0xFF21 => {
                self.ch4.env_initial = val >> 4;
                self.ch4.env_direction = if val & 0x08 != 0 { 1 } else { -1 };
                self.ch4.env_period = val & 0x07;
            }
            0xFF22 => {
                self.ch4.clock_shift = val >> 4;
                self.ch4.width_mode = val & 0x08 != 0;
                self.ch4.divisor_code = val & 0x07;
            }
            0xFF23 => {
                self.ch4.length_enabled = val & 0x40 != 0;
                if val & 0x80 != 0 { self.ch4.trigger(); }
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
