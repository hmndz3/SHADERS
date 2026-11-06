//! Ruido procedural: simplex 3D (estilo OpenSimplex/Gustavson), fBm, ridged
//! y turbulencia.
//!
//! Todo el ruido se evalúa en 3D sobre la dirección de la esfera, así que los
//! patrones no tienen costuras ni polos: no se usan coordenadas UV planas.

use crate::math::{v3, Vec3};

const GRAD3: [[f32; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

const F3: f32 = 1.0 / 3.0;
const G3: f32 = 1.0 / 6.0;

/// Generador de ruido con tabla de permutaciones propia por semilla.
#[derive(Clone)]
pub struct Noise {
    perm: [u8; 512],
    perm12: [u8; 512],
    seed: u64,
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

impl Noise {
    pub fn new(seed: u64) -> Noise {
        let mut base: [u8; 256] = [0; 256];
        for (i, b) in base.iter_mut().enumerate() {
            *b = i as u8;
        }
        // Barajado determinista de Fisher-Yates.
        let mut st = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for i in (1..256).rev() {
            let j = (xorshift(&mut st) % (i as u64 + 1)) as usize;
            base.swap(i, j);
        }
        let mut perm = [0u8; 512];
        let mut perm12 = [0u8; 512];
        for i in 0..512 {
            perm[i] = base[i & 255];
            perm12[i] = perm[i] % 12;
        }
        Noise { perm, perm12, seed }
    }

    fn grad_index(&self, i: i32, j: i32, k: i32) -> usize {
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;
        let kk = (k & 255) as usize;
        self.perm12[ii + self.perm[jj + self.perm[kk] as usize] as usize] as usize
    }

    /// Ruido simplex 3D en el rango aproximado [-1, 1].
    pub fn simplex3(&self, p: Vec3) -> f32 {
        let (x, y, z) = (p.x, p.y, p.z);
        let s = (x + y + z) * F3;
        let i = (x + s).floor();
        let j = (y + s).floor();
        let k = (z + s).floor();
        let t = (i + j + k) * G3;
        let x0 = x - (i - t);
        let y0 = y - (j - t);
        let z0 = z - (k - t);

        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };

        let corners = [
            (x0, y0, z0, 0, 0, 0),
            (
                x0 - i1 as f32 + G3,
                y0 - j1 as f32 + G3,
                z0 - k1 as f32 + G3,
                i1,
                j1,
                k1,
            ),
            (
                x0 - i2 as f32 + 2.0 * G3,
                y0 - j2 as f32 + 2.0 * G3,
                z0 - k2 as f32 + 2.0 * G3,
                i2,
                j2,
                k2,
            ),
            (
                x0 - 1.0 + 3.0 * G3,
                y0 - 1.0 + 3.0 * G3,
                z0 - 1.0 + 3.0 * G3,
                1,
                1,
                1,
            ),
        ];

        let (ii, jj, kk) = (i as i32, j as i32, k as i32);
        let mut n = 0.0;
        for (cx, cy, cz, di, dj, dk) in corners {
            let mut tt = 0.6 - cx * cx - cy * cy - cz * cz;
            if tt > 0.0 {
                let g = GRAD3[self.grad_index(ii + di, jj + dj, kk + dk)];
                tt *= tt;
                n += tt * tt * (g[0] * cx + g[1] * cy + g[2] * cz);
            }
        }
        32.0 * n
    }

    /// Suma de octavas clásica. Cada octava se rota para que no se alineen los ejes.
    pub fn fbm(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        let mut q = p;
        for _ in 0..octaves {
            sum += self.simplex3(q * freq) * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
            q = rotate_octave(q);
        }
        sum / norm.max(1e-6)
    }

    /// Valor absoluto de las octavas: crea vetas y filamentos. Rango [0, 1].
    pub fn turbulence(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        let mut q = p;
        for _ in 0..octaves {
            sum += self.simplex3(q * freq).abs() * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
            q = rotate_octave(q);
        }
        sum / norm.max(1e-6)
    }

    /// Ruido ridged: crestas afiladas, bueno para cordilleras y fibras de plasma.
    pub fn ridged(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        let mut q = p;
        for _ in 0..octaves {
            let n = 1.0 - self.simplex3(q * freq).abs();
            sum += n * n * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
            q = rotate_octave(q);
        }
        sum / norm.max(1e-6)
    }

}

/// Rotación fija entre octavas (ángulos irracionales) para romper la rejilla.
fn rotate_octave(p: Vec3) -> Vec3 {
    v3(
        0.800 * p.x + 0.600 * p.z,
        0.936 * p.y + 0.352 * p.x,
        -0.600 * p.x + 0.800 * p.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplex_es_determinista() {
        let n = Noise::new(7);
        let p = v3(1.3, -2.7, 0.5);
        assert_eq!(n.simplex3(p), n.simplex3(p));
    }

    #[test]
    fn simplex_se_mantiene_en_rango() {
        let n = Noise::new(42);
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for i in 0..20000 {
            let f = i as f32 * 0.013;
            let v = n.simplex3(v3(f, f * 1.7 - 3.0, -f * 0.6 + 11.0));
            min = min.min(v);
            max = max.max(v);
        }
        assert!(min >= -1.05 && max <= 1.05, "rango {min}..{max}");
        assert!(min < -0.4 && max > 0.4, "el ruido debe variar de verdad");
    }

    #[test]
    fn semillas_distintas_dan_campos_distintos() {
        let a = Noise::new(1);
        let b = Noise::new(2);
        let dif: f32 = (0..200)
            .map(|i| {
                let p = v3(i as f32 * 0.21, 0.4, -1.1);
                (a.simplex3(p) - b.simplex3(p)).abs()
            })
            .sum();
        assert!(dif > 1.0, "dos semillas no deberían coincidir");
    }

    #[test]
    fn simplex_es_continuo() {
        let n = Noise::new(99);
        let p = v3(0.7, 1.2, -0.3);
        let q = p + v3(1e-4, 0.0, 0.0);
        assert!((n.simplex3(p) - n.simplex3(q)).abs() < 1e-2);
    }

    #[test]
    fn fbm_queda_acotado_y_turbulencia_es_positiva() {
        let n = Noise::new(5);
        for i in 0..2000 {
            let p = v3(i as f32 * 0.07, i as f32 * -0.031, 2.0);
            let f = n.fbm(p, 6, 2.0, 0.5);
            assert!((-1.05..=1.05).contains(&f), "fbm fuera de rango: {f}");
            let t = n.turbulence(p, 5, 2.0, 0.5);
            assert!((0.0..=1.05).contains(&t), "turbulencia fuera de rango: {t}");
            let r = n.ridged(p, 5, 2.0, 0.5);
            assert!((0.0..=1.05).contains(&r), "ridged fuera de rango: {r}");
        }
    }

}
