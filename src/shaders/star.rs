//! BRASA — estrella.
//!
//! Primera capa: el plasma. Un fBm sobre un dominio deformado que avanza con el
//! tiempo da la masa caliente, y un ruido ridged sobre el mismo dominio dibuja
//! las fibras brillantes. Todo se suma en una "temperatura" 0..1 que entra a una
//! rampa de color tipo cuerpo negro.
//!
//! La estrella es emisiva: ignora la iluminación externa del renderer a propósito.

use super::{ramp, Fragment, Shader, Uniforms};
use crate::math::{mix, saturate, smoothstep, v3, Vec3};
use crate::noise::Noise;

pub struct Star {
    plasma: Noise,
    /// Frecuencia base de la turbulencia del plasma. Más alto = detalle más fino.
    pub turbulencia: f32,
    /// Velocidad con la que hierve la superficie.
    pub velocidad: f32,
}

impl Default for Star {
    fn default() -> Star {
        Star {
            plasma: Noise::new(101),
            turbulencia: 2.6,
            velocidad: 0.22,
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
    /// Temperatura normalizada de la superficie en [0, 1].
    pub fn temperatura(&self, p: Vec3, t: f32) -> f32 {
        // Deformación de dominio que se desplaza: es lo que hace "hervir" el plasma.
        let flujo = v3(0.0, t * 0.12, t * 0.30);
        let w = self
            .plasma
            .warp(p * self.turbulencia + flujo, 0.55, 1.0, 3);

        let base = self.plasma.fbm(w, 4, 2.0, 0.5) * 0.5 + 0.5;
        let fibras = self.plasma.ridged(w * 2.1, 5, 2.15, 0.55);

        let mut temp = base * 0.52 + fibras * 0.48;
        temp += 0.30 * fibras.powi(3); // fáculas brillantes
        temp += 0.14 * smoothstep(0.56, 0.92, base); // playas de plasma caliente
        saturate(temp).powf(0.82)
    }
}

impl Shader for Star {
    fn nombre(&self) -> &'static str {
        "Brasa (estrella)"
    }

    fn descripcion(&self) -> &'static str {
        "plasma turbulento, emisivo"
    }

    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3 {
        let t = u.time * self.velocidad;
        let temp = self.temperatura(f.dir, t);

        let mut color = ramp(&PALETA, temp);

        // El plasma más caliente tira a blanco azulado.
        color = mix(color, v3(1.0, 0.98, 0.95), smoothstep(0.86, 1.0, temp) * 0.6);

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
}
