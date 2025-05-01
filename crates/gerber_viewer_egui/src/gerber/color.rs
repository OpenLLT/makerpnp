use egui::Color32;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};

pub fn generate_pastel_color(index: u64) -> Color32 {
    let mut rng = SmallRng::seed_from_u64(index);

    let hue = rng.random_range(0.0..360.0);
    let saturation = rng.random_range(0.2..0.3);
    let value = rng.random_range(0.8..1.0);

    let (r, g, b) = hsv_to_rgb(hue, saturation, value);
    Color32::from_rgb(r, g, b)
}

pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
    let hue = hue % 360.0;
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - chroma;

    let sector = (hue / 60.0) as u8;
    let (r1, g1, b1) = match sector {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        5 => (chroma, 0.0, x),
        _ => (0.0, 0.0, 0.0), // Unreachable due to modulus
    };

    // Calculate each RGB component and clamp to valid range
    let red = ((r1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let green = ((g1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let blue = ((b1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (red, green, blue)
}
