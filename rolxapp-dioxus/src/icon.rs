//! The app icon, drawn in code so there is no image file to ship: a blue disc with clock hands.

/// RGBA pixels of a `size` x `size` icon.
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let (center, radius) = (s / 2.0, s / 2.0 - 1.0);
    let hand = s * 0.055;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
            let distance = (dx * dx + dy * dy).sqrt();
            // Hour hand points up, minute hand to the right.
            let on_up = dx.abs() <= hand && dy <= hand && dy >= -radius * 0.62;
            let on_right = dy.abs() <= hand && dx >= -hand && dx <= radius * 0.48;
            let pixel = if distance > radius {
                [0, 0, 0, 0]
            } else if on_up || on_right {
                [255, 255, 255, 255]
            } else {
                [59, 108, 246, 255]
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    pixels
}
