//! Процедурная иконка приложения (та же геометрия, что в assets/ps5emu.svg
//! и tools/gen_icon.py): скруглённый квадрат с градиентом, светящееся кольцо
//! и треугольник «play». Оригинальный дизайн, без логотипов третьих лиц.

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    let l = (bx - ax).hypot(by - ay);
    ((bx - ax) * (py - ay) - (by - ay) * (px - ax)) / l
}

/// RGBA8 (не premultiplied), size x size.
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / s;
            let v = (y as f32 + 0.5) / s;

            // скруглённый квадрат (SDF)
            let px = (u - 0.5).abs() - 0.32;
            let py = (v - 0.5).abs() - 0.32;
            let d = px.max(0.0).hypot(py.max(0.0)) + px.max(py).min(0.0) - 0.18;
            let alpha = (0.5 - d * s).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                continue;
            }

            let mut c = mix([24.0, 36.0, 92.0], [88.0, 40.0, 160.0], (u + v) / 2.0);

            // кольцо со свечением
            let dist = (u - 0.5).hypot(v - 0.5);
            let glow = (-(dist - 0.30).abs() * 9.0).exp() * 0.35;
            c = mix(c, [90.0, 170.0, 255.0], glow);
            let ring = (0.5 - ((dist - 0.30).abs() - 0.028) * s).clamp(0.0, 1.0);
            c = mix(c, [155.0, 224.0, 255.0], ring);

            // треугольник
            let e1 = edge(0.43, 0.38, 0.43, 0.62, u, v);
            let e2 = edge(0.43, 0.62, 0.64, 0.50, u, v);
            let e3 = edge(0.64, 0.50, 0.43, 0.38, u, v);
            let inside = (-e1).min(-e2).min(-e3);
            let tri = (0.5 + inside * s).clamp(0.0, 1.0);
            c = mix(c, [255.0, 255.0, 255.0], tri);

            let i = ((y * size + x) * 4) as usize;
            out[i] = c[0] as u8;
            out[i + 1] = c[1] as u8;
            out[i + 2] = c[2] as u8;
            out[i + 3] = (alpha * 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_transparent_center_opaque() {
        let n = 64u32;
        let img = rgba(n);
        assert_eq!(img.len(), (n * n * 4) as usize);
        assert_eq!(img[3], 0); // левый верхний угол прозрачен
        let c = (((n / 2) * n + n / 2) * 4) as usize;
        assert_eq!(img[c + 3], 255);
    }
}
