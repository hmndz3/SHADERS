//! TERRALBA — planeta rocoso.
//!
//! Cadena de cálculo de un fragmento:
//!   1. ALTURA: fBm con deformación de dominio para los continentes + ruido ridged
//!      para las cordilleras + detalle fino.
//!   2. REGIONES: océano profundo / plataforma / playa / desierto / pradera / bosque
//!      / roca / nieve / casquete polar, por altura, latitud y un campo de humedad.
//!   3. BUMP MAPPING: se perturba la normal con el gradiente del campo de altura.
//!   4. ILUMINACIÓN DIFERENCIADA: el océano responde con especular duro + Fresnel,
//!      la tierra es mate, la nieve tiene un brillo ancho.
//!   5. ATMÓSFERA: borde azul por Fresnel y rojo en el terminador.

use super::{
    blinn_phong, fresnel, mix, perturb_normal_fwd, wrap_diffuse, Fragment, Shader, Uniforms,
};
use crate::math::{saturate, smoothstep, v3, Vec3};
use crate::noise::Noise;

pub struct Rocky {
    relieve: Noise,
    detalle: Noise,
    humedad: Noise,
    /// Altura que separa el agua de la tierra. Subirlo inunda los continentes.
    pub nivel_mar: f32,
    /// Escala de los continentes. Más alto = continentes más chicos y numerosos.
    pub escala_continentes: f32,
    /// Fuerza del bump mapping sobre el terreno.
    pub fuerza_relieve: f32,
    /// Intensidad del reflejo especular del océano. En 0 el agua queda mate,
    /// igual que la tierra.
    pub brillo_oceano: f32,
}

impl Default for Rocky {
    fn default() -> Rocky {
        Rocky {
            relieve: Noise::new(11),
            detalle: Noise::new(22),
            humedad: Noise::new(33),
            nivel_mar: 0.50,
            escala_continentes: 1.40,
            fuerza_relieve: 0.30,
            brillo_oceano: 11.0,
        }
    }
}

// Paleta del planeta.
const AGUA_PROFUNDA: Vec3 = v3(0.020, 0.080, 0.210);
const AGUA_MEDIA: Vec3 = v3(0.050, 0.205, 0.375);
const AGUA_COSTA: Vec3 = v3(0.130, 0.470, 0.530);
const PLAYA: Vec3 = v3(0.800, 0.725, 0.500);
const DESIERTO: Vec3 = v3(0.780, 0.600, 0.330);
const PRADERA: Vec3 = v3(0.400, 0.520, 0.240);
const BOSQUE: Vec3 = v3(0.155, 0.340, 0.165);
const ROCA: Vec3 = v3(0.400, 0.365, 0.320);
const NIEVE: Vec3 = v3(0.945, 0.960, 0.980);

impl Rocky {
    /// Campo de altura procedural. Devuelve valores alrededor de [0, 1.2].
    pub fn altura(&self, p: Vec3) -> f32 {
        let w = self
            .relieve
            .warp(p * self.escala_continentes, 0.28, 1.1, 1);
        let continentes = self.relieve.fbm(w, 4, 2.05, 0.45) * 0.5 + 0.5;
        let tierra = smoothstep(self.nivel_mar - 0.02, self.nivel_mar + 0.14, continentes);
        let cordilleras = self.relieve.ridged(w * 2.6, 3, 2.3, 0.50);
        let fino = self.detalle.fbm(p * 8.0, 2, 2.0, 0.5) * 0.5 + 0.5;
        continentes + 0.16 * cordilleras * tierra + 0.025 * fino * tierra
    }

    /// Humedad en [0, 1]: decide desierto contra bosque.
    pub fn humedad(&self, p: Vec3) -> f32 {
        saturate(self.humedad.fbm(p * 2.4 + v3(17.0, 3.0, -9.0), 3, 2.1, 0.5) * 0.5 + 0.5)
    }

}

impl Shader for Rocky {
    fn nombre(&self) -> &'static str {
        "Terralba (planeta rocoso)"
    }

    fn descripcion(&self) -> &'static str {
        "continentes, biomas y relieve por bump mapping"
    }

    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3 {
        let n_geo = f.normal;
        let l = f.light_dir;
        let v = f.view_dir;

        let p = f.dir;
        let h = self.altura(p);

        // 1. Clasificación de la superficie.
        let lat = p.y.abs();
        let es_tierra = smoothstep(self.nivel_mar - 0.004, self.nivel_mar + 0.004, h);
        let altura_rel = saturate((h - self.nivel_mar) / 0.42);

        // Casquetes polares: borde irregular, también sobre el agua.
        let borde_polar = lat + 0.085 * self.detalle.fbm(p * 5.0, 2, 2.0, 0.5);
        let hielo = smoothstep(0.78, 0.88, borde_polar);

        // Océano: tres profundidades.
        let color_agua = if es_tierra < 1.0 {
            let prof = smoothstep(self.nivel_mar, self.nivel_mar - 0.18, h);
            mix(
                mix(AGUA_COSTA, AGUA_MEDIA, smoothstep(0.0, 0.35, prof)),
                AGUA_PROFUNDA,
                smoothstep(0.35, 1.0, prof),
            )
        } else {
            AGUA_PROFUNDA
        };

        // Tierra: bioma por humedad y altura.
        let color_tierra = if es_tierra > 0.0 {
            let hum = self.humedad(p);
            let seco = mix(DESIERTO, PRADERA, smoothstep(0.36, 0.52, hum));
            let bioma = mix(seco, BOSQUE, smoothstep(0.50, 0.68, hum));
            let con_playa = mix(PLAYA, bioma, smoothstep(0.004, 0.030, altura_rel));
            let con_roca = mix(con_playa, ROCA, smoothstep(0.46, 0.72, altura_rel));
            // La nieve pide altura y latitud a la vez: no hay cumbres blancas en
            // el ecuador.
            let nieve_alta = smoothstep(0.86, 1.00, altura_rel * 0.75 + lat * 0.55);
            mix(con_roca, NIEVE, nieve_alta)
        } else {
            PRADERA
        };

        let mut albedo = mix(color_agua, color_tierra, es_tierra);
        albedo = mix(albedo, NIEVE, hielo);

        // 2. BUMP MAPPING. El gradiente del campo de altura inclina la normal.
        //    En el agua se usa un microoleaje de alta frecuencia, mucho más suave.
        let relieve = self.fuerza_relieve * es_tierra * (1.0 - hielo * 0.6);
        let n_terreno = if relieve > 0.0 {
            perturb_normal_fwd(p, n_geo, h, relieve, 0.0035, |q| self.altura(q))
        } else {
            n_geo
        };
        let n = if es_tierra < 1.0 {
            let oleaje = |q: Vec3| {
                self.detalle
                    .fbm(q * 45.0 + v3(0.0, u.time * 0.25, u.time * 0.1), 2, 2.0, 0.5)
            };
            let n_agua = perturb_normal_fwd(
                p,
                n_geo,
                oleaje(p),
                0.0022 * (1.0 - es_tierra),
                0.004,
                oleaje,
            );
            mix(n_agua, n_terreno, es_tierra).normalize()
        } else {
            n_terreno
        };

        let ndotl = n.dot(l);

        // 3. RESPUESTAS DISTINTAS A LA LUZ según la región.
        //    Océano: difusa baja + especular duro con Fresnel.
        //    Tierra: mate, con wrap para un terminador suave.
        //    Nieve/hielo: especular ancho y débil.
        let dif_tierra = wrap_diffuse(ndotl, 0.14);
        let dif_agua = saturate(ndotl);

        let luz_tierra = albedo * (0.04 + 0.96 * dif_tierra);
        let mut color = if es_tierra < 1.0 || hielo > 0.0 {
            let spec_agua =
                blinn_phong(n, l, v, 70.0) * fresnel(n_geo, v, 0.03) * self.brillo_oceano;
            let spec_hielo = blinn_phong(n, l, v, 20.0) * 0.30;
            let luz_agua = albedo * (0.03 + 0.97 * dif_agua) + Vec3::ONE * spec_agua * dif_agua;
            let luz_hielo =
                albedo * (0.05 + 0.95 * dif_tierra) + Vec3::ONE * spec_hielo * dif_agua;
            mix(
                mix(luz_agua, luz_tierra, es_tierra * (1.0 - hielo)),
                luz_hielo,
                hielo,
            )
        } else {
            luz_tierra
        };

        // 4. ATMÓSFERA: dispersión azul en el borde y rojo en el terminador.
        let borde = saturate(1.0 - saturate(n_geo.dot(v)));
        let halo = borde.powf(3.2);
        // Para la atmósfera se usa la intensidad difusa que entrega el renderer
        // (normal geométrica), no la normal perturbada por el relieve.
        color += v3(0.26, 0.47, 0.98) * halo * (0.20 + 0.80 * f.intensity);
        let terminador = smoothstep(0.32, 0.0, ndotl.abs()) * halo;
        color += v3(0.85, 0.32, 0.10) * terminador * 0.55;

        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{tangent_frame, v3};

    fn frag(dir: Vec3, light: Vec3, view: Vec3) -> Fragment {
        let n = dir.normalize();
        Fragment {
            dir: n,
            normal: n,
            view_dir: view.normalize(),
            light_dir: light.normalize(),
            intensity: saturate(n.dot(light.normalize())),
        }
    }

    /// Puntos repartidos de forma pareja sobre la esfera (espiral de Fibonacci).
    fn direcciones(n: usize) -> Vec<Vec3> {
        let phi = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
        (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let a = phi * i as f32;
                v3(a.cos() * r, y, a.sin() * r)
            })
            .collect()
    }

    /// Busca una dirección que sea tierra y otra que sea agua.
    fn puntos_tierra_y_agua(r: &Rocky) -> (Vec3, Vec3) {
        let mut tierra = None;
        let mut agua = None;
        for p in direcciones(4000) {
            let h = r.altura(p);
            let altura_rel = saturate((h - r.nivel_mar) / 0.42);
            let apto = (0.05..0.35).contains(&altura_rel) && p.y.abs() < 0.5;
            if apto && tierra.is_none() {
                tierra = Some(p);
            }
            if h < r.nivel_mar - 0.10 && p.y.abs() < 0.5 && agua.is_none() {
                agua = Some(p);
            }
            if tierra.is_some() && agua.is_some() {
                break;
            }
        }
        (
            tierra.expect("debería haber continentes"),
            agua.expect("debería haber océanos"),
        )
    }

    #[test]
    fn hay_continentes_y_oceanos_en_proporcion_razonable() {
        let r = Rocky::default();
        let mut tierra = 0;
        let total = 3000;
        for i in 0..total {
            let a = i as f32 * 0.211;
            let p = v3(a.sin(), (a * 0.53).sin(), (a * 0.77).cos()).normalize();
            if r.altura(p) > r.nivel_mar {
                tierra += 1;
            }
        }
        let frac = tierra as f32 / total as f32;
        assert!(
            (0.15..0.75).contains(&frac),
            "fracción de tierra fuera de rango: {frac}"
        );
    }

    #[test]
    fn subir_el_nivel_del_mar_inunda_el_planeta() {
        let mut r = Rocky::default();
        let cuenta = |r: &Rocky| {
            (0..1500)
                .filter(|i| {
                    let a = *i as f32 * 0.211;
                    let p = v3(a.sin(), (a * 0.53).sin(), (a * 0.77).cos()).normalize();
                    r.altura(p) > r.nivel_mar
                })
                .count()
        };
        let antes = cuenta(&r);
        r.nivel_mar = 0.70;
        assert!(cuenta(&r) < antes, "más mar debería dejar menos tierra");
    }

    #[test]
    fn el_oceano_y_la_tierra_tienen_colores_distintos() {
        let r = Rocky::default();
        let (t, a) = puntos_tierra_y_agua(&r);
        let u = Uniforms { time: 0.0 };
        let ct = r.shade(&frag(t, t, t), &u);
        let ca = r.shade(&frag(a, a, a), &u);
        assert!((ct - ca).length() > 0.05, "{ct:?} vs {ca:?}");
    }

    /// Ganancia especular media: cuánto brillo gana la superficie cuando la
    /// cámara se pone justo en la dirección del reflejo. El agua debería ganar
    /// mucho; la tierra, nada.
    fn ganancia_especular_media(r: &Rocky, agua: bool) -> f32 {
        let u = Uniforms { time: 0.0 };
        let mut suma = 0.0;
        let mut n = 0;
        for p in direcciones(3000) {
            if p.y.abs() > 0.5 {
                continue; // fuera de los casquetes polares
            }
            let h = r.altura(p);
            let es_agua = h < r.nivel_mar - 0.05;
            if es_agua != agua || (!agua && h < r.nivel_mar + 0.05) {
                continue;
            }
            let (tg, _) = tangent_frame(p);
            let fuera_de_eje = (p * 0.766 + tg * 0.643).normalize(); // 40 grados
            let brillo = |v: Vec3| {
                let c = r.shade(&frag(p, p, v), &u);
                (c.x + c.y + c.z) / 3.0
            };
            suma += brillo(p) - brillo(fuera_de_eje);
            n += 1;
        }
        assert!(n > 100, "pocas muestras: {n}");
        suma / n as f32
    }

    #[test]
    fn el_oceano_responde_a_la_luz_distinto_que_la_tierra() {
        let r = Rocky::default();
        let agua = ganancia_especular_media(&r, true);
        let tierra = ganancia_especular_media(&r, false);
        assert!(agua > 0.03, "el océano debería reflejar: {agua}");
        assert!(
            agua > tierra + 0.03,
            "la tierra debería ser mate: agua {agua}, tierra {tierra}"
        );
    }

    #[test]
    fn apagar_el_brillo_del_oceano_lo_deja_mate() {
        let mut r = Rocky::default();
        let con = ganancia_especular_media(&r, true);
        r.brillo_oceano = 0.0;
        let sin = ganancia_especular_media(&r, true);
        assert!(con > sin + 0.03, "con {con}, sin {sin}");
    }

    #[test]
    fn el_bump_cambia_el_resultado() {
        let (t, _) = puntos_tierra_y_agua(&Rocky::default());
        let u = Uniforms { time: 0.0 };
        let luz = (t + v3(0.6, 0.2, 0.0)).normalize();
        let mut liso = Rocky::default();
        liso.fuerza_relieve = 0.0;
        let rugoso = Rocky::default();
        let f = frag(t, luz, t);
        let a = liso.shade(&f, &u);
        let b = rugoso.shade(&f, &u);
        assert!((a - b).length() > 0.005, "el relieve no afectó la luz: {a:?} vs {b:?}");
    }

    #[test]
    fn el_color_queda_en_rango_visible() {
        let r = Rocky::default();
        for i in 0..1500 {
            let a = i as f32 * 0.101;
            let p = v3(a.sin(), (a * 0.63).cos(), (a * 1.17).sin()).normalize();
            let luz = v3(0.4, 0.5, 0.76).normalize();
            let c = r.shade(&frag(p, luz, p), &Uniforms { time: a });
            assert!(c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0, "color negativo {c:?}");
            assert!(c.x < 4.0 && c.y < 4.0 && c.z < 4.0, "color desbordado {c:?}");
        }
    }
}
