//! Renderer por software: pasada de geometría, que rasteriza la malla con
//! z-buffer en un buffer intermedio de posiciones y normales.
//!
//! El modelo nunca se mueve: la que orbita es la cámara. Así el espacio objeto y el
//! espacio mundo coinciden y los patrones quedan fijos a la esfera.

use crate::framebuffer::background;
use crate::math::{v3, Mat4, Vec3};
use crate::noise::Noise;
use crate::obj::Mesh;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub distancia: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            distancia: 3.1,
            yaw: 0.6,
            pitch: 0.25,
            fov: std::f32::consts::FRAC_PI_4,
        }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        let cp = self.pitch.cos();
        v3(
            self.distancia * cp * self.yaw.sin(),
            self.distancia * self.pitch.sin(),
            self.distancia * cp * self.yaw.cos(),
        )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_at(self.eye(), Vec3::ZERO, v3(0.0, 1.0, 0.0));
        let proj = Mat4::perspective(self.fov, aspect, 0.1, 100.0);
        proj.mul(&view)
    }
}

/// Buffer intermedio: lo que el rasterizador deja listo para el shader.
pub struct Renderer {
    pub width: usize,
    pub height: usize,
    depth: Vec<f32>,
    covered: Vec<bool>,
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    /// El fondo estrellado no depende de la cámara ni del tiempo: se calcula una
    /// sola vez y se reutiliza en cada cuadro.
    fondo: Vec<Vec3>,
}

impl Renderer {
    pub fn new(width: usize, height: usize) -> Renderer {
        let n = width * height;
        let ruido = Noise::new(2024);
        let mut fondo = vec![Vec3::ZERO; n];
        fondo.par_chunks_mut(width).enumerate().for_each(|(y, fila)| {
            let fy = y as f32 / height as f32;
            for (x, px) in fila.iter_mut().enumerate() {
                *px = background(x as f32 / width as f32, fy, &ruido);
            }
        });
        Renderer {
            width,
            height,
            depth: vec![f32::INFINITY; n],
            covered: vec![false; n],
            positions: vec![Vec3::ZERO; n],
            normals: vec![Vec3::ZERO; n],
            fondo,
        }
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    fn clear(&mut self) {
        self.depth.iter_mut().for_each(|d| *d = f32::INFINITY);
        self.covered.iter_mut().for_each(|c| *c = false);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_covered(&self, x: usize, y: usize) -> bool {
        self.covered[y * self.width + x]
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn depth_at(&self, x: usize, y: usize) -> f32 {
        self.depth[y * self.width + x]
    }

    /// Pasada de geometría: transforma, descarta caras traseras, rasteriza con
    /// coordenadas baricéntricas e interpola posición y normal con corrección de
    /// perspectiva, resolviendo la visibilidad con z-buffer.
    pub fn geometry_pass(&mut self, mesh: &Mesh, camera: &Camera) {
        self.clear();
        let eye = camera.eye();
        let mvp = camera.view_proj(self.aspect());
        let viewport = Mat4::viewport(self.width as f32, self.height as f32);

        for tri in mesh.indices.chunks_exact(3) {
            let vs = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];

            // Descarte de caras traseras por geometría (independiente del orden
            // de los vértices en el OBJ).
            let centro = (vs[0].position + vs[1].position + vs[2].position) / 3.0;
            let normal_cara = (vs[1].position - vs[0].position)
                .cross(vs[2].position - vs[0].position)
                .normalize();
            let hacia_camara = (eye - centro).normalize();
            if normal_cara.dot(hacia_camara) <= 0.0 {
                continue;
            }

            let clip: [[f32; 4]; 3] = [
                mvp.transform_point(vs[0].position),
                mvp.transform_point(vs[1].position),
                mvp.transform_point(vs[2].position),
            ];
            if clip.iter().any(|c| c[3] <= 1e-4) {
                continue; // el triángulo cruza el plano cercano
            }

            let mut scr = [[0.0f32; 3]; 3];
            let mut inv_w = [0.0f32; 3];
            for i in 0..3 {
                let w = clip[i][3];
                inv_w[i] = 1.0 / w;
                let ndc = v3(clip[i][0] / w, clip[i][1] / w, clip[i][2] / w);
                let s = viewport.transform_point(ndc);
                scr[i] = [s[0], s[1], s[2]];
            }

            let area = (scr[1][0] - scr[0][0]) * (scr[2][1] - scr[0][1])
                - (scr[2][0] - scr[0][0]) * (scr[1][1] - scr[0][1]);
            if area.abs() < 1e-9 {
                continue;
            }
            let inv_area = 1.0 / area;

            let min_x = scr.iter().map(|s| s[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_x = (scr.iter().map(|s| s[0]).fold(f32::MIN, f32::max).ceil() as isize)
                .clamp(0, self.width as isize - 1) as usize;
            let min_y = scr.iter().map(|s| s[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_y = (scr.iter().map(|s| s[1]).fold(f32::MIN, f32::max).ceil() as isize)
                .clamp(0, self.height as isize - 1) as usize;
            if min_x > max_x || min_y > max_y {
                continue;
            }

            for y in min_y..=max_y {
                let py = y as f32 + 0.5;
                for x in min_x..=max_x {
                    let px = x as f32 + 0.5;
                    let l0 = ((scr[1][0] - px) * (scr[2][1] - py)
                        - (scr[2][0] - px) * (scr[1][1] - py))
                        * inv_area;
                    let l1 = ((scr[2][0] - px) * (scr[0][1] - py)
                        - (scr[0][0] - px) * (scr[2][1] - py))
                        * inv_area;
                    let l2 = 1.0 - l0 - l1;
                    if l0 < 0.0 || l1 < 0.0 || l2 < 0.0 {
                        continue;
                    }

                    let z = l0 * scr[0][2] + l1 * scr[1][2] + l2 * scr[2][2];
                    let idx = y * self.width + x;
                    if z >= self.depth[idx] {
                        continue;
                    }

                    // Corrección de perspectiva.
                    let b0 = l0 * inv_w[0];
                    let b1 = l1 * inv_w[1];
                    let b2 = l2 * inv_w[2];
                    let den = b0 + b1 + b2;
                    if den <= 0.0 {
                        continue;
                    }
                    let inv_den = 1.0 / den;
                    let pos = (vs[0].position * b0 + vs[1].position * b1 + vs[2].position * b2)
                        * inv_den;
                    let nor =
                        (vs[0].normal * b0 + vs[1].normal * b1 + vs[2].normal * b2) * inv_den;

                    self.depth[idx] = z;
                    self.covered[idx] = true;
                    self.positions[idx] = pos;
                    self.normals[idx] = nor.normalize();
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj;

    fn esfera() -> Mesh {
        let mut m = obj::load_from_str(include_str!("../assets/sphere.obj")).unwrap();
        m.fit_to_unit_sphere();
        m
    }

    #[test]
    fn la_esfera_cubre_el_centro_y_no_las_esquinas() {
        let mut r = Renderer::new(120, 120);
        r.geometry_pass(&esfera(), &Camera::default());
        assert!(r.is_covered(60, 60), "el centro debería estar cubierto");
        assert!(!r.is_covered(1, 1), "la esquina debería ser fondo");
        assert!(!r.is_covered(118, 118));
    }

    #[test]
    fn la_silueta_ocupa_una_fraccion_razonable_de_la_pantalla() {
        let mut r = Renderer::new(200, 200);
        r.geometry_pass(&esfera(), &Camera::default());
        let cubiertos = (0..200)
            .flat_map(|y| (0..200).map(move |x| (x, y)))
            .filter(|&(x, y)| r.is_covered(x, y))
            .count();
        let frac = cubiertos as f32 / 40000.0;
        assert!(
            (0.10..0.60).contains(&frac),
            "la esfera ocupa {frac} de la pantalla"
        );
    }

    #[test]
    fn el_zbuffer_se_queda_con_lo_mas_cercano() {
        let mut r = Renderer::new(64, 64);
        r.geometry_pass(&esfera(), &Camera::default());
        let cam = Camera::default();
        let eye = cam.eye();
        // El punto visible en el centro debe mirar hacia la cámara.
        let idx = 32 * 64 + 32;
        assert!(r.covered[idx]);
        let p = r.positions[idx];
        assert!(
            (eye - p).normalize().dot(r.normals[idx]) > 0.8,
            "la cara visible debería apuntar a la cámara"
        );
        // Y ser la mitad cercana de la esfera.
        assert!(p.dot(eye.normalize()) > 0.5);
    }

    #[test]
    fn las_posiciones_interpoladas_caen_sobre_la_esfera() {
        let mut r = Renderer::new(100, 100);
        r.geometry_pass(&esfera(), &Camera::default());
        for y in 0..100 {
            for x in 0..100 {
                if r.is_covered(x, y) {
                    let l = r.positions[y * 100 + x].length();
                    assert!((0.80..=1.01).contains(&l), "radio {l} fuera de la esfera");
                }
            }
        }
    }

    #[test]
    fn girar_la_camara_cambia_lo_que_se_ve() {
        let mesh = esfera();
        let mut r = Renderer::new(80, 80);
        let mut cam = Camera::default();
        r.geometry_pass(&mesh, &cam);
        let a = r.positions[40 * 80 + 40];
        cam.yaw += 1.2;
        r.geometry_pass(&mesh, &cam);
        let b = r.positions[40 * 80 + 40];
        assert!((a - b).length() > 0.3, "la cámara no se movió: {a:?} {b:?}");
    }

}
