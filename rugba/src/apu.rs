const CPU_CLOCK: u32 = 16_777_216;
const SAMPLE_RATE: u32 = 32_768;
const CYCLES_PER_SAMPLE: u32 = CPU_CLOCK / SAMPLE_RATE; // 512

const RING_BUFFER_CAPACITY: usize = 8192;
const RING_BUFFER_MASK: usize = RING_BUFFER_CAPACITY - 1;

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
    // PSG channels (simplified -- register storage + basic output, stubbed silent)
    pub psg_enabled: bool,
    pub ch1_vol: u8,
    pub ch2_vol: u8,
    pub ch3_vol: u8,
    pub ch4_vol: u8,

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

    // -- Public API ---------------------------------------------------------

    /// Advance the APU by `cycles` CPU cycles, generating samples as needed.
    pub fn tick(&mut self, cycles: u32) {
        if !self.master_enable() {
            return;
        }

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

        if self.fifo_a_timer() == timer_id {
            if self.fifo_a.pop() {
                needs_dma |= 1;
            }
        }
        if self.fifo_b_timer() == timer_id {
            if self.fifo_b.pop() {
                needs_dma |= 2;
            }
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
