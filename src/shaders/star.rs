//! BRASA — estrella.
//!
//! Capas, en orden:
//!   1. Granulación convectiva: ruido celular (Worley) con F2-F1, que dibuja las
//!      paredes frías entre celdas de plasma.
//!   2. Plasma turbulento: fBm sobre un dominio deformado que avanza con el tiempo,
//!      más ruido ridged que produce las fibras brillantes (fáculas).
//!   3. Manchas frías: segundo campo celular umbralado, con penumbra suave.
//!   4. Todo se suma en una "temperatura" 0..1 que entra a una rampa de color
//!      tipo cuerpo negro (rojo oscuro → naranja → amarillo → blanco).
//!   5. Corona: término de Fresnel que enciende el borde del disco.
//!
//! La estrella es emisiva: ignora la iluminación externa del renderer a propósito.

use super::{ramp, Fragment, Shader, Uniforms};
use crate::math::{mix, rotate_y, saturate, smoothstep, v3, Vec3};
use crate::noise::Noise;

pub struct Star {
    plasma: Noise,
    granos: Noise,
    manchas: Noise,
    /// Frecuencia base de la turbulencia del plasma. Más alto = detalle más fino.
    pub turbulencia: f32,
    /// Velocidad con la que hierve la superficie.
    pub velocidad: f32,
    /// Umbral de las manchas frías: más alto = manchas más grandes y numerosas.
    pub cobertura_manchas: f32,
    /// Intensidad del halo incandescente del borde.
    pub brillo_corona: f32,
    /// Tamaño de las celdas de granulación.
    pub escala_granos: f32,
}

impl Default for Star {
    fn default() -> Star {
        Star {
            plasma: Noise::new(101),
            granos: Noise::new(202),
            manchas: Noise::new(303),
            turbulencia: 2.6,
            velocidad: 0.22,
            cobertura_manchas: 0.16,
            brillo_corona: 0.85,
            escala_granos: 13.0,
        }
    }
}

const PALETA: [Vec3; 6] = [
    v3(0.12, 0.018, 0.022), // núcleo de mancha
    v3(0.36, 0.050, 0.020), // penumbra
    v3(0.80, 0.180, 0.030), // rojo incandescente
    v3(0.99, 0.470, 0.060), // naranja
    v3(1.00, 0.800, 0.260), // amarillo
    v3(1.00, 0.985, 0.870), // blanco cálido
];

impl Star {
    /// Granulación: 1 dentro de una celda de convección, 0 en la pared fría.
    pub fn granulacion(&self, p: Vec3, t: f32) -> f32 {
        let q = rotate_y(p, t * 0.035) * self.escala_granos;
        let (f1, f2) = self.granos.worley(q);
        smoothstep(0.0, 0.30, f2 - f1)
    }

    /// Temperatura normalizada de la superficie en [0, 1].
    pub fn temperatura(&self, p: Vec3, t: f32) -> f32 {
        // Deformación de dominio que se desplaza: es lo que hace "hervir" el plasma.
        let flujo = v3(0.0, t * 0.12, t * 0.30);
        let w = self
            .plasma
            .warp(p * self.turbulencia + flujo, 0.55, 1.0, 3);

        let base = self.plasma.fbm(w, 4, 2.0, 0.5) * 0.5 + 0.5;
        let fibras = self.plasma.ridged(w * 2.1, 5, 2.15, 0.55);

        let granos = self.granulacion(p, t);

        // Manchas frías: celdas grandes, con penumbra por el borde.
        let (m1, _) = self.manchas.worley(rotate_y(p, t * 0.02) * 3.1);
        let umbra = 1.0 - smoothstep(self.cobertura_manchas, self.cobertura_manchas + 0.10, m1);
        let penumbra =
            1.0 - smoothstep(self.cobertura_manchas + 0.06, self.cobertura_manchas + 0.26, m1);

        let mut temp = base * 0.52 + fibras * 0.48;
        temp *= 0.58 + 0.42 * granos; // las paredes de los gránulos son más frías
        temp += 0.30 * fibras.powi(3); // fáculas brillantes
        temp += 0.14 * smoothstep(0.56, 0.92, base); // playas de plasma caliente
        temp -= 0.26 * penumbra + 0.44 * umbra;
        saturate(temp).powf(0.82)
    }
}

impl Shader for Star {
    fn nombre(&self) -> &'static str {
        "Brasa (estrella)"
    }

    fn descripcion(&self) -> &'static str {
        "plasma turbulento, granulación y manchas frías; emisiva con corona"
    }

    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3 {
        let t = u.time * self.velocidad;
        let temp = self.temperatura(f.dir, t);

        let mut color = ramp(&PALETA, temp);

        // El plasma más caliente tira a blanco azulado.
        color = mix(color, v3(1.0, 0.98, 0.95), smoothstep(0.86, 1.0, temp) * 0.6);

        // Corona: el borde del disco se enciende en vez de oscurecerse.
        let borde = saturate(1.0 - saturate(f.normal.dot(f.view_dir)));
        let corona = borde.powf(2.4) * self.brillo_corona;
        color += v3(1.00, 0.42, 0.12) * corona;
        color += v3(0.45, 0.14, 0.03) * borde.powf(6.0) * self.brillo_corona;

        // Latido global muy leve.
        let pulso = 1.0 + 0.035 * (u.time * 0.9).sin();
        color * pulso
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
            view_dir: v3(0.0, 0.0, 1.0),
            light_dir: v3(1.0, 0.0, 0.0),
            intensity: 0.0,
            }
    }

    #[test]
    fn la_estrella_brilla_aunque_no_haya_luz() {
        let s = Star::default();
        let f = frag(v3(0.0, 0.0, 1.0));
        let c = s.shade(&f, &Uniforms { time: 0.0 });
        assert!(
            c.x > 0.2,
            "con intensity = 0 la estrella debe seguir emitiendo: {c:?}"
        );
    }

    #[test]
    fn el_material_cambia_con_el_tiempo() {
        let s = Star::default();
        let f = frag(v3(0.3, 0.5, 0.8));
        let a = s.shade(&f, &Uniforms { time: 0.0 });
        let b = s.shade(&f, &Uniforms { time: 6.0 });
        assert!(
            (a - b).length() > 0.02,
            "el plasma debería evolucionar: {a:?} vs {b:?}"
        );
    }

    #[test]
    fn la_temperatura_se_mantiene_en_rango_y_varia() {
        let s = Star::default();
        let mut min = 1.0f32;
        let mut max = 0.0f32;
        for i in 0..800 {
            let a = i as f32 * 0.21;
            let p = v3(a.sin(), (a * 0.7).cos(), (a * 1.3).sin()).normalize();
            let v = s.temperatura(p, 1.0);
            assert!((0.0..=1.0).contains(&v));
            min = min.min(v);
            max = max.max(v);
        }
        assert!(max - min > 0.35, "la superficie debería tener contraste");
    }

    #[test]
    fn hay_manchas_frias_cuando_sube_la_cobertura() {
        let mut s = Star::default();
        s.cobertura_manchas = 0.0;
        let sin_manchas: f32 = muestreo_medio(&s);
        s.cobertura_manchas = 0.40;
        let con_manchas: f32 = muestreo_medio(&s);
        assert!(
            con_manchas < sin_manchas - 0.03,
            "subir la cobertura debe oscurecer la estrella: {con_manchas} vs {sin_manchas}"
        );
    }

    #[test]
    fn la_corona_enciende_el_borde() {
        let s = Star::default();
        let dir = v3(0.0, 0.0, 1.0);
        let mut centro = frag(dir);
        centro.normal = v3(0.0, 0.0, 1.0);
        let mut borde = frag(dir);
        borde.normal = v3(0.98, 0.0, 0.2).normalize();
        let u = Uniforms { time: 0.0 };
        assert!(s.shade(&borde, &u).x >= s.shade(&centro, &u).x);
    }

    fn muestreo_medio(s: &Star) -> f32 {
        let mut acc = 0.0;
        let n = 500;
        for i in 0..n {
            let a = i as f32 * 0.37;
            let p = v3(a.sin(), (a * 0.63).cos(), (a * 1.11).sin()).normalize();
            acc += s.temperatura(p, 0.0);
        }
        acc / n as f32
    }
}
