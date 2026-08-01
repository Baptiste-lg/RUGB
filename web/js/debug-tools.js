// SM83 opcode table (abbreviated — covers common opcodes).
// Values ending in 'n' = 1 byte immediate follows.
// Values ending in 'nn' = 2 byte immediate follows.
// Values ending in 'e' = 1 signed byte (branch offset) follows.
const GB_OPCODES = {
    0x00: 'NOP',
    0x01: 'LD BC,nn',
    0x02: 'LD (BC),A',
    0x03: 'INC BC',
    0x04: 'INC B',
    0x05: 'DEC B',
    0x06: 'LD B,n',
    0x07: 'RLCA',
    0x08: 'LD (nn),SP',
    0x09: 'ADD HL,BC',
    0x0A: 'LD A,(BC)',
    0x0B: 'DEC BC',
    0x0C: 'INC C',
    0x0D: 'DEC C',
    0x0E: 'LD C,n',
    0x0F: 'RRCA',
    0x10: 'STOP',
    0x11: 'LD DE,nn',
    0x12: 'LD (DE),A',
    0x13: 'INC DE',
    0x14: 'INC D',
    0x15: 'DEC D',
    0x16: 'LD D,n',
    0x17: 'RLA',
    0x18: 'JR e',
    0x19: 'ADD HL,DE',
    0x1A: 'LD A,(DE)',
    0x1B: 'DEC DE',
    0x1C: 'INC E',
    0x1D: 'DEC E',
    0x1E: 'LD E,n',
    0x1F: 'RRA',
    0x20: 'JR NZ,e',
    0x21: 'LD HL,nn',
    0x22: 'LD (HL+),A',
    0x23: 'INC HL',
    0x24: 'INC H',
    0x25: 'DEC H',
    0x26: 'LD H,n',
    0x27: 'DAA',
    0x28: 'JR Z,e',
    0x29: 'ADD HL,HL',
    0x2A: 'LD A,(HL+)',
    0x2B: 'DEC HL',
    0x2C: 'INC L',
    0x2D: 'DEC L',
    0x2E: 'LD L,n',
    0x2F: 'CPL',
    0x30: 'JR NC,e',
    0x31: 'LD SP,nn',
    0x32: 'LD (HL-),A',
    0x33: 'INC SP',
    0x34: 'INC (HL)',
    0x35: 'DEC (HL)',
    0x36: 'LD (HL),n',
    0x37: 'SCF',
    0x38: 'JR C,e',
    0x39: 'ADD HL,SP',
    0x3A: 'LD A,(HL-)',
    0x3B: 'DEC SP',
    0x3C: 'INC A',
    0x3D: 'DEC A',
    0x3E: 'LD A,n',
    0x3F: 'CCF',
    0x40: 'LD B,B', 0x41: 'LD B,C', 0x42: 'LD B,D', 0x43: 'LD B,E',
    0x44: 'LD B,H', 0x45: 'LD B,L', 0x46: 'LD B,(HL)', 0x47: 'LD B,A',
    0x48: 'LD C,B', 0x49: 'LD C,C', 0x4A: 'LD C,D', 0x4B: 'LD C,E',
    0x4C: 'LD C,H', 0x4D: 'LD C,L', 0x4E: 'LD C,(HL)', 0x4F: 'LD C,A',
    0x50: 'LD D,B', 0x51: 'LD D,C', 0x52: 'LD D,D', 0x53: 'LD D,E',
    0x54: 'LD D,H', 0x55: 'LD D,L', 0x56: 'LD D,(HL)', 0x57: 'LD D,A',
    0x58: 'LD E,B', 0x59: 'LD E,C', 0x5A: 'LD E,D', 0x5B: 'LD E,E',
    0x5C: 'LD E,H', 0x5D: 'LD E,L', 0x5E: 'LD E,(HL)', 0x5F: 'LD E,A',
    0x60: 'LD H,B', 0x61: 'LD H,C', 0x62: 'LD H,D', 0x63: 'LD H,E',
    0x64: 'LD H,H', 0x65: 'LD H,L', 0x66: 'LD H,(HL)', 0x67: 'LD H,A',
    0x68: 'LD L,B', 0x69: 'LD L,C', 0x6A: 'LD L,D', 0x6B: 'LD L,E',
    0x6C: 'LD L,H', 0x6D: 'LD L,L', 0x6E: 'LD L,(HL)', 0x6F: 'LD L,A',
    0x70: 'LD (HL),B', 0x71: 'LD (HL),C', 0x72: 'LD (HL),D', 0x73: 'LD (HL),E',
    0x74: 'LD (HL),H', 0x75: 'LD (HL),L', 0x76: 'HALT', 0x77: 'LD (HL),A',
    0x78: 'LD A,B', 0x79: 'LD A,C', 0x7A: 'LD A,D', 0x7B: 'LD A,E',
    0x7C: 'LD A,H', 0x7D: 'LD A,L', 0x7E: 'LD A,(HL)', 0x7F: 'LD A,A',
    0x80: 'ADD A,B', 0x81: 'ADD A,C', 0x82: 'ADD A,D', 0x83: 'ADD A,E',
    0x84: 'ADD A,H', 0x85: 'ADD A,L', 0x86: 'ADD A,(HL)', 0x87: 'ADD A,A',
    0x88: 'ADC A,B', 0x89: 'ADC A,C', 0x8A: 'ADC A,D', 0x8B: 'ADC A,E',
    0x8C: 'ADC A,H', 0x8D: 'ADC A,L', 0x8E: 'ADC A,(HL)', 0x8F: 'ADC A,A',
    0x90: 'SUB B', 0x91: 'SUB C', 0x92: 'SUB D', 0x93: 'SUB E',
    0x94: 'SUB H', 0x95: 'SUB L', 0x96: 'SUB (HL)', 0x97: 'SUB A',
    0x98: 'SBC A,B', 0x99: 'SBC A,C', 0x9A: 'SBC A,D', 0x9B: 'SBC A,E',
    0x9C: 'SBC A,H', 0x9D: 'SBC A,L', 0x9E: 'SBC A,(HL)', 0x9F: 'SBC A,A',
    0xA0: 'AND B', 0xA1: 'AND C', 0xA2: 'AND D', 0xA3: 'AND E',
    0xA4: 'AND H', 0xA5: 'AND L', 0xA6: 'AND (HL)', 0xA7: 'AND A',
    0xA8: 'XOR B', 0xA9: 'XOR C', 0xAA: 'XOR D', 0xAB: 'XOR E',
    0xAC: 'XOR H', 0xAD: 'XOR L', 0xAE: 'XOR (HL)', 0xAF: 'XOR A',
    0xB0: 'OR B', 0xB1: 'OR C', 0xB2: 'OR D', 0xB3: 'OR E',
    0xB4: 'OR H', 0xB5: 'OR L', 0xB6: 'OR (HL)', 0xB7: 'OR A',
    0xB8: 'CP B', 0xB9: 'CP C', 0xBA: 'CP D', 0xBB: 'CP E',
    0xBC: 'CP H', 0xBD: 'CP L', 0xBE: 'CP (HL)', 0xBF: 'CP A',
    0xC0: 'RET NZ', 0xC1: 'POP BC', 0xC2: 'JP NZ,nn', 0xC3: 'JP nn',
    0xC4: 'CALL NZ,nn', 0xC5: 'PUSH BC', 0xC6: 'ADD A,n', 0xC7: 'RST 00',
    0xC8: 'RET Z', 0xC9: 'RET', 0xCA: 'JP Z,nn', 0xCB: 'CB prefix',
    0xCC: 'CALL Z,nn', 0xCD: 'CALL nn', 0xCE: 'ADC A,n', 0xCF: 'RST 08',
    0xD0: 'RET NC', 0xD1: 'POP DE', 0xD2: 'JP NC,nn', 0xD3: 'ILLEGAL',
    0xD4: 'CALL NC,nn', 0xD5: 'PUSH DE', 0xD6: 'SUB n', 0xD7: 'RST 10',
    0xD8: 'RET C', 0xD9: 'RETI', 0xDA: 'JP C,nn', 0xDB: 'ILLEGAL',
    0xDC: 'CALL C,nn', 0xDD: 'ILLEGAL', 0xDE: 'SBC A,n', 0xDF: 'RST 18',
    0xE0: 'LDH (n),A', 0xE1: 'POP HL', 0xE2: 'LD (C),A', 0xE3: 'ILLEGAL',
    0xE4: 'ILLEGAL', 0xE5: 'PUSH HL', 0xE6: 'AND n', 0xE7: 'RST 20',
    0xE8: 'ADD SP,e', 0xE9: 'JP HL', 0xEA: 'LD (nn),A', 0xEB: 'ILLEGAL',
    0xEC: 'ILLEGAL', 0xED: 'ILLEGAL', 0xEE: 'XOR n', 0xEF: 'RST 28',
    0xF0: 'LDH A,(n)', 0xF1: 'POP AF', 0xF2: 'LD A,(C)', 0xF3: 'DI',
    0xF4: 'ILLEGAL', 0xF5: 'PUSH AF', 0xF6: 'OR n', 0xF7: 'RST 30',
    0xF8: 'LD HL,SP+e', 0xF9: 'LD SP,HL', 0xFA: 'LD A,(nn)', 0xFB: 'EI',
    0xFC: 'ILLEGAL', 0xFD: 'ILLEGAL', 0xFE: 'CP n', 0xFF: 'RST 38',
};

// Opcodes that consume an extra 8-bit immediate (n)
const GB_IMM8 = new Set([
    0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E,
    0xC6, 0xCE, 0xD6, 0xDE, 0xE0, 0xE6, 0xEE, 0xF0, 0xF6, 0xFE,
]);

// Opcodes that consume an extra 16-bit immediate (nn)
const GB_IMM16 = new Set([
    0x01, 0x08, 0x11, 0x21, 0x31,
    0xC2, 0xC3, 0xC4, 0xCA, 0xCC, 0xCD,
    0xD2, 0xD4, 0xDA, 0xDC,
    0xEA, 0xFA,
]);

// Opcodes that consume a signed 8-bit offset (e) — JR instructions
const GB_REL8 = new Set([0x18, 0x20, 0x28, 0x30, 0x38, 0xE8, 0xF8]);

function hex2(n) { return n.toString(16).toUpperCase().padStart(2, '0'); }
function hex4(n) { return n.toString(16).toUpperCase().padStart(4, '0'); }
function hex8(n) { return (n >>> 0).toString(16).toUpperCase().padStart(8, '0'); }

function gbDisasmOne(emu, addr) {
    const op = emu.read_byte(addr);
    let name = GB_OPCODES[op] || `DB $${hex2(op)}`;
    let size = 1;
    let extra = '';

    if (GB_IMM8.has(op)) {
        const imm = emu.read_byte((addr + 1) & 0xFFFF);
        name = name.replace('n', `$${hex2(imm)}`);
        extra = hex2(imm);
        size = 2;
    } else if (GB_REL8.has(op)) {
        const raw = emu.read_byte((addr + 1) & 0xFFFF);
        const offset = raw >= 0x80 ? raw - 256 : raw;
        const target = (addr + 2 + offset) & 0xFFFF;
        name = name.replace('e', `$${hex4(target)}`);
        extra = hex2(raw);
        size = 2;
    } else if (GB_IMM16.has(op)) {
        const lo = emu.read_byte((addr + 1) & 0xFFFF);
        const hi = emu.read_byte((addr + 2) & 0xFFFF);
        const imm = (hi << 8) | lo;
        name = name.replace('nn', `$${hex4(imm)}`);
        extra = `${hex2(lo)} ${hex2(hi)}`;
        size = 3;
    }

    const bytes = [`${hex2(op)}`, ...extra.split(' ').filter(Boolean)];
    return { addr, bytes, name, size };
}

export class DebugTools {
    constructor(getEmu, getSystem, getWasm) {
        this.getEmu = getEmu;
        this.getSystem = getSystem;
        this.getWasm = getWasm;
        this.activeTab = 'cpu';
        this.memoryOffset = 0;
        this.autoUpdate = false;
        this._tileCanvas = null;
    }

    update() {
        const panel = document.getElementById('debug-panel');
        if (!panel) return;
        const emu = this.getEmu();
        if (!emu) { panel.innerHTML = '<p class="debug-empty">No ROM loaded.</p>'; return; }

        switch (this.activeTab) {
            case 'cpu':    this.renderCPU(panel); break;
            case 'memory': this.renderMemory(panel); break;
            case 'tiles':  this.renderTiles(panel); break;
            case 'disasm': this.renderDisasm(panel); break;
        }
    }

    renderCPU(container) {
        const emu = this.getEmu();
        const system = this.getSystem();

        let html = '<div class="debug-regs">';
        if (system === 'gba') {
            const cpsr = emu.cpu_cpsr();
            const mode = cpsr & 0x1F;
            const thumb = (cpsr >> 5) & 1;
            const fiq   = (cpsr >> 6) & 1;
            const irq   = (cpsr >> 7) & 1;
            const flagV = (cpsr >> 28) & 1;
            const flagC = (cpsr >> 29) & 1;
            const flagZ = (cpsr >> 30) & 1;
            const flagN = (cpsr >> 31) & 1;
            const modeNames = { 0x10: 'USR', 0x11: 'FIQ', 0x12: 'IRQ', 0x13: 'SVC', 0x17: 'ABT', 0x1B: 'UND', 0x1F: 'SYS' };
            const modeName = modeNames[mode] || `M${hex2(mode)}`;

            for (let i = 0; i < 16; i++) {
                const regNames = ['R0','R1','R2','R3','R4','R5','R6','R7',
                                  'R8','R9','R10','R11','R12','SP','LR','PC'];
                html += `<span class="debug-reg-label">${regNames[i]}</span><span class="debug-reg-val">$${hex8(emu.cpu_reg(i))}</span>`;
            }
            html += `<span class="debug-reg-label">CPSR</span><span class="debug-reg-val">$${hex8(cpsr)}</span>`;
            html += `<span class="debug-reg-label">Mode</span><span class="debug-reg-val">${modeName} ${thumb ? 'THUMB' : 'ARM'}</span>`;
            html += `<span class="debug-reg-label">Flags</span><span class="debug-reg-val">`;
            html += `<span class="flag-bit ${flagN ? 'flag-set' : ''}">N</span>`;
            html += `<span class="flag-bit ${flagZ ? 'flag-set' : ''}">Z</span>`;
            html += `<span class="flag-bit ${flagC ? 'flag-set' : ''}">C</span>`;
            html += `<span class="flag-bit ${flagV ? 'flag-set' : ''}">V</span>`;
            html += `<span class="flag-bit ${irq ? '' : 'flag-set'}">I</span>`;
            html += `<span class="flag-bit ${fiq ? '' : 'flag-set'}">F</span>`;
            html += '</span>';
        } else {
            const af = emu.cpu_af();
            const bc = emu.cpu_bc();
            const de = emu.cpu_de();
            const hl = emu.cpu_hl();
            const pc = emu.cpu_pc();
            const sp = emu.cpu_sp();
            const f  = af & 0xFF;
            const flagZ = (f >> 7) & 1;
            const flagN = (f >> 6) & 1;
            const flagH = (f >> 5) & 1;
            const flagC = (f >> 4) & 1;
            const halted = emu.cpu_halted();
            const ime    = emu.cpu_ime();

            html += `<span class="debug-reg-label">AF</span><span class="debug-reg-val">$${hex4(af)}</span>`;
            html += `<span class="debug-reg-label">BC</span><span class="debug-reg-val">$${hex4(bc)}</span>`;
            html += `<span class="debug-reg-label">DE</span><span class="debug-reg-val">$${hex4(de)}</span>`;
            html += `<span class="debug-reg-label">HL</span><span class="debug-reg-val">$${hex4(hl)}</span>`;
            html += `<span class="debug-reg-label">SP</span><span class="debug-reg-val">$${hex4(sp)}</span>`;
            html += `<span class="debug-reg-label">PC</span><span class="debug-reg-val">$${hex4(pc)}</span>`;
            html += `<span class="debug-reg-label">Flags</span><span class="debug-reg-val">`;
            html += `<span class="flag-bit ${flagZ ? 'flag-set' : ''}">Z</span>`;
            html += `<span class="flag-bit ${flagN ? 'flag-set' : ''}">N</span>`;
            html += `<span class="flag-bit ${flagH ? 'flag-set' : ''}">H</span>`;
            html += `<span class="flag-bit ${flagC ? 'flag-set' : ''}">C</span>`;
            html += '</span>';
            html += `<span class="debug-reg-label">HALT</span><span class="debug-reg-val ${halted ? 'flag-set' : ''}">${halted ? 'YES' : 'no'}</span>`;
            html += `<span class="debug-reg-label">IME</span><span class="debug-reg-val ${ime ? 'flag-set' : ''}">${ime ? 'YES' : 'no'}</span>`;
        }

        html += '</div>';
        container.innerHTML = html;
    }

    renderMemory(container) {
        const emu = this.getEmu();
        const system = this.getSystem();
        const isGba = system === 'gba';
        const offset = this.memoryOffset;
        const ROWS = 16;
        const COLS = 16;
        const total = ROWS * COLS;

        // Read 256 bytes
        const bytes = new Uint8Array(total);
        for (let i = 0; i < total; i++) {
            try {
                bytes[i] = isGba ? emu.read_byte(offset + i) : emu.read_byte((offset + i) & 0xFFFF);
            } catch { bytes[i] = 0xFF; }
        }

        let html = '<div class="debug-mem-controls">';
        html += `<button id="dbg-mem-prev">&#8592; -256</button>`;
        html += `<span class="debug-mem-addr">${isGba ? '$' + hex8(offset) : '$' + hex4(offset & 0xFFFF)}</span>`;
        html += `<button id="dbg-mem-next">+256 &#8594;</button>`;
        html += '</div>';
        html += '<div class="debug-hex">';

        // Header row
        html += '<span class="dbg-hex-addr">Addr</span>';
        for (let c = 0; c < COLS; c++) {
            html += `<span class="dbg-hex-hdr">${hex2(c)}</span>`;
        }
        html += '<span class="dbg-hex-hdr">ASCII</span>';

        for (let r = 0; r < ROWS; r++) {
            const rowAddr = offset + r * COLS;
            html += `<span class="dbg-hex-addr">${isGba ? hex8(rowAddr) : hex4(rowAddr & 0xFFFF)}</span>`;
            let ascii = '';
            for (let c = 0; c < COLS; c++) {
                const b = bytes[r * COLS + c];
                html += `<span class="dbg-hex-byte">${hex2(b)}</span>`;
                ascii += (b >= 0x20 && b <= 0x7E) ? String.fromCharCode(b) : '.';
            }
            html += `<span class="dbg-hex-ascii">${ascii}</span>`;
        }

        html += '</div>';
        container.innerHTML = html;

        container.querySelector('#dbg-mem-prev')?.addEventListener('click', () => {
            this.memoryOffset = Math.max(0, this.memoryOffset - 256);
            this.update();
        });
        container.querySelector('#dbg-mem-next')?.addEventListener('click', () => {
            const max = isGba ? 0xFFFFFF00 : 0xFF00;
            this.memoryOffset = Math.min(max, this.memoryOffset + 256);
            this.update();
        });
    }

    renderTiles(container) {
        const emu = this.getEmu();
        const system = this.getSystem();

        if (system === 'gba') {
            container.innerHTML = '<p class="debug-empty">Tile viewer not available for GBA.</p>';
            return;
        }

        // GB/GBC: 384 tiles in VRAM at 0x8000..0x97FF (each tile = 16 bytes, 8x8 pixels, 2bpp)
        const TILE_COUNT = 384;
        const TILES_PER_ROW = 16;
        const TILE_ROWS = Math.ceil(TILE_COUNT / TILES_PER_ROW); // 24
        const TILE_PX = 8;
        const SCALE = 2;
        const W = TILES_PER_ROW * TILE_PX * SCALE;
        const H = TILE_ROWS * TILE_PX * SCALE;

        // Reuse the canvas element if possible
        if (!this._tileCanvas) {
            this._tileCanvas = document.createElement('canvas');
        }
        const cv = this._tileCanvas;
        cv.width = W;
        cv.height = H;
        cv.className = 'debug-tile-canvas';
        const ctx = cv.getContext('2d');
        const img = ctx.createImageData(W, H);
        const data = img.data;

        // Read BGP (0xFF47) for shade-to-color mapping
        const bgp = emu.read_byte(0xFF47);
        // Shade 0..3 -> palette index 0..3
        const shadeR = [0xE0, 0x90, 0x40, 0x10]; // greenish DMG colours
        const shadeG = [0xF8, 0xD0, 0x88, 0x38];
        const shadeB = [0xD0, 0xA0, 0x50, 0x20];

        for (let t = 0; t < TILE_COUNT; t++) {
            const tileAddr = 0x8000 + t * 16;
            const tx = (t % TILES_PER_ROW) * TILE_PX * SCALE;
            const ty = Math.floor(t / TILES_PER_ROW) * TILE_PX * SCALE;

            for (let row = 0; row < TILE_PX; row++) {
                const lo = emu.read_byte(tileAddr + row * 2);
                const hi = emu.read_byte(tileAddr + row * 2 + 1);
                for (let col = 0; col < TILE_PX; col++) {
                    const bit = 7 - col;
                    const rawShade = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
                    const palIdx = (bgp >> (rawShade * 2)) & 0x03;

                    const r = shadeR[palIdx];
                    const g = shadeG[palIdx];
                    const b = shadeB[palIdx];

                    // Scale pixel up by SCALE
                    for (let sy = 0; sy < SCALE; sy++) {
                        for (let sx = 0; sx < SCALE; sx++) {
                            const px = tx + col * SCALE + sx;
                            const py = ty + row * SCALE + sy;
                            const idx = (py * W + px) * 4;
                            data[idx] = r;
                            data[idx + 1] = g;
                            data[idx + 2] = b;
                            data[idx + 3] = 255;
                        }
                    }
                }
            }
        }

        ctx.putImageData(img, 0, 0);
        container.innerHTML = '<p class="debug-label">VRAM tiles (0x8000-0x97FF, 384 tiles, 16x24 grid)</p>';
        container.appendChild(cv);
    }

    renderDisasm(container) {
        const emu = this.getEmu();
        const system = this.getSystem();

        if (system === 'gba') {
            const cpsr = emu.cpu_cpsr();
            const thumb = (cpsr >> 5) & 1;
            const pc = emu.cpu_reg(15);
            const mode = thumb ? 'THUMB' : 'ARM';
            const wordSize = thumb ? 2 : 4;

            let html = `<p class="debug-label">GBA ${mode} mode — showing raw words around PC</p>`;
            html += '<table class="debug-disasm">';
            const start = Math.max(0, pc - 8 * wordSize);
            for (let i = 0; i < 16; i++) {
                const addr = start + i * wordSize;
                const isCurrent = addr === (pc & (thumb ? ~1 : ~3));
                let word = 0;
                for (let b = 0; b < wordSize; b++) {
                    word |= emu.read_byte(addr + b) << (b * 8);
                }
                const wordStr = thumb ? hex4(word & 0xFFFF) : hex8(word >>> 0);
                html += `<tr class="${isCurrent ? 'disasm-current' : ''}">`;
                html += `<td class="disasm-addr">$${hex8(addr)}</td>`;
                html += `<td class="disasm-bytes">${wordStr}</td>`;
                html += `<td class="disasm-mnem">${mode} ${wordStr}</td>`;
                html += '</tr>';
            }
            html += '</table>';
            container.innerHTML = html;
            return;
        }

        // GB/GBC: SM83 disassembly
        const pc = emu.cpu_pc();

        // Walk backward to find a reasonable start address (heuristic: go back ~8 instructions)
        // Simple: just start 24 bytes before PC and decode forward
        let startAddr = (pc - 24 + 0x10000) & 0xFFFF;

        // Collect instructions starting from startAddr until we have ~24
        const instrs = [];
        let addr = startAddr;
        while (instrs.length < 24) {
            const instr = gbDisasmOne(emu, addr);
            instrs.push(instr);
            addr = (addr + instr.size) & 0xFFFF;
        }

        // Find PC in the list — show 6 before and rest after
        let pcIdx = instrs.findIndex(i => i.addr === pc);
        if (pcIdx < 0) pcIdx = 8; // fallback
        const displayStart = Math.max(0, pcIdx - 6);
        const display = instrs.slice(displayStart, displayStart + 16);

        let html = '<table class="debug-disasm">';
        for (const instr of display) {
            const isCurrent = instr.addr === pc;
            const bytesStr = instr.bytes.join(' ');
            html += `<tr class="${isCurrent ? 'disasm-current' : ''}">`;
            html += `<td class="disasm-addr">$${hex4(instr.addr)}</td>`;
            html += `<td class="disasm-bytes">${bytesStr}</td>`;
            html += `<td class="disasm-mnem">${instr.name}</td>`;
            html += '</tr>';
        }
        html += '</table>';
        container.innerHTML = html;
    }
}
