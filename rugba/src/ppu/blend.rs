/// GBA color blending / brightness effects.
///
/// BLDCNT (0x04000050): selects blend mode and target layers
///   Bits 0-5: 1st target (BG0-BG3, OBJ, BD)
///   Bits 6-7: blend mode (0=off, 1=alpha, 2=brighten, 3=darken)
///   Bits 8-13: 2nd target (BG0-BG3, OBJ, BD)
/// BLDALPHA (0x04000052): EVA (bits 0-4) / EVB (bits 8-12) for alpha blend
/// BLDY (0x04000054): EVY (bits 0-4) for brightness
const SCREEN_WIDTH: usize = 240;

/// Per-pixel layer info for compositing (stored during BG/OBJ rendering).
/// For now we use a simplified approach: blend the entire scanline post-render.
/// Apply alpha blending between the framebuffer (1st target) and a saved
/// "below" layer. Since we don't have per-pixel layer tracking yet, this
/// blends the current framebuffer toward a reference color (backdrop or
/// semi-transparent approximation).
///
/// EVA = 1st target coefficient (0-16), EVB = 2nd target coefficient (0-16).
pub fn apply_alpha_blend(fb: &mut [u8], below: &[u8], line: usize, eva: u8, evb: u8) {
    let eva = eva.min(16) as u16;
    let evb = evb.min(16) as u16;
    if eva == 16 && evb == 0 {
        return; // No visible blending
    }
    let start = line * SCREEN_WIDTH * 4;

    for x in 0..SCREEN_WIDTH {
        let dst = start + x * 4;
        if dst + 2 >= fb.len() || dst + 2 >= below.len() {
            break;
        }
        let r1 = fb[dst] as u16;
        let g1 = fb[dst + 1] as u16;
        let b1 = fb[dst + 2] as u16;
        let r2 = below[dst] as u16;
        let g2 = below[dst + 1] as u16;
        let b2 = below[dst + 2] as u16;

        fb[dst] = ((r1 * eva + r2 * evb) / 16).min(255) as u8;
        fb[dst + 1] = ((g1 * eva + g2 * evb) / 16).min(255) as u8;
        fb[dst + 2] = ((b1 * eva + b2 * evb) / 16).min(255) as u8;
    }
}

/// Apply brightness fade (increase or decrease) to the entire scanline.
/// Mode 2 = brightness increase (fade to white), Mode 3 = brightness decrease (fade to black).
pub fn apply_brightness(fb: &mut [u8], line: usize, mode: u8, evy: u8) {
    if evy == 0 {
        return;
    }
    let evy = evy.min(16) as u16;
    let start = line * SCREEN_WIDTH * 4;

    for x in 0..SCREEN_WIDTH {
        let dst = start + x * 4;
        let r = fb[dst] as u16;
        let g = fb[dst + 1] as u16;
        let b = fb[dst + 2] as u16;

        let (nr, ng, nb) = if mode == 2 {
            (
                r + ((255 - r) * evy) / 16,
                g + ((255 - g) * evy) / 16,
                b + ((255 - b) * evy) / 16,
            )
        } else {
            (r - (r * evy) / 16, g - (g * evy) / 16, b - (b * evy) / 16)
        };

        fb[dst] = nr.min(255) as u8;
        fb[dst + 1] = ng.min(255) as u8;
        fb[dst + 2] = nb.min(255) as u8;
    }
}
