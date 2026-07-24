/// GBA color blending / brightness effects.
///
/// BLDCNT (0x04000050): selects blend mode and target layers
/// BLDALPHA (0x04000052): EVA/EVB coefficients for alpha blend
/// BLDY (0x04000054): EVY coefficient for brightness

const SCREEN_WIDTH: usize = 240;

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
            // Brightness increase: color + (255 - color) * EVY / 16
            (
                r + ((255 - r) * evy) / 16,
                g + ((255 - g) * evy) / 16,
                b + ((255 - b) * evy) / 16,
            )
        } else {
            // Brightness decrease: color - color * EVY / 16
            (
                r - (r * evy) / 16,
                g - (g * evy) / 16,
                b - (b * evy) / 16,
            )
        };

        fb[dst] = nr.min(255) as u8;
        fb[dst + 1] = ng.min(255) as u8;
        fb[dst + 2] = nb.min(255) as u8;
    }
}
