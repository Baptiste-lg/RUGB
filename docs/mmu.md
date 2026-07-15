# MMU (Memory Management Unit)

**File:** `src/mmu.rs` (178 lines)

The MMU routes all CPU memory reads and writes to the correct subsystem. It is the central hub connecting the CPU to every piece of hardware.

## Address Routing

### Read Path

```rust
match addr {
    0x0000..=0x7FFF => cartridge.read(addr),        // ROM
    0x8000..=0x9FFF => ppu.read_vram(addr),          // VRAM
    0xA000..=0xBFFF => cartridge.read(addr),         // External RAM
    0xC000..=0xDFFF => wram[(addr - 0xC000)],        // Work RAM
    0xE000..=0xFDFF => wram[(addr - 0xE000)],        // Echo RAM
    0xFE00..=0xFE9F => ppu.read_oam(addr),           // OAM
    0xFEA0..=0xFEFF => 0xFF,                         // Unusable
    0xFF00          => joypad.read(),                 // Joypad
    0xFF01          => serial_data,                   // Serial
    0xFF04..=0xFF07 => timer.read(addr),              // Timer
    0xFF0F          => interrupt_flag,                // IF
    0xFF10..=0xFF3F => apu.read(addr),                // APU
    0xFF40..=0xFF4B => ppu.read_register(addr),       // PPU registers
    0xFF80..=0xFFFE => hram[(addr - 0xFF80)],        // High RAM
    0xFFFF          => ie,                            // IE
    _               => 0xFF,                         // Unmapped
}
```

### Write Path

The write path mirrors the read path, with two special cases:

1. **ROM writes (0x0000–0x7FFF)**: Forwarded to the cartridge mapper for bank switching.
2. **OAM DMA (write to 0xFF46)**: Triggers a 160-byte copy from the source address to OAM.

### OAM DMA

Writing a value `V` to 0xFF46 copies 160 bytes from `V << 8` to OAM:

```rust
let source = (val as u16) << 8;
for i in 0..0xA0 {
    let byte = self.read(source + i);
    self.ppu.write_oam(0xFE00 + i, byte);
}
```

On real hardware, DMA takes 160 M-cycles and the CPU can only access HRAM during transfer. This implementation is instantaneous (good enough for most games).

## Echo RAM

0xE000–0xFDFF mirrors 0xC000–0xDDFF. Note: the echo is only 7,680 bytes, not the full 8 KB of WRAM. The last 512 bytes (0xDE00–0xDFFF) are not echoed.

## I/O Register Gaps

Addresses 0xFF03, 0xFF08–0xFF0E, and 0xFF4C–0xFF7F are unmapped:
- Reads return 0xFF
- Writes are ignored

## Serial Port

0xFF01 (serial data) and 0xFF02 (serial control) are minimally implemented. Writing 0x81 to 0xFF02 prints the serial data byte to stderr (used by Blargg test ROMs for output).

## Interrupt Registers

- **IF (0xFF0F)**: Interrupt Flag — bits set by hardware when an interrupt condition occurs. Bits 0–4 correspond to VBlank, STAT, Timer, Serial, Joypad.
- **IE (0xFFFF)**: Interrupt Enable — game sets which interrupts it wants to handle.

Both are read/write. The interrupt controller checks `IF & IE` to find pending, enabled interrupts.
