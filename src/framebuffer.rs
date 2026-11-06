//! Buffer de color en coma flotante, con reducción por supermuestreo y volcado a PNG.

use crate::math::{saturate, v3, Vec3};
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

    /// Promedia bloques de `factor` x `factor` píxeles (antialiasing por supermuestreo).
    pub fn downsample(&self, factor: usize) -> Framebuffer {
        if factor <= 1 {
            return Framebuffer {
                width: self.width,
                height: self.height,
                pixels: self.pixels.clone(),
            };
        }
        let w = self.width / factor;
        let h = self.height / factor;
        let mut out = Framebuffer::new(w, h);
        let inv = 1.0 / (factor * factor) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = Vec3::ZERO;
                for sy in 0..factor {
                    for sx in 0..factor {
                        acc += self.get(x * factor + sx, y * factor + sy);
                    }
                }
                out.set(x, y, acc * inv);
            }
        }
        out
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

/// Color de fondo: un degradado muy oscuro con estrellas lejanas procedurales.
pub fn background(x: f32, y: f32, noise: &crate::noise::Noise) -> Vec3 {
    let base = v3(0.004, 0.006, 0.014) * (1.0 - 0.45 * y);
    let p = v3(x * 180.0, y * 180.0, 7.0);
    let (f1, _) = noise.worley(p);
    let star = ((1.0 - f1 * 7.0).max(0.0)).powf(10.0);
    let tint = v3(0.85, 0.9, 1.0) + v3(0.25, 0.05, -0.15) * noise.simplex3(p * 0.37);
    base + tint * star * 0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpiar_y_leer() {
        let mut fb = Framebuffer::new(4, 3);
        fb.clear(v3(0.5, 0.25, 0.0));
        assert_eq!(fb.get(3, 2), v3(0.5, 0.25, 0.0));
    }

    #[test]
    fn to_u32_empaqueta_y_recorta() {
        let mut fb = Framebuffer::new(1, 1);
        fb.set(0, 0, v3(2.0, 0.0, -1.0));
        assert_eq!(fb.to_u32()[0], 0x00FF_0000);
        fb.set(0, 0, v3(1.0, 1.0, 1.0));
        assert_eq!(fb.to_u32()[0], 0x00FF_FFFF);
    }

    #[test]
    fn downsample_promedia_los_bloques() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set(0, 0, Vec3::ONE);
        fb.set(1, 0, Vec3::ZERO);
        fb.set(0, 1, Vec3::ZERO);
        fb.set(1, 1, Vec3::ZERO);
        let small = fb.downsample(2);
        assert_eq!(small.width, 1);
        assert!((small.get(0, 0).x - 0.25).abs() < 1e-6);
    }

    #[test]
    fn el_fondo_es_oscuro_casi_siempre() {
        let n = crate::noise::Noise::new(2024);
        let mut brillantes = 0;
        for i in 0..2000 {
            let x = (i % 50) as f32 / 50.0;
            let y = (i / 50) as f32 / 40.0;
            if background(x, y, &n).length() > 0.25 {
                brillantes += 1;
            }
        }
        assert!(brillantes < 200, "demasiadas estrellas de fondo: {brillantes}");
    }
}
