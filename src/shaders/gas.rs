//! ÁMBAR — gigante gaseoso.
//!
//! Cadena de cálculo de un fragmento:
//!   1. DEFORMACIÓN DE DOMINIO: fBm que mueve las coordenadas antes de calcular las
//!      bandas. Es lo que vuelve "líquida" la transición entre una barra y la otra.
//!   2. BANDAS: suma de tres senos de distinta frecuencia sobre la latitud ya
//!      deformada, más ruido ridged para los festones del borde.
//!   3. COLOR: rampa ámbar de seis tonos.
//!   4. LUZ: difusa suave, oscurecimiento de borde y halo atmosférico cálido.

use super::{ramp, wrap_diffuse, Fragment, Shader, Uniforms};
use crate::math::{rotate_y, saturate, v3, Vec3};
use crate::noise::Noise;

pub struct GasGiant {
    bandas_n: Noise,
    turbulencia_n: Noise,
    /// Cantidad de bandas de polo a polo.
    pub frecuencia_bandas: f32,
    /// Cuánto deforma el ruido las bandas: 0 = barras rectas, alto = fluido.
    pub deformacion: f32,
    /// Velocidad general del flujo atmosférico.
    pub velocidad: f32,
}

impl Default for GasGiant {
    fn default() -> GasGiant {
        GasGiant {
            bandas_n: Noise::new(707),
            turbulencia_n: Noise::new(808),
            frecuencia_bandas: 9.0,
            deformacion: 0.26,
            velocidad: 0.20,
        }
    }
}

const PALETA: [Vec3; 6] = [
    v3(0.130, 0.070, 0.045),
    v3(0.330, 0.175, 0.085),
    v3(0.600, 0.370, 0.170),
    v3(0.820, 0.620, 0.360),
    v3(0.940, 0.840, 0.660),
    v3(0.995, 0.965, 0.905),
];

impl GasGiant {
    /// Patrón de bandas en [0, 1], ya deformado y en movimiento.
    pub fn patron(&self, p: Vec3, time: f32) -> f32 {
        let q = rotate_y(p, time * 0.18);

        let w = self
            .turbulencia_n
            .warp(q * 1.6, self.deformacion, 1.0, 3);
        let fino = self.turbulencia_n.fbm(q * 7.5, 4, 2.2, 0.5);
        let y = w.y + 0.05 * fino;

        let k = self.frecuencia_bandas;
        let bandas = (y * k).sin() * 0.50
            + (y * k * 2.3 + 1.1).sin() * 0.30
            + (y * k * 0.47 - 0.6).sin() * 0.20;
        let v = saturate(bandas * 0.5 + 0.5);

        let festones = self.bandas_n.ridged(w * 4.5, 4, 2.1, 0.55);
        saturate(v * 0.84 + festones * 0.16)
    }
}

impl Shader for GasGiant {
    fn nombre(&self) -> &'static str {
        "Ámbar (gigante gaseoso)"
    }

    fn descripcion(&self) -> &'static str {
        "bandas líquidas por deformación de dominio"
    }

    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3 {
        let p = f.dir;
        let t = u.time * self.velocidad;

        let v = self.patron(p, t);
        let mut color = ramp(&PALETA, v);

        // Iluminación: difusa suave, oscurecimiento de borde y halo cálido.
        let ndotl = f.normal.dot(f.light_dir);
        let dif = wrap_diffuse(ndotl, 0.25).powf(0.85);
        let limbo = saturate(f.normal.dot(f.view_dir)).powf(0.42);
        color = color * (0.045 + 0.955 * dif) * (0.45 + 0.55 * limbo);

        let borde = saturate(1.0 - saturate(f.normal.dot(f.view_dir)));
        color += v3(0.95, 0.72, 0.45) * borde.powf(3.6) * (0.15 + 0.85 * f.intensity) * 0.75;

        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::v3;

    fn frag(dir: Vec3) -> Fragment {
        let n = dir.normalize();
        Fragment {
            dir: n,
            normal: n,
            view_dir: n,
            light_dir: v3(0.3, 0.3, 0.9).normalize(),
            intensity: saturate(n.dot(v3(0.3, 0.3, 0.9).normalize())),
        }
    }

    #[test]
    fn el_patron_varia_sobre_todo_con_la_latitud() {
        let g = GasGiant::default();
        // Recorriendo latitudes debe haber más cambio que recorriendo longitudes.
        let por_latitud: f32 = (0..60)
            .map(|i| {
                let y = -0.95 + i as f32 * 0.032;
                let r = (1.0 - y * y).sqrt();
                g.patron(v3(r, y, 0.0), 0.0)
            })
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum();
        let por_longitud: f32 = (0..60)
            .map(|i| {
                let a = i as f32 * 0.1047;
                g.patron(v3(a.cos() * 0.6, 0.8, a.sin() * 0.6).normalize(), 0.0)
            })
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum();
        assert!(
            por_latitud > por_longitud,
            "deberían verse bandas horizontales: lat {por_latitud} vs lon {por_longitud}"
        );
    }

    #[test]
    fn mas_frecuencia_da_mas_bandas() {
        let cruces = |k: f32| {
            let mut g = GasGiant::default();
            g.frecuencia_bandas = k;
            let vals: Vec<f32> = (0..400)
                .map(|i| {
                    let y = -0.98 + i as f32 * 0.0049;
                    let r = (1.0 - y * y).sqrt();
                    g.patron(v3(r, y, 0.0), 0.0)
                })
                .collect();
            vals.windows(2)
                .filter(|w| (w[0] - 0.5).signum() != (w[1] - 0.5).signum())
                .count()
        };
        assert!(cruces(16.0) > cruces(5.0), "subir la frecuencia debe apretar las bandas");
    }

    #[test]
    fn la_deformacion_hace_que_las_bandas_dejen_de_ser_rectas() {
        let recto = {
            let mut g = GasGiant::default();
            g.deformacion = 0.0;
            g
        };
        let fluido = {
            let mut g = GasGiant::default();
            g.deformacion = 0.45;
            g
        };
        // Variación a lo largo de una misma latitud: con deformación debe ser mayor.
        let variacion = |g: &GasGiant| -> f32 {
            let y = 0.25f32;
            let r = (1.0 - y * y).sqrt();
            let vals: Vec<f32> = (0..120)
                .map(|i| {
                    let a = i as f32 * 0.0524;
                    g.patron(v3(a.cos() * r, y, a.sin() * r), 0.0)
                })
                .collect();
            vals.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
        };
        assert!(
            variacion(&fluido) > variacion(&recto) * 1.5,
            "recto {} fluido {}",
            variacion(&recto),
            variacion(&fluido)
        );
    }

    #[test]
    fn la_atmosfera_fluye_con_el_tiempo() {
        let g = GasGiant::default();
        let mut dif = 0.0;
        for i in 0..200 {
            let a = i as f32 * 0.31;
            let p = v3(a.cos(), (a * 0.4).sin() * 0.8, a.sin()).normalize();
            dif += (g.patron(p, 0.0) - g.patron(p, 12.0)).abs();
        }
        assert!(dif / 200.0 > 0.05, "las bandas deberían moverse");
    }

    #[test]
    fn el_color_queda_en_rango_visible() {
        let g = GasGiant::default();
        for i in 0..1200 {
            let a = i as f32 * 0.093;
            let p = v3(a.sin(), (a * 0.57).cos(), (a * 1.21).sin()).normalize();
            let c = g.shade(&frag(p), &Uniforms { time: a });
            assert!(c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0, "color negativo {c:?}");
            assert!(c.x < 3.0 && c.y < 3.0 && c.z < 3.0, "color desbordado {c:?}");
        }
    }
}
