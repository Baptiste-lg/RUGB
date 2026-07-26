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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a flat single-scanline framebuffer (240 px × 4 bytes) filled with `color`.
    fn make_fb(color: [u8; 4]) -> Vec<u8> {
        let mut v = vec![0u8; SCREEN_WIDTH * 4];
        for x in 0..SCREEN_WIDTH {
            v[x * 4] = color[0];
            v[x * 4 + 1] = color[1];
            v[x * 4 + 2] = color[2];
            v[x * 4 + 3] = color[3];
        }
        v
    }

    // --- apply_alpha_blend ---

    #[test]
    fn alpha_blend_full_eva_no_change() {
        // eva=16, evb=0 → early return, fb untouched
        let mut fb = make_fb([100, 150, 200, 255]);
        let below = make_fb([0, 0, 0, 255]);
        let original = fb.clone();
        apply_alpha_blend(&mut fb, &below, 0, 16, 0);
        assert_eq!(fb, original);
    }

    #[test]
    fn alpha_blend_half_half() {
        // eva=8, evb=8 → result = (a*8 + b*8)/16 = (a+b)/2
        let mut fb = make_fb([200, 100, 0, 255]);
        let below = make_fb([100, 100, 200, 255]);
        apply_alpha_blend(&mut fb, &below, 0, 8, 8);
        // R: (200*8 + 100*8)/16 = 150
        assert_eq!(fb[0], 150, "R after half-half blend");
        // G: (100*8 + 100*8)/16 = 100
        assert_eq!(fb[1], 100, "G after half-half blend");
        // B: (0*8 + 200*8)/16 = 100
        assert_eq!(fb[2], 100, "B after half-half blend");
    }

    #[test]
    fn alpha_blend_clamped_to_255() {
        // Both channels fully bright → (255*16 + 255*16)/16 = 510 → clamped to 255
        let mut fb = make_fb([255, 255, 255, 255]);
        let below = make_fb([255, 255, 255, 255]);
        apply_alpha_blend(&mut fb, &below, 0, 16, 16);
        // eva=16 AND evb=0 fast-path won't fire here (evb=16), so blend happens
        // But wait — eva=16 && evb=0 is the only early-return case; eva=16, evb=16 continues
        assert_eq!(fb[0], 255);
        assert_eq!(fb[1], 255);
        assert_eq!(fb[2], 255);
    }

    // --- apply_brightness ---

    #[test]
    fn brightness_zero_evy_noop() {
        // evy=0 → early return, fb unchanged
        let mut fb = make_fb([100, 150, 200, 255]);
        let original = fb.clone();
        apply_brightness(&mut fb, 0, 2, 0);
        assert_eq!(fb, original);
    }

    #[test]
    fn brightness_increase() {
        // mode=2, evy=8 → r = r + (255-r)*8/16 = r + (255-r)/2
        let mut fb = make_fb([0, 100, 200, 255]);
        apply_brightness(&mut fb, 0, 2, 8);
        // R: 0 + (255-0)*8/16 = 127
        assert_eq!(fb[0], 127, "R increased");
        // G: 100 + (255-100)*8/16 = 100 + 77 = 177
        assert_eq!(fb[1], 177, "G increased");
        // B: 200 + (255-200)*8/16 = 200 + 27 = 227
        assert_eq!(fb[2], 227, "B increased");
    }

    #[test]
    fn brightness_decrease() {
        // mode=3, evy=8 → r = r - r*8/16 = r/2
        let mut fb = make_fb([200, 100, 0, 255]);
        apply_brightness(&mut fb, 0, 3, 8);
        // R: 200 - 200*8/16 = 200 - 100 = 100
        assert_eq!(fb[0], 100, "R decreased");
        // G: 100 - 100*8/16 = 50
        assert_eq!(fb[1], 50, "G decreased");
        // B: 0 - 0 = 0
        assert_eq!(fb[2], 0, "B decreased");
    }

    #[test]
    fn brightness_evy_capped() {
        // evy > 16 is clamped to 16 → same as evy=16
        let mut fb_capped = make_fb([100, 100, 100, 255]);
        let mut fb_exact = make_fb([100, 100, 100, 255]);
        apply_brightness(&mut fb_capped, 0, 2, 32); // oversized evy
        apply_brightness(&mut fb_exact, 0, 2, 16); // max legal evy
        assert_eq!(fb_capped, fb_exact, "evy>16 should behave as evy=16");
    }
}
