# APU (Audio Processing Unit)

**File:** `src/apu.rs` (880 lines)

The APU generates sample-accurate audio at 48 kHz with all four Game Boy channels. Each T-cycle ticks the channel frequency timers, and a sample is emitted every ~87 cycles.

## Channel Overview

| Channel | Type | Frequency Control | Volume | Extra |
|---------|------|-------------------|--------|-------|
| CH1 | Square wave | 11-bit register | 4-bit envelope | Frequency sweep |
| CH2 | Square wave | 11-bit register | 4-bit envelope | — |
| CH3 | Programmable wave | 11-bit register | 2-bit shift | 32 4-bit samples in wave RAM |
| CH4 | Noise | Divisor + shift | 4-bit envelope | 15-bit LFSR |

## Square Channels (CH1, CH2)

### Duty Cycle

4 duty patterns, selected by NRx1 bits 6–7:

```
0: 12.5%  [0,0,0,0,0,0,0,1]  ─────── ▌
1: 25%    [1,0,0,0,0,0,0,1]  ▌────── ▌
2: 50%    [1,0,0,0,0,1,1,1]  ▌──── ▌▌▌
3: 75%    [0,1,1,1,1,1,1,0]  ─▌▌▌▌▌▌─  (inverted 25%)
```

### Frequency Timer

The timer counts down every T-cycle:
```
period = (2048 - freq_raw) * 4
```
When it reaches 0, it reloads and advances `duty_position` (0–7 cycling).

### Output

```
output = DUTY_TABLE[duty][position] * volume
DAC output = output / 7.5 - 1.0    (maps 0-15 to -1.0..+1.0)
```

### Frequency Sweep (CH1 only)

Clocked at steps 2 and 6 of the frame sequencer (128 Hz):
```
new_freq = shadow_freq ± (shadow_freq >> shift)
```
If new_freq > 2047, the channel is disabled. After writing the new frequency, a **second** overflow check is performed.

## Wave Channel (CH3)

Plays arbitrary waveforms from 16 bytes of wave RAM (0xFF30–0xFF3F), encoding 32 4-bit samples.

### Frequency Timer
```
period = (2048 - freq_raw) * 2
```

### Output

```
byte = wave_ram[position / 2]
sample = high nibble (even position) or low nibble (odd position)
shifted = sample >> volume_shift    (0=mute, 1=100%, 2=50%, 3=25%)
DAC output = shifted / 7.5 - 1.0
```

## Noise Channel (CH4)

Generates pseudo-random noise using a linear feedback shift register.

### LFSR Clocking

```
period = DIVISORS[divisor_code] << clock_shift
DIVISORS = [8, 16, 32, 48, 64, 80, 96, 112]
```

Each time the timer reaches 0:
```
xor_bit = (lfsr & 1) ^ ((lfsr >> 1) & 1)
lfsr >>= 1
lfsr |= xor_bit << 14         // bit 14
if width_mode:
    lfsr |= xor_bit << 6      // also bit 6 (7-bit mode)
```

**Width mode** (NR43 bit 3): produces more metallic/tonal noise by shortening the LFSR from 15-bit to effectively 7-bit.

## Frame Sequencer

Clocked at 512 Hz (every 8,192 T-cycles), 8 steps:

| Step | Clocks |
|------|--------|
| 0 | Length |
| 1 | — |
| 2 | Length + Sweep |
| 3 | — |
| 4 | Length |
| 5 | — |
| 6 | Length + Sweep |
| 7 | Envelope |

### Length Counter

Each channel has a length counter (6-bit for CH1/2/4, 8-bit for CH3). When enabled and the counter reaches 0, the channel is disabled.

### Volume Envelope (CH1, CH2, CH4)

Clocked at step 7 (64 Hz). Adjusts volume by ±1 every N envelope periods until it hits 0 or 15.

## Mixing

```
left/right = sum of enabled channels (filtered by NR51 panning bits)
left *= ((NR50 >> 4) & 7 + 1) / 32.0     // master volume
right *= (NR50 & 7 + 1) / 32.0
```

Maximum output: 4 channels × 1.0 × 8.0 / 32.0 = 1.0 (no clipping).

## High-Pass Filter

A one-pole DC-blocking filter emulates the Game Boy's coupling capacitor:

```
y[n] = x[n] - x[n-1] + 0.999 * y[n-1]
```

Cutoff ≈ 7.6 Hz at 48 kHz — below audible range but removes DC offset that causes pops on channel enable/disable.

## DAC Enable

- **CH1/CH2/CH4**: DAC is on when NRx2 bits 3–7 are not all zero (volume > 0 OR envelope direction is up).
- **CH3**: DAC is on when NR30 bit 7 is set.

When the DAC is off, the channel is forced off regardless of trigger.

## Per-Channel Mute

The `ch_mute: [bool; 4]` array allows the frontend to silence individual channels without affecting the emulation state. Muted channels output 0.0 in `generate_sample()`.

## Sample Buffer

Samples are pushed to a `Vec<f32>` (interleaved L/R). The JS frontend reads via `audio_buffer_ptr()`/`audio_buffer_len()` and consumes via `audio_buffer_consume(count)`. The buffer is capped at 4,096 sample pairs to bound memory usage.

## Register Map

| Address | Register | Channel |
|---------|----------|---------|
| 0xFF10 | NR10 (sweep) | CH1 |
| 0xFF11 | NR11 (duty + length) | CH1 |
| 0xFF12 | NR12 (envelope) | CH1 |
| 0xFF13 | NR13 (freq low) | CH1 |
| 0xFF14 | NR14 (freq high + trigger) | CH1 |
| 0xFF16–0xFF19 | NR21–NR24 | CH2 |
| 0xFF1A–0xFF1E | NR30–NR34 | CH3 |
| 0xFF20–0xFF23 | NR41–NR44 | CH4 |
| 0xFF24 | NR50 (master volume) | — |
| 0xFF25 | NR51 (panning) | — |
| 0xFF26 | NR52 (power + status) | — |
| 0xFF30–0xFF3F | Wave RAM | CH3 |
