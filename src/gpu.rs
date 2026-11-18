//! Заглушка GPU (RDNA 2) и программный framebuffer.
//! Реальная эмуляция потребует трансляции командных буферов в Vulkan
//! и компиляции шейдеров.

pub trait Gpu {
    fn submit_command_buffer(&mut self, _buf: &[u8]) {}
    fn present(&mut self) {}
}

pub struct NullGpu;
impl Gpu for NullGpu {}

/// Простой framebuffer 0x00RRGGBB с экспортом в PPM для отладки.
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![0; width * height] }
    }

    pub fn fill(&mut self, color: u32) {
        self.pixels.iter_mut().for_each(|p| *p = color);
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for &p in &self.pixels {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ppm_has_expected_size_and_pixel() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(1, 0, 0xFF8040);
        fb.set_pixel(99, 99, 1); // за границей: игнорируется
        let ppm = fb.to_ppm();
        let header = b"P6\n2 2\n255\n".len();
        assert_eq!(ppm.len(), header + 12);
        assert_eq!(&ppm[header + 3..header + 6], &[0xFF, 0x80, 0x40]);
    }
}
