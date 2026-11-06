//! Buffer de color en coma flotante, con volcado a PNG.

use crate::math::{saturate, Vec3};
use std::path::Path;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Vec3>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Framebuffer {
        Framebuffer {
            width,
            height,
            pixels: vec![Vec3::ZERO; width * height],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn clear(&mut self, color: Vec3) {
        self.pixels.iter_mut().for_each(|p| *p = color);
    }

    pub fn set(&mut self, x: usize, y: usize, c: Vec3) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = c;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Vec3 {
        self.pixels[y * self.width + x]
    }

    /// Convierte a 0xRRGGBB para la ventana.
    pub fn to_u32(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|c| {
                let r = (saturate(c.x) * 255.0 + 0.5) as u32;
                let g = (saturate(c.y) * 255.0 + 0.5) as u32;
                let b = (saturate(c.z) * 255.0 + 0.5) as u32;
                (r << 16) | (g << 8) | b
            })
            .collect()
    }

    pub fn to_rgb8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 3);
        for c in &self.pixels {
            out.push((saturate(c.x) * 255.0 + 0.5) as u8);
            out.push((saturate(c.y) * 255.0 + 0.5) as u8);
            out.push((saturate(c.z) * 255.0 + 0.5) as u8);
        }
        out
    }

    pub fn save_png<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let file = std::fs::File::create(&path)
            .map_err(|e| format!("no se pudo crear {}: {e}", path.as_ref().display()))?;
        let w = std::io::BufWriter::new(file);
        let mut enc = png::Encoder::new(w, self.width as u32, self.height as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&self.to_rgb8())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpiar_y_leer() {
        let mut fb = Framebuffer::new(4, 3);
        fb.clear(crate::math::v3(0.5, 0.25, 0.0));
        assert_eq!(fb.get(3, 2), crate::math::v3(0.5, 0.25, 0.0));
    }

    #[test]
    fn to_u32_empaqueta_y_recorta() {
        let mut fb = Framebuffer::new(1, 1);
        fb.set(0, 0, crate::math::v3(2.0, 0.0, -1.0));
        assert_eq!(fb.to_u32()[0], 0x00FF_0000);
        fb.set(0, 0, crate::math::v3(1.0, 1.0, 1.0));
        assert_eq!(fb.to_u32()[0], 0x00FF_FFFF);
    }

}
