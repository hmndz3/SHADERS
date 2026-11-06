//! Interfaz común de los fragment shaders y utilidades compartidas.

use crate::math::{mix, saturate, v3, Vec3};

/// Todo lo que el renderer entrega al shader para un píxel cubierto por la esfera.
/// Las direcciones están en espacio objeto (el modelo no se mueve, se mueve la cámara).
#[derive(Debug, Clone, Copy)]
pub struct Fragment {
    /// Dirección unitaria desde el centro: es la "coordenada esférica" del patrón.
    pub dir: Vec3,
    /// Normal suavizada e interpolada.
    pub normal: Vec3,
    /// De la superficie hacia la cámara.
    pub view_dir: Vec3,
    /// De la superficie hacia la luz.
    pub light_dir: Vec3,
    /// Difusa básica que calcula el renderer: max(N·L, 0).
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Uniforms {
    pub time: f32,
}

pub trait Shader: Sync + Send {
    /// Nombre del cuerpo celeste.
    fn nombre(&self) -> &'static str;
    /// Descripción corta para el título de la ventana.
    fn descripcion(&self) -> &'static str;
    /// Color final del fragmento.
    fn shade(&self, f: &Fragment, u: &Uniforms) -> Vec3;
}

/// Rampa de color por tramos uniformes. `t` se recorta a [0, 1].
pub fn ramp(colors: &[Vec3], t: f32) -> Vec3 {
    debug_assert!(colors.len() >= 2);
    let t = saturate(t) * (colors.len() - 1) as f32;
    let i = (t.floor() as usize).min(colors.len() - 2);
    mix(colors[i], colors[i + 1], t - i as f32)
}

/// Especular Blinn-Phong. `exponent` alto = brillo chico y duro.
pub fn blinn_phong(normal: Vec3, light: Vec3, view: Vec3, exponent: f32) -> f32 {
    let h = (light + view).normalize();
    saturate(normal.dot(h)).powf(exponent)
}

/// Término de Fresnel barato (Schlick) según el ángulo de vista.
pub fn fresnel(normal: Vec3, view: Vec3, f0: f32) -> f32 {
    let c = saturate(1.0 - saturate(normal.dot(view)));
    f0 + (1.0 - f0) * c.powi(5)
}

/// Difusa con "wrap": suaviza el terminador para que no corte en seco.
pub fn wrap_diffuse(ndotl: f32, wrap: f32) -> f32 {
    saturate((ndotl + wrap) / (1.0 + wrap))
}

/// Normal perturbada a partir del gradiente de un campo de altura 3D.
/// No toca la geometría: solo cambia la normal usada para iluminar (bump mapping).
/// Usa diferencias centradas: seis evaluaciones del campo.
#[cfg_attr(not(test), allow(dead_code))]
pub fn perturb_normal<F>(point: Vec3, normal: Vec3, strength: f32, eps: f32, height: F) -> Vec3
where
    F: Fn(Vec3) -> f32,
{
    let hx = height(point + v3(eps, 0.0, 0.0)) - height(point - v3(eps, 0.0, 0.0));
    let hy = height(point + v3(0.0, eps, 0.0)) - height(point - v3(0.0, eps, 0.0));
    let hz = height(point + v3(0.0, 0.0, eps)) - height(point - v3(0.0, 0.0, eps));
    let grad = v3(hx, hy, hz) / (2.0 * eps);
    // Solo la parte tangente a la superficie inclina la normal.
    let surf = grad - normal * grad.dot(normal);
    (normal - surf * strength).normalize()
}

/// Igual que `perturb_normal` pero con diferencias hacia adelante reutilizando la
/// altura ya calculada en el punto: tres evaluaciones en vez de seis. Es la versión
/// que usan los shaders en tiempo real.
pub fn perturb_normal_fwd<F>(
    point: Vec3,
    normal: Vec3,
    h0: f32,
    strength: f32,
    eps: f32,
    height: F,
) -> Vec3
where
    F: Fn(Vec3) -> f32,
{
    let grad = v3(
        height(point + v3(eps, 0.0, 0.0)) - h0,
        height(point + v3(0.0, eps, 0.0)) - h0,
        height(point + v3(0.0, 0.0, eps)) - h0,
    ) / eps;
    let surf = grad - normal * grad.dot(normal);
    (normal - surf * strength).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::v3;

    #[test]
    fn la_rampa_toca_los_extremos() {
        let c = [v3(0.0, 0.0, 0.0), v3(0.5, 0.5, 0.5), v3(1.0, 1.0, 1.0)];
        assert_eq!(ramp(&c, 0.0), c[0]);
        assert_eq!(ramp(&c, 1.0), c[2]);
        assert_eq!(ramp(&c, -3.0), c[0]);
        let medio = ramp(&c, 0.5);
        assert!((medio.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn el_especular_es_maximo_en_el_reflejo() {
        let n = v3(0.0, 1.0, 0.0);
        let l = v3(0.0, 1.0, 0.0);
        assert!((blinn_phong(n, l, l, 32.0) - 1.0).abs() < 1e-5);
        let rasante = v3(1.0, 0.05, 0.0).normalize();
        assert!(blinn_phong(n, l, rasante, 32.0) < 0.5);
    }

    #[test]
    fn fresnel_crece_en_el_borde() {
        let n = v3(0.0, 0.0, 1.0);
        let frente = fresnel(n, v3(0.0, 0.0, 1.0), 0.04);
        let borde = fresnel(n, v3(1.0, 0.0, 0.02).normalize(), 0.04);
        assert!(borde > frente);
        assert!(borde <= 1.0 && frente >= 0.04);
    }

    #[test]
    fn el_bump_inclina_la_normal_en_una_pendiente() {
        let n = v3(0.0, 0.0, 1.0);
        let p = v3(0.0, 0.0, 1.0);
        // Altura que crece con x: la normal debe inclinarse hacia -x.
        let perturbada = perturb_normal(p, n, 0.5, 1e-3, |q| q.x * 0.3);
        assert!(perturbada.x < -0.01, "normal sin inclinar: {perturbada:?}");
        assert!((perturbada.length() - 1.0).abs() < 1e-5);
        // Campo plano: la normal no cambia.
        let plana = perturb_normal(p, n, 0.5, 1e-3, |_| 1.0);
        assert!((plana - n).length() < 1e-5);
    }

    #[test]
    fn las_dos_versiones_del_bump_coinciden() {
        let n = v3(0.0, 0.0, 1.0);
        let p = v3(0.0, 0.0, 1.0);
        let campo = |q: Vec3| 0.2 * q.x + 0.1 * q.y;
        let centrada = perturb_normal(p, n, 0.5, 1e-3, campo);
        let adelante = perturb_normal_fwd(p, n, campo(p), 0.5, 1e-3, campo);
        assert!((centrada - adelante).length() < 1e-4);
    }

    #[test]
    fn wrap_diffuse_ilumina_un_poco_mas_alla_del_terminador() {
        assert!(wrap_diffuse(-0.05, 0.2) > 0.0);
        assert_eq!(wrap_diffuse(-0.5, 0.2), 0.0);
        assert!((wrap_diffuse(1.0, 0.2) - 1.0).abs() < 1e-6);
    }
}
