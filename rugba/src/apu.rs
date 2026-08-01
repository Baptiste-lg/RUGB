const CPU_CLOCK: u32 = 16_777_216;
const SAMPLE_RATE: u32 = 32_768;
const CYCLES_PER_SAMPLE: u32 = CPU_CLOCK / SAMPLE_RATE; // 512

const RING_BUFFER_CAPACITY: usize = 8192;
const RING_BUFFER_MASK: usize = RING_BUFFER_CAPACITY - 1;

// Duty cycle waveform table: each row is a pattern of 8 steps (high/low).
// Index 0 = 12.5%, 1 = 25%, 2 = 50%, 3 = 75%.
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

// ---------------------------------------------------------------------------
// PSG channel state structs
// ---------------------------------------------------------------------------

struct PsgSquare {
    enabled: bool,
    frequency: u16, // 11-bit frequency timer value
    volume: u8,     // 4-bit volume (0-15)
    duty: u8,       // duty cycle pattern index (0-3)
    timer: u32,     // countdown timer (CPU cycles)
    duty_pos: u8,   // position in 8-step duty cycle
    length_enabled: bool,
    length_counter: u16,
}

impl PsgSquare {
    fn new() -> Self {
        PsgSquare {
            enabled: false,
            frequency: 0,
            volume: 0,
            duty: 0,
            timer: 0,
            duty_pos: 0,
            length_enabled: false,
            length_counter: 0,
        }
    }

    /// Returns the period in CPU cycles for this channel's frequency.
    /// GBA square wave period = (2048 - frequency) * 16 cycles.
    fn period(&self) -> u32 {
        (2048u32.saturating_sub(self.frequency as u32)) * 16
    }

    /// Advance the channel by `cycles` CPU cycles.
    fn tick(&mut self, cycles: u32) {
        if !self.enabled || self.volume == 0 {
            return;
        }
        let period = self.period();
        if period == 0 {
            return;
        }
        // Add cycles to timer; advance duty position for each full period elapsed.
        if self.timer < cycles {
            let remaining = cycles - self.timer;
            let steps = remaining / period + 1;
            self.duty_pos = ((self.duty_pos as u32 + steps) % 8) as u8;
            self.timer = period - (remaining % period);
        } else {
            self.timer -= cycles;
        }
    }

    /// Current output sample: +volume or 0 based on duty cycle.
    fn sample(&self) -> i16 {
        if !self.enabled {
            return 0;
        }
        if DUTY_TABLE[self.duty as usize & 3][self.duty_pos as usize] != 0 {
            self.volume as i16
        } else {
            0
        }
    }
}

struct PsgWave {
    enabled: bool,
    frequency: u16,
    volume_shift: u8, // 0=mute, 1=100%, 2=50%, 3=25%
    wave_ram: [u8; 16],
    timer: u32,
    sample_pos: u8, // 0-31 (32 4-bit samples)
}

impl PsgWave {
    fn new() -> Self {
        PsgWave {
            enabled: false,
            frequency: 0,
            volume_shift: 0,
            wave_ram: [0; 16],
            timer: 0,
            sample_pos: 0,
        }
    }

    /// Period in CPU cycles: (2048 - frequency) * 8
    fn period(&self) -> u32 {
        (2048u32.saturating_sub(self.frequency as u32)) * 8
    }

    fn tick(&mut self, cycles: u32) {
        if !self.enabled || self.volume_shift == 0 {
            return;
        }
        let period = self.period();
        if period == 0 {
            return;
        }
        if self.timer < cycles {
            let remaining = cycles - self.timer;
            let steps = remaining / period + 1;
            self.sample_pos = ((self.sample_pos as u32 + steps) % 32) as u8;
            self.timer = period - (remaining % period);
        } else {
            self.timer -= cycles;
        }
    }

    fn sample(&self) -> i16 {
        if !self.enabled || self.volume_shift == 0 {
            return 0;
        }
        let byte = self.wave_ram[(self.sample_pos / 2) as usize];
        let nibble = if self.sample_pos & 1 == 0 {
            (byte >> 4) & 0xF
        } else {
            byte & 0xF
        };
        let shifted = match self.volume_shift {
            1 => nibble,      // 100%
            2 => nibble >> 1, // 50%
            3 => nibble >> 2, // 25%
            _ => 0,           // mute
        };
        shifted as i16
    }
}

struct PsgNoise {
    enabled: bool,
    volume: u8,
    lfsr: u16,
    width_mode: bool, // false=15-bit, true=7-bit
    timer: u32,
    divisor_code: u8,
    clock_shift: u8,
}

impl PsgNoise {
    fn new() -> Self {
        PsgNoise {
            enabled: false,
            volume: 0,
            lfsr: 0x7FFF,
            width_mode: false,
            timer: 0,
            divisor_code: 0,
            clock_shift: 0,
        }
    }

    /// LFSR clock period in CPU cycles.
    fn period(&self) -> u32 {
        // Base divisors: 0→8, 1→16, 2→32, ... 7→128; then shift left by clock_shift.
        let divisor: u32 = if self.divisor_code == 0 {
            8
        } else {
            (self.divisor_code as u32) * 16
        };
        divisor << self.clock_shift
    }

    fn tick(&mut self, cycles: u32) {
        if !self.enabled || self.volume == 0 {
            return;
        }
        let period = self.period();
        if period == 0 {
            return;
        }
        let mut remaining = cycles;
        loop {
            if self.timer >= remaining {
                self.timer -= remaining;
                break;
            }
            remaining -= self.timer;
            // Clock the LFSR
            let xor_bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.width_mode {
                // Also feedback into bit 6 for 7-bit mode
                self.lfsr &= !(1 << 6);
                self.lfsr |= xor_bit << 6;
            }
            self.timer = period;
        }
    }

    fn sample(&self) -> i16 {
        if !self.enabled {
            return 0;
        }
        // Output is high when LFSR bit 0 is 0
        if self.lfsr & 1 == 0 {
            self.volume as i16
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// FIFO — 32-byte circular buffer for DMA Sound A / B
// ---------------------------------------------------------------------------

pub struct Fifo {
    buffer: [i8; 32],
    read_pos: usize,
    write_pos: usize,
    count: usize,
    current_sample: i8,
}

impl Fifo {
    pub fn new() -> Self {
        Fifo {
            buffer: [0; 32],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            current_sample: 0,
        }
    }

    /// Push 4 bytes (one 32-bit write) into the FIFO.
    pub fn write32(&mut self, data: u32) {
        for i in 0..4 {
            if self.count < 32 {
                self.buffer[self.write_pos] = (data >> (i * 8)) as i8;
                self.write_pos = (self.write_pos + 1) & 31;
                self.count += 1;
            }
        }
    }

    /// Pop one sample from the FIFO and latch it as `current_sample`.
    /// Returns true if FIFO needs refill (<= 16 bytes remaining).
    pub fn pop(&mut self) -> bool {
        if self.count > 0 {
            self.current_sample = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) & 31;
            self.count -= 1;
        }
        self.count <= 16
    }

    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
        self.current_sample = 0;
    }
}

// ---------------------------------------------------------------------------
// APU
// ---------------------------------------------------------------------------

pub struct Apu {
    // PSG master enable (SOUNDCNT_X bit 7)
    pub psg_enabled: bool,
    // PSG channel volumes (derived from SOUNDCNT_L, 3-bit each)
    pub ch1_vol: u8,
    pub ch2_vol: u8,
    pub ch3_vol: u8,
    pub ch4_vol: u8,

    // PSG channel state
    psg_ch1: PsgSquare,
    psg_ch2: PsgSquare,
    psg_ch3: PsgWave,
    psg_ch4: PsgNoise,

    // PSG panning from SOUNDCNT_L (NR51 equivalent)
    // Bits 0-3: right enable for CH1-4; bits 4-7: left enable for CH1-4
    psg_panning: u8,

    // DMA Sound FIFOs
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,

    // Sound control registers
    pub soundcnt_l: u16, // NR50/NR51 equivalent (PSG volume/panning)
    pub soundcnt_h: u16, // DMA sound control (volume, timer select, enable)
    pub soundcnt_x: u16, // Master enable
    pub soundbias: u16,  // SOUNDBIAS (default 0x0200)

    // Audio output ring buffer (interleaved L/R f32)
    ring_buffer: Box<[f32; RING_BUFFER_CAPACITY]>,
    write_pos: usize,
    read_pos: usize,

    // Sample generation timing
    sample_clock: u32,
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            psg_enabled: false,
            ch1_vol: 0,
            ch2_vol: 0,
            ch3_vol: 0,
            ch4_vol: 0,
            psg_ch1: PsgSquare::new(),
            psg_ch2: PsgSquare::new(),
            psg_ch3: PsgWave::new(),
            psg_ch4: PsgNoise::new(),
            psg_panning: 0,
            fifo_a: Fifo::new(),
            fifo_b: Fifo::new(),
            soundcnt_l: 0,
            soundcnt_h: 0,
            soundcnt_x: 0,
            soundbias: 0x0200,
            ring_buffer: Box::new([0.0; RING_BUFFER_CAPACITY]),
            write_pos: 0,
            read_pos: 0,
            sample_clock: 0,
        }
    }

    // -- Register helpers ---------------------------------------------------

    /// DMA Sound A volume: false = 50%, true = 100%
    fn fifo_a_full_vol(&self) -> bool {
        self.soundcnt_h & (1 << 2) != 0
    }

    /// DMA Sound B volume: false = 50%, true = 100%
    fn fifo_b_full_vol(&self) -> bool {
        self.soundcnt_h & (1 << 3) != 0
    }

    fn fifo_a_enable_right(&self) -> bool {
        self.soundcnt_h & (1 << 8) != 0
    }
    fn fifo_a_enable_left(&self) -> bool {
        self.soundcnt_h & (1 << 9) != 0
    }
    fn fifo_a_timer(&self) -> usize {
        ((self.soundcnt_h >> 10) & 1) as usize
    }

    fn fifo_b_enable_right(&self) -> bool {
        self.soundcnt_h & (1 << 12) != 0
    }
    fn fifo_b_enable_left(&self) -> bool {
        self.soundcnt_h & (1 << 13) != 0
    }
    fn fifo_b_timer(&self) -> usize {
        ((self.soundcnt_h >> 14) & 1) as usize
    }

    fn master_enable(&self) -> bool {
        self.soundcnt_x & (1 << 7) != 0
    }

    // PSG volume accessors from SOUNDCNT_L
    // Bits 0-2: right master volume, bits 4-6: left master volume
    fn psg_vol_right(&self) -> u8 {
        (self.soundcnt_l & 0x07) as u8
    }

    fn psg_vol_left(&self) -> u8 {
        ((self.soundcnt_l >> 4) & 0x07) as u8
    }

    // -- PSG register write -------------------------------------------------

    /// Handle a write to a PSG I/O register. `offset` is relative to 0x04000000.
    pub fn write_psg_register(&mut self, offset: u16, val: u16) {
        match offset {
            // CH1 — Square with sweep
            // SOUND1CNT_L (sweep): bits 0-2=shift, bit3=direction, bits 4-6=time
            0x060 => { /* sweep — not fully modelled; ignore for now */ }
            // SOUND1CNT_H: bits 0-5=length, bits 6-7=duty, bits 8-10=env period,
            //              bit 11=env direction, bits 12-15=initial volume
            0x062 => {
                self.psg_ch1.duty = ((val >> 6) & 0x3) as u8;
                self.psg_ch1.volume = ((val >> 12) & 0xF) as u8;
                // length: 64 - (val & 0x3F)
                self.psg_ch1.length_counter = 64 - (val & 0x3F);
            }
            // SOUND1CNT_X: bits 0-10=frequency, bit 14=length enable, bit 15=trigger
            0x064 => {
                self.psg_ch1.frequency = val & 0x7FF ;
                self.psg_ch1.length_enabled = val & (1 << 14) != 0;
                if val & (1 << 15) != 0 {
                    // Trigger: restart channel
                    self.psg_ch1.enabled = true;
                    self.psg_ch1.timer = self.psg_ch1.period();
                    self.psg_ch1.duty_pos = 0;
                }
            }

            // CH2 — Square (no sweep)
            // SOUND2CNT_L: bits 0-5=length, bits 6-7=duty, bits 8-10=env period,
            //              bit 11=env direction, bits 12-15=initial volume
            0x068 => {
                self.psg_ch2.duty = ((val >> 6) & 0x3) as u8;
                self.psg_ch2.volume = ((val >> 12) & 0xF) as u8;
                self.psg_ch2.length_counter = 64 - (val & 0x3F);
            }
            // SOUND2CNT_H: bits 0-10=frequency, bit 14=length enable, bit 15=trigger
            0x06C => {
                self.psg_ch2.frequency = val & 0x7FF ;
                self.psg_ch2.length_enabled = val & (1 << 14) != 0;
                if val & (1 << 15) != 0 {
                    self.psg_ch2.enabled = true;
                    self.psg_ch2.timer = self.psg_ch2.period();
                    self.psg_ch2.duty_pos = 0;
                }
            }

            // CH3 — Wave
            // SOUND3CNT_L: bit 7=DAC enable
            0x070 => {
                if val & (1 << 7) == 0 {
                    self.psg_ch3.enabled = false;
                }
            }
            // SOUND3CNT_H: bits 0-7=length (256-n), bits 13-14=volume
            0x072 => {
                self.psg_ch3.volume_shift = match (val >> 13) & 0x3 {
                    0 => 0, // mute
                    1 => 1, // 100%
                    2 => 2, // 50%
                    3 => 3, // 25%
                    _ => 0,
                };
            }
            // SOUND3CNT_X: bits 0-10=frequency, bit 14=length enable, bit 15=trigger
            0x074 => {
                self.psg_ch3.frequency = val & 0x7FF ;
                if val & (1 << 15) != 0 {
                    self.psg_ch3.enabled = true;
                    self.psg_ch3.timer = self.psg_ch3.period();
                    self.psg_ch3.sample_pos = 0;
                }
            }

            // CH4 — Noise
            // SOUND4CNT_L: bits 0-5=length, bits 8-10=env period,
            //              bit 11=env direction, bits 12-15=initial volume
            0x078 => {
                self.psg_ch4.volume = ((val >> 12) & 0xF) as u8;
            }
            // SOUND4CNT_H: bits 0-2=divisor code, bit 3=width mode, bits 4-7=clock shift,
            //              bit 14=length enable, bit 15=trigger
            0x07C => {
                self.psg_ch4.divisor_code = (val & 0x7) as u8;
                self.psg_ch4.width_mode = val & (1 << 3) != 0;
                self.psg_ch4.clock_shift = ((val >> 4) & 0xF) as u8;
                if val & (1 << 15) != 0 {
                    self.psg_ch4.enabled = true;
                    self.psg_ch4.lfsr = 0x7FFF;
                    self.psg_ch4.timer = self.psg_ch4.period();
                }
            }

            // SOUNDCNT_L: PSG master volume + panning
            0x080 => {
                self.soundcnt_l = val;
                // Bits 8-11: right enable for CH1-4; bits 12-15: left enable for CH1-4
                self.psg_panning = ((val >> 8) & 0xFF) as u8;
                self.ch1_vol = self.psg_vol_right().max(self.psg_vol_left());
                self.ch2_vol = self.ch1_vol;
                self.ch3_vol = self.ch1_vol;
                self.ch4_vol = self.ch1_vol;
            }

            // SOUNDCNT_X: master enable (bit 7); lower bits are read-only channel status
            0x084 => {
                let master = val & (1 << 7);
                self.soundcnt_x = (self.soundcnt_x & 0x0F) | master;
                self.psg_enabled = master != 0;
            }

            // Wave RAM: 0x090-0x09F (16 bytes, written as 8 halfwords)
            0x090..=0x09E => {
                let idx = ((offset - 0x090) / 2) as usize;
                if idx + 1 < 16 {
                    self.psg_ch3.wave_ram[idx * 2] = val as u8;
                    self.psg_ch3.wave_ram[idx * 2 + 1] = (val >> 8) as u8;
                }
            }

            _ => {}
        }
    }

    // -- PSG tick -----------------------------------------------------------

    /// Advance PSG channel timers by `cycles` CPU cycles.
    pub fn tick_psg(&mut self, cycles: u32) {
        self.psg_ch1.tick(cycles);
        self.psg_ch2.tick(cycles);
        self.psg_ch3.tick(cycles);
        self.psg_ch4.tick(cycles);
    }

    // -- Public API ---------------------------------------------------------

    /// Advance the APU by `cycles` CPU cycles, generating samples as needed.
    pub fn tick(&mut self, cycles: u32) {
        if !self.master_enable() {
            return;
        }

        self.tick_psg(cycles);

        self.sample_clock += cycles;

        while self.sample_clock >= CYCLES_PER_SAMPLE {
            self.sample_clock -= CYCLES_PER_SAMPLE;
            self.generate_sample();
        }
    }

    /// Called when timer 0 or timer 1 overflows. Pops the associated FIFO(s).
    /// Returns a bitmask indicating which FIFOs need DMA refill:
    ///   bit 0 = FIFO A, bit 1 = FIFO B.
    pub fn timer_overflow(&mut self, timer_id: usize) -> u8 {
        let mut needs_dma: u8 = 0;

        if self.fifo_a_timer() == timer_id && self.fifo_a.pop() {
            needs_dma |= 1;
        }
        if self.fifo_b_timer() == timer_id && self.fifo_b.pop() {
            needs_dma |= 2;
        }

        needs_dma
    }

    /// Write 4 bytes to FIFO A (address 0x0400_00A0).
    pub fn write_fifo_a(&mut self, data: u32) {
        self.fifo_a.write32(data);
    }

    /// Write 4 bytes to FIFO B (address 0x0400_00A4).
    pub fn write_fifo_b(&mut self, data: u32) {
        self.fifo_b.write32(data);
    }

    /// Handle writes to SOUNDCNT_H; resets FIFOs when reset bits are set.
    pub fn write_soundcnt_h(&mut self, value: u16) {
        if value & (1 << 11) != 0 {
            self.fifo_a.reset();
        }
        if value & (1 << 15) != 0 {
            self.fifo_b.reset();
        }
        // Clear reset bits before storing
        self.soundcnt_h = value & !(1 << 11 | 1 << 15);
    }

    // -- Ring buffer accessors (mirror the rugb pattern) --------------------

    pub fn ring_buffer_ptr(&self) -> *const f32 {
        self.ring_buffer.as_ptr()
    }

    pub fn ring_buffer_available(&self) -> usize {
        (self.write_pos + RING_BUFFER_CAPACITY - self.read_pos) & RING_BUFFER_MASK
    }

    pub fn ring_read_pos(&self) -> usize {
        self.read_pos
    }

    pub fn ring_capacity(&self) -> usize {
        RING_BUFFER_CAPACITY
    }

    pub fn ring_clear(&mut self) {
        self.read_pos = self.write_pos;
    }

    pub fn ring_consume(&mut self, count: usize) {
        let avail = self.ring_buffer_available();
        let n = count.min(avail);
        self.read_pos = (self.read_pos + n) & RING_BUFFER_MASK;
    }

    // -- Internal -----------------------------------------------------------

    #[cfg(test)]
    pub fn ring_write_pos(&self) -> usize {
        self.write_pos
    }

    fn generate_sample(&mut self) {
        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;

        // FIFO A
        let a = self.fifo_a.current_sample as f32 / 128.0;
        let a = if self.fifo_a_full_vol() { a } else { a * 0.5 };
        if self.fifo_a_enable_left() {
            left += a;
        }
        if self.fifo_a_enable_right() {
            right += a;
        }

        // FIFO B
        let b = self.fifo_b.current_sample as f32 / 128.0;
        let b = if self.fifo_b_full_vol() { b } else { b * 0.5 };
        if self.fifo_b_enable_left() {
            left += b;
        }
        if self.fifo_b_enable_right() {
            right += b;
        }

        // PSG mixing
        // Each PSG channel outputs 0-15 (for square/noise) or 0-15 (for wave after shift).
        // Normalise to [-1.0, 1.0] range: scale by 1/15 then by master volume (0-7)/7.
        // SOUNDCNT_L bits 0-2=right master vol, bits 4-6=left master vol.
        // SOUNDCNT_L bits 8-11=right ch enable, bits 12-15=left ch enable.
        // The PSG channels occupy SOUNDCNT_H bits 0-1: PSG volume (00=25%, 01=50%, 10=100%).
        // soundcnt_h bits 0-1 scale PSG relative to full scale.
        let psg_scale = match self.soundcnt_h & 0x3 {
            0 => 0.25_f32,
            1 => 0.5,
            _ => 1.0,
        };

        let vol_right = self.psg_vol_right() as f32 / 7.0;
        let vol_left = self.psg_vol_left() as f32 / 7.0;

        let ch1 = self.psg_ch1.sample() as f32 / 15.0;
        let ch2 = self.psg_ch2.sample() as f32 / 15.0;
        let ch3 = self.psg_ch3.sample() as f32 / 15.0;
        let ch4 = self.psg_ch4.sample() as f32 / 15.0;

        // psg_panning: bits 0-3=right enable (CH1-4), bits 4-7=left enable (CH1-4)
        let psg_right = {
            let mut s = 0.0_f32;
            if self.psg_panning & (1 << 0) != 0 {
                s += ch1;
            }
            if self.psg_panning & (1 << 1) != 0 {
                s += ch2;
            }
            if self.psg_panning & (1 << 2) != 0 {
                s += ch3;
            }
            if self.psg_panning & (1 << 3) != 0 {
                s += ch4;
            }
            s * psg_scale * vol_right * 0.25 // divide by 4 channels max
        };

        let psg_left = {
            let mut s = 0.0_f32;
            if self.psg_panning & (1 << 4) != 0 {
                s += ch1;
            }
            if self.psg_panning & (1 << 5) != 0 {
                s += ch2;
            }
            if self.psg_panning & (1 << 6) != 0 {
                s += ch3;
            }
            if self.psg_panning & (1 << 7) != 0 {
                s += ch4;
            }
            s * psg_scale * vol_left * 0.25
        };

        left += psg_left;
        right += psg_right;

        // Clamp to [-1.0, 1.0]
        left = left.clamp(-1.0, 1.0);
        right = right.clamp(-1.0, 1.0);

        // Push interleaved L/R into ring buffer (drop if full)
        let avail = self.ring_buffer_available();
        if avail + 2 < RING_BUFFER_CAPACITY {
            self.ring_buffer[self.write_pos] = left;
            self.write_pos = (self.write_pos + 1) & RING_BUFFER_MASK;
            self.ring_buffer[self.write_pos] = right;
            self.write_pos = (self.write_pos + 1) & RING_BUFFER_MASK;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Fifo tests
    // =========================================================================

    // ---- fifo_new_empty ----

    #[test]
    fn fifo_new_empty() {
        let fifo = Fifo::new();
        assert_eq!(fifo.count, 0);
    }

    // ---- fifo_write32_fills_4 ----

    #[test]
    fn fifo_write32_fills_4() {
        let mut fifo = Fifo::new();
        // 0x04030201 in little-endian bytes: b0=0x01, b1=0x02, b2=0x03, b3=0x04
        fifo.write32(0x0403_0201);
        assert_eq!(fifo.count, 4);

        // Pop 4 times and check bytes in LE order
        fifo.pop(); // pops b0=0x01
        assert_eq!(fifo.current_sample, 0x01_i8);
        fifo.pop(); // pops b1=0x02
        assert_eq!(fifo.current_sample, 0x02_i8);
        fifo.pop(); // pops b2=0x03
        assert_eq!(fifo.current_sample, 0x03_i8);
        fifo.pop(); // pops b3=0x04
        assert_eq!(fifo.current_sample, 0x04_i8);
    }

    // ---- fifo_pop_needs_refill ----

    #[test]
    fn fifo_pop_needs_refill() {
        let mut fifo = Fifo::new();
        // Fill exactly 16 bytes (4 × write32)
        for _ in 0..4 {
            fifo.write32(0x0101_0101);
        }
        assert_eq!(fifo.count, 16);
        // Popping when count == 16 → after pop count becomes 15 ≤ 16 → needs refill
        let needs = fifo.pop();
        assert!(needs, "should need refill when count drops to <= 16");
    }

    // ---- fifo_reset_clears ----

    #[test]
    fn fifo_reset_clears() {
        let mut fifo = Fifo::new();
        fifo.write32(0xDEAD_BEEF);
        assert_eq!(fifo.count, 4);
        fifo.reset();
        assert_eq!(fifo.count, 0);
        assert_eq!(fifo.current_sample, 0);
    }

    // ---- fifo_full_drops ----

    #[test]
    fn fifo_full_drops() {
        let mut fifo = Fifo::new();
        // Push 9 times × 4 bytes = 36 bytes; FIFO capacity is 32
        for _ in 0..9 {
            fifo.write32(0x0101_0101);
        }
        // Only 32 bytes should be stored
        assert_eq!(fifo.count, 32);
    }

    // =========================================================================
    // Apu tests
    // =========================================================================

    // ---- apu_initial_state ----

    #[test]
    fn apu_initial_state() {
        let apu = Apu::new();
        assert_eq!(apu.soundbias, 0x0200);
    }

    // ---- apu_tick_disabled_no_samples ----

    #[test]
    fn apu_tick_disabled_no_samples() {
        let mut apu = Apu::new();
        // soundcnt_x bit 7 = master enable; default is 0 (disabled)
        apu.soundcnt_x = 0;
        apu.tick(512 * 10); // tick many cycles
        assert_eq!(apu.ring_buffer_available(), 0);
    }

    // ---- write_soundcnt_h_resets_fifo_a ----

    #[test]
    fn write_soundcnt_h_resets_fifo_a() {
        let mut apu = Apu::new();
        apu.fifo_a.write32(0xDEAD_BEEF);
        assert_eq!(apu.fifo_a.count, 4);
        // Bit 11 set → reset FIFO A
        apu.write_soundcnt_h(1 << 11);
        assert_eq!(apu.fifo_a.count, 0);
    }

    // ---- write_soundcnt_h_resets_fifo_b ----

    #[test]
    fn write_soundcnt_h_resets_fifo_b() {
        let mut apu = Apu::new();
        apu.fifo_b.write32(0xDEAD_BEEF);
        assert_eq!(apu.fifo_b.count, 4);
        // Bit 15 set → reset FIFO B
        apu.write_soundcnt_h(1 << 15);
        assert_eq!(apu.fifo_b.count, 0);
    }

    // ---- timer_overflow_pops_fifo ----

    #[test]
    fn timer_overflow_pops_fifo() {
        let mut apu = Apu::new();
        // fifo_a_timer() reads bit 10 of soundcnt_h; 0 → timer 0
        apu.soundcnt_h = 0; // timer 0 for FIFO A
        apu.fifo_a.write32(0x0102_0304);

        let count_before = apu.fifo_a.count;
        apu.timer_overflow(0);
        // One sample should have been popped
        assert_eq!(apu.fifo_a.count, count_before - 1);
    }

    // ---- timer_overflow_wrong_timer ----

    #[test]
    fn timer_overflow_wrong_timer() {
        let mut apu = Apu::new();
        // FIFO A expects timer 0 (bit 10 of soundcnt_h = 0)
        apu.soundcnt_h = 0;
        apu.fifo_a.write32(0x0102_0304);

        let count_before = apu.fifo_a.count;
        // Overflow on timer 1 → FIFO A should NOT pop
        apu.timer_overflow(1);
        assert_eq!(apu.fifo_a.count, count_before);
    }

    // ---- ring_buffer_consume ----

    #[test]
    fn ring_buffer_consume() {
        let mut apu = Apu::new();
        // Enable master and left output for FIFO A so samples are generated
        apu.soundcnt_x = 0x80; // master enable
        apu.soundcnt_h = 1 << 9; // FIFO A enable left
        apu.fifo_a.write32(0x4040_4040); // non-silent data

        // Tick enough cycles to generate at least 2 samples (1 L+R pair = 2 f32)
        apu.tick(512 * 2);

        let avail_before = apu.ring_buffer_available();
        assert!(
            avail_before >= 2,
            "should have generated at least one L+R pair"
        );

        apu.ring_consume(2);
        assert_eq!(apu.ring_buffer_available(), avail_before - 2);
    }

    // ---- psg_ch2_trigger_produces_samples ----

    #[test]
    fn psg_ch2_trigger_produces_samples() {
        let mut apu = Apu::new();
        apu.soundcnt_x = 0x80; // master enable

        // Set CH2: duty=2 (50%), volume=8, 500 Hz-ish frequency
        // SOUND2CNT_L: vol=8 (bits 12-15), duty=2 (bits 6-7)
        apu.write_psg_register(0x068, (8 << 12) | (2 << 6));
        // SOUND2CNT_H: frequency=1024, trigger (bit 15), panning all
        apu.write_psg_register(0x06C, (1 << 15) | 1024);
        // SOUNDCNT_L: max vol both sides, enable CH2 on both (bit 9 right, bit 13 left)
        apu.write_psg_register(0x080, 0xFF77); // enable all channels, max vol

        // Tick enough to generate some samples
        apu.tick(512 * 4);

        assert!(
            apu.ring_buffer_available() >= 2,
            "PSG CH2 trigger should result in samples being generated"
        );
    }

    // ---- psg_ch4_trigger_noise ----

    #[test]
    fn psg_ch4_trigger_noise() {
        let mut apu = Apu::new();
        apu.soundcnt_x = 0x80;

        // CH4 noise: volume=15, divisor=0, shift=3
        apu.write_psg_register(0x078, 15 << 12);
        apu.write_psg_register(0x07C, (1 << 15) | (3 << 4));
        // Enable CH4 right+left in panning (bits 3 and 7)
        apu.write_psg_register(0x080, 0x8877);

        apu.tick(512 * 4);

        assert!(
            apu.ring_buffer_available() >= 2,
            "PSG CH4 (noise) trigger should produce samples"
        );
    }

    // ---- psg_wave_ram_write ----

    #[test]
    fn psg_wave_ram_write() {
        let mut apu = Apu::new();
        // Write a halfword to wave RAM offset 0x090
        apu.write_psg_register(0x090, 0xABCD);
        assert_eq!(apu.psg_ch3.wave_ram[0], 0xCD);
        assert_eq!(apu.psg_ch3.wave_ram[1], 0xAB);
    }

    // ---- psg_disabled_when_master_off ----

    #[test]
    fn psg_disabled_when_master_off() {
        let mut apu = Apu::new();
        // Master enable is off (soundcnt_x default = 0)
        apu.write_psg_register(0x068, (8 << 12) | (2 << 6));
        apu.write_psg_register(0x06C, (1 << 15) | 1024);
        apu.write_psg_register(0x080, 0xFF77);
        // Do NOT set master enable
        apu.tick(512 * 4);
        assert_eq!(
            apu.ring_buffer_available(),
            0,
            "no samples when master is disabled"
        );
    }
}
