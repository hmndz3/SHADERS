//! ÁMBAR — gigante gaseoso.
//!
//! Cadena de cálculo de un fragmento:
//!   1. VÓRTICES: antes de evaluar nada, el punto se rota alrededor del centro de
//!      cada tormenta con un ángulo que decae con la distancia. Eso curva las
//!      bandas que pasan cerca y las arrastra hacia adentro.
//!   2. CIZALLA: cada latitud se desplaza en longitud a distinta velocidad, así
//!      los bordes entre bandas se rompen en remolinos.
//!   3. DEFORMACIÓN DE DOMINIO: fBm que mueve las coordenadas antes de calcular las
//!      bandas. Es lo que vuelve "líquida" la transición entre una barra y la otra.
//!   4. BANDAS: suma de tres senos de distinta frecuencia sobre la latitud ya
//!      deformada, más ruido ridged para los festones del borde.
//!   5. COLOR: rampa ámbar, y la gran tormenta pinta encima con su propio fBm.
//!   6. LUZ: difusa suave, oscurecimiento de borde y halo atmosférico cálido.

use super::{mix, ramp, wrap_diffuse, Fragment, Shader, Uniforms};
use crate::math::{rotate_axis, rotate_y, saturate, smoothstep, v3, Vec3};
use crate::noise::Noise;

/// Una tormenta ciclónica: centro sobre la esfera, radio angular y giro.
#[derive(Clone, Copy)]
pub struct Tormenta {
    pub centro: Vec3,
    pub radio: f32,
    pub fuerza: f32,
    pub sentido: f32,
}

pub struct GasGiant {
    bandas_n: Noise,
    turbulencia_n: Noise,
    vortice_n: Noise,
    /// Cantidad de bandas de polo a polo.
    pub frecuencia_bandas: f32,
    /// Cuánto deforma el ruido las bandas: 0 = barras rectas, alto = fluido.
    pub deformacion: f32,
    /// Diferencia de velocidad entre bandas vecinas.
    pub cizalla: f32,
    /// Velocidad general del flujo atmosférico.
    pub velocidad: f32,
    /// Multiplicador del giro de todas las tormentas.
    pub fuerza_tormenta: f32,
    pub tormentas: Vec<Tormenta>,
}

impl Default for GasGiant {
    fn default() -> GasGiant {
        GasGiant {
            bandas_n: Noise::new(707),
            turbulencia_n: Noise::new(808),
            vortice_n: Noise::new(909),
            frecuencia_bandas: 9.0,
            deformacion: 0.26,
            cizalla: 0.55,
            velocidad: 0.20,
            fuerza_tormenta: 1.0,
            tormentas: vec![
                Tormenta {
                    centro: v3(0.62, -0.30, 0.72),
                    radio: 0.46,
                    fuerza: 2.6,
                    sentido: 1.0,
                },
                Tormenta {
                    centro: v3(-0.55, 0.42, 0.72),
                    radio: 0.20,
                    fuerza: 1.7,
                    sentido: -1.0,
                },
                Tormenta {
                    centro: v3(-0.20, -0.62, -0.75),
                    radio: 0.16,
                    fuerza: 1.4,
                    sentido: 1.0,
                },
            ],
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
    /// Caída radial de una tormenta: 1 en el ojo, 0 fuera del radio.
    pub fn influencia(&self, p: Vec3, t: &Tormenta) -> f32 {
        let c = t.centro.normalize();
        let d = saturate(p.dot(c)).min(1.0).acos();
        1.0 - smoothstep(0.0, t.radio, d)
    }

    /// Aplica el remolino de todas las tormentas al punto de muestreo.
    pub fn arremolinar(&self, p: Vec3, time: f32) -> Vec3 {
        let mut q = p;
        for t in &self.tormentas {
            let w = self.influencia(p, t);
            if w <= 0.0 {
                continue;
            }
            let eje = t.centro.normalize();
            let ang = t.sentido * self.fuerza_tormenta * (w * w * t.fuerza + time * 0.55 * w);
            q = rotate_axis(q, eje, ang);
        }
        q
    }

    /// Patrón de bandas en [0, 1] ya deformado, arremolinado y con cizalla.
    pub fn patron(&self, p: Vec3, time: f32) -> f32 {
        let q = self.arremolinar(p, time);
        // Cizalla: la velocidad angular depende de la latitud.
        let vel = (q.y * self.frecuencia_bandas * 0.5).sin() * self.cizalla + 0.18;
        let q = rotate_y(q, time * vel);

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
        "bandas líquidas por deformación de dominio y una gran tormenta"
    }

    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3 {
        let p = f.dir;
        let t = u.time * self.velocidad;

        let v = self.patron(p, t);
        let mut color = ramp(&PALETA, v);

        // La gran tormenta pinta encima, con su propia textura y un anillo claro.
        let principal = self.tormentas[0];
        let w = self.influencia(p, &principal);
        if w > 0.0 {
            let eje = principal.centro.normalize();
            let giro = rotate_axis(p, eje, w * w * principal.fuerza + t * 0.55 * w);
            let tex = self.vortice_n.fbm(giro * 9.0, 4, 2.1, 0.5) * 0.5 + 0.5;
            let cuerpo = mix(v3(0.72, 0.26, 0.14), v3(0.97, 0.70, 0.46), tex);
            color = mix(color, cuerpo, smoothstep(0.10, 0.70, w) * 0.88);
            // Anillo de nubes altas en el borde del ciclón.
            let anillo = smoothstep(0.52, 0.66, w) * (1.0 - smoothstep(0.66, 0.82, w));
            color = mix(color, v3(1.0, 0.95, 0.88), anillo * 0.55);
        }

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
    use crate::math::{tangent_frame, v3};

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
            g.tormentas.clear();
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
    fn la_tormenta_curva_el_patron_a_su_alrededor() {
        let g = GasGiant::default();
        let mut sin_tormentas = GasGiant::default();
        sin_tormentas.tormentas.clear();
        let centro = g.tormentas[0].centro.normalize();
        let (t, b) = tangent_frame(centro);

        // Puntos en el anillo intermedio del vórtice: ahí la distorsión es máxima.
        let mut diferentes = 0;
        for i in 0..40 {
            let a = i as f32 * 0.157;
            let off = (t * a.cos() + b * a.sin()) * 0.30;
            let p = (centro + off).normalize();
            if (g.patron(p, 0.0) - sin_tormentas.patron(p, 0.0)).abs() > 0.05 {
                diferentes += 1;
            }
        }
        assert!(diferentes > 20, "la tormenta no curvó las bandas: {diferentes}/40");
    }

    #[test]
    fn lejos_de_la_tormenta_el_patron_no_cambia() {
        let g = GasGiant::default();
        let mut sin_tormentas = GasGiant::default();
        sin_tormentas.tormentas.clear();
        let lejos = (-g.tormentas[0].centro).normalize();
        // Se elige un punto alejado de todas las tormentas.
        let p = if g.tormentas.iter().all(|t| g.influencia(lejos, t) <= 0.0) {
            lejos
        } else {
            v3(0.0, 1.0, 0.0)
        };
        assert!((g.patron(p, 0.0) - sin_tormentas.patron(p, 0.0)).abs() < 1e-5);
    }

    #[test]
    fn la_deformacion_hace_que_las_bandas_dejen_de_ser_rectas() {
        let recto = {
            let mut g = GasGiant::default();
            g.deformacion = 0.0;
            g.tormentas.clear();
            g
        };
        let fluido = {
            let mut g = GasGiant::default();
            g.deformacion = 0.45;
            g.tormentas.clear();
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
