# Cartridge Mappers

**Files:** `src/cartridge/mod.rs`, `src/cartridge/no_mbc.rs`, `src/cartridge/mbc1.rs`, `src/cartridge/mbc3.rs`

Game Boy cartridges contain the ROM and optionally external RAM with bank switching logic. The mapper type is determined by the cartridge header byte at 0x0147.

## ROM Header

| Offset | Field | Used by RUGB |
|--------|-------|-------------|
| 0x0134–0x0143 | Title (ASCII, null-padded) | Parsed for display and save keys |
| 0x0147 | Cartridge type (mapper + features) | Selects mapper implementation |
| 0x0148 | ROM size | Not explicitly used (inferred from data length) |
| 0x0149 | RAM size | Determines external RAM allocation |

### RAM Size Table

| Code | Size |
|------|------|
| 0x00 | None |
| 0x01 | 2 KB |
| 0x02 | 8 KB |
| 0x03 | 32 KB |
| 0x04 | 128 KB |
| 0x05 | 64 KB |

## Supported Mappers

### NoMBC (ROM only)

**Cart types:** 0x00

Simplest case — ROM is mapped directly with no banking. Max 32 KB ROM, no external RAM.

- 0x0000–0x7FFF: direct ROM access
- 0xA000–0xBFFF: returns 0xFF

### MBC1

**Cart types:** 0x01 (ROM), 0x02 (ROM+RAM), 0x03 (ROM+RAM+Battery)

The most common mapper. Supports up to 2 MB ROM and 32 KB RAM.

**Bank registers:**

| Address range | Register | Function |
|---|---|---|
| 0x0000–0x1FFF | RAM enable | Write 0x0A to enable, anything else to disable |
| 0x2000–0x3FFF | ROM bank (lower 5 bits) | Bank 0 maps to bank 1 (zero-avoidance) |
| 0x4000–0x5FFF | RAM bank / upper ROM bits | 2 bits |
| 0x6000–0x7FFF | Banking mode | 0 = ROM mode, 1 = RAM mode |

**Mode 0 (ROM banking):**
- 0x0000–0x3FFF: always bank 0
- 0x4000–0x7FFF: bank N (5-bit + 2-bit upper = 7-bit)
- RAM: always bank 0

**Mode 1 (RAM banking):**
- 0x0000–0x3FFF: bank 0x00/0x20/0x40/0x60 (upper 2 bits applied)
- 0x4000–0x7FFF: bank N (lower 5 bits only)
- RAM: bank 0–3

ROM bank is masked to the actual number of banks: `bank % (rom.len() / 0x4000)`.

### MBC2

**Cart types:** 0x05–0x06

Simple mapper with built-in 512×4-bit RAM (no external RAM chip). Only the lower 4 bits of each byte are used; upper 4 bits read as 1.

**Bank registers:**

| Address range | Register | Function |
|---|---|---|
| 0x0000–0x3FFF (bit 8 = 0) | RAM enable | Low nibble 0x0A enables |
| 0x0000–0x3FFF (bit 8 = 1) | ROM bank (4 bits) | Bank 0 maps to bank 1 |

**RAM:** 512 bytes at 0xA000–0xA1FF, mirrored through 0xBFFF. Only lower 4 bits are valid.

### MBC3

**Cart types:** 0x0F–0x13

Used by Pokemon Gold/Silver/Crystal and other later titles. Simpler banking than MBC1.

**Bank registers:**

| Address range | Register | Function |
|---|---|---|
| 0x0000–0x1FFF | RAM/RTC enable | Write 0x0A to enable |
| 0x2000–0x3FFF | ROM bank (7 bits) | Bank 0 maps to bank 1 |
| 0x4000–0x5FFF | RAM bank / RTC select | 0x00–0x03 = RAM, 0x08–0x0C = RTC |
| 0x6000–0x7FFF | RTC latch | Stubbed |

**RTC:** Banks 0x08–0x0C select real-time clock registers instead of RAM. Currently stubbed — reads return 0, writes are ignored.

### MBC5

**Cart types:** 0x19–0x1E

The most common mapper for later Game Boy and Game Boy Color titles. Supports up to 8 MB ROM (9-bit bank number) and 128 KB RAM (4-bit bank number). Unlike MBC1, bank 0 IS valid for the switchable region.

**Bank registers:**

| Address range | Register | Function |
|---|---|---|
| 0x0000–0x1FFF | RAM enable | Write 0x0A to enable |
| 0x2000–0x2FFF | ROM bank low 8 bits | 0x00–0xFF |
| 0x3000–0x3FFF | ROM bank bit 8 | 1 bit (0 or 1) |
| 0x4000–0x5FFF | RAM bank (4 bits) | 0x00–0x0F |

**Key difference from MBC1:** No bank-0 avoidance — writing 0x00 to the ROM bank register maps bank 0 to the switchable region.

## Battery Save

Cartridge types with battery backup (0x03, 0x0F, 0x10, 0x13, etc.) persist their external RAM across sessions.

### Trait Methods

```rust
trait Cartridge {
    fn has_battery(&self) -> bool;   // Does this cart have battery backup?
    fn ram_data(&self) -> &[u8];     // Get current RAM contents
    fn load_ram(&mut self, data: &[u8]); // Restore RAM from saved data
}
```

### Persistence Flow

1. On ROM load, the JS checks localStorage for `rugb-sram-{title}`
2. If found, the base64-decoded data is passed to `load_battery_ram()`
3. Every 5 seconds, the JS reads `battery_ram_ptr()`/`battery_ram_len()` and saves to localStorage
4. On page unload (`beforeunload` event), a final save is triggered

## Unsupported Mappers

Cart types not matching the supported ranges (0x00, 0x01–0x03, 0x05–0x06, 0x0F–0x13, 0x19–0x1E) fall back to NoMBC with a debug warning. This means games using MBC7, HuC1, HuC3, etc. will not work correctly.

## Adding a New Mapper

1. Create `src/cartridge/your_mbc.rs` implementing the `Cartridge` trait
2. Add the module to `src/cartridge/mod.rs`
3. Add matching cart type codes to the `from_rom()` match statement
4. Set `has_battery` appropriately based on the cart type byte
