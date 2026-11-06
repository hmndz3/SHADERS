//! Álgebra mínima para el renderer: vectores, matrices y utilidades de interpolación.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = v3(1.0, 1.0, 1.0);

    pub fn splat(v: f32) -> Vec3 {
        v3(v, v, v)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l > 1e-12 {
            self / l
        } else {
            Vec3::ZERO
        }
    }

    pub fn min_elem(self, o: Vec3) -> Vec3 {
        v3(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }

    pub fn max_elem(self, o: Vec3) -> Vec3 {
        v3(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}

impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        v3(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// Matriz 4x4 en orden fila-mayor; multiplica columnas (`m * v`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += self.0[i][k] * o.0[k][j];
                }
                r[i][j] = s;
            }
        }
        Mat4(r)
    }

    /// Transforma un punto homogéneo (w = 1) y devuelve (x, y, z, w) sin dividir.
    pub fn transform_point(&self, p: Vec3) -> [f32; 4] {
        let m = &self.0;
        [
            m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
            m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
            m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
            m[3][0] * p.x + m[3][1] * p.y + m[3][2] * p.z + m[3][3],
        ]
    }

    pub fn look_at(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
        let f = (center - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);
        Mat4([
            [s.x, s.y, s.z, -s.dot(eye)],
            [u.x, u.y, u.z, -u.dot(eye)],
            [-f.x, -f.y, -f.z, f.dot(eye)],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Proyección en perspectiva con z de cámara negativa hacia adelante.
    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let t = 1.0 / (fov_y_rad * 0.5).tan();
        Mat4([
            [t / aspect, 0.0, 0.0, 0.0],
            [0.0, t, 0.0, 0.0],
            [
                0.0,
                0.0,
                (far + near) / (near - far),
                2.0 * far * near / (near - far),
            ],
            [0.0, 0.0, -1.0, 0.0],
        ])
    }

    /// Lleva coordenadas normalizadas [-1, 1] a píxeles de pantalla.
    pub fn viewport(width: f32, height: f32) -> Mat4 {
        Mat4([
            [width * 0.5, 0.0, 0.0, width * 0.5],
            [0.0, -height * 0.5, 0.0, height * 0.5],
            [0.0, 0.0, 0.5, 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
}

pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

pub fn saturate(x: f32) -> f32 {
    clamp(x, 0.0, 1.0)
}

pub fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// Transición suave tipo Hermite entre `edge0` y `edge1`.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-9 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = saturate((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Rota un punto alrededor del eje Y (se usa para desplazar patrones en longitud).
pub fn rotate_y(p: Vec3, ang: f32) -> Vec3 {
    let (s, c) = ang.sin_cos();
    v3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z)
}

/// Rota `p` alrededor de un eje unitario cualquiera (fórmula de Rodrigues).
pub fn rotate_axis(p: Vec3, axis: Vec3, ang: f32) -> Vec3 {
    let (s, c) = ang.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

/// Base tangente estable sobre una esfera, a partir de la normal.
pub fn tangent_frame(n: Vec3) -> (Vec3, Vec3) {
    let up = if n.y.abs() > 0.99 {
        v3(1.0, 0.0, 0.0)
    } else {
        v3(0.0, 1.0, 0.0)
    };
    let t = up.cross(n).normalize();
    let b = n.cross(t);
    (t, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producto_cruz_y_punto() {
        let a = v3(1.0, 0.0, 0.0);
        let b = v3(0.0, 1.0, 0.0);
        assert_eq!(a.cross(b), v3(0.0, 0.0, 1.0));
        assert_eq!(a.dot(b), 0.0);
    }

    #[test]
    fn normalizar_da_longitud_uno() {
        let n = v3(3.0, 4.0, 12.0).normalize();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert_eq!(Vec3::ZERO.normalize(), Vec3::ZERO);
    }

    #[test]
    fn smoothstep_en_los_extremos() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn look_at_pone_la_camara_en_el_origen() {
        let m = Mat4::look_at(v3(0.0, 0.0, 5.0), Vec3::ZERO, v3(0.0, 1.0, 0.0));
        let p = m.transform_point(v3(0.0, 0.0, 5.0));
        assert!(p[0].abs() < 1e-5 && p[1].abs() < 1e-5 && p[2].abs() < 1e-5);
    }

    #[test]
    fn perspectiva_mantiene_el_centro() {
        let m = Mat4::perspective(1.0, 1.0, 0.1, 100.0);
        let p = m.transform_point(v3(0.0, 0.0, -10.0));
        assert!(p[0].abs() < 1e-6 && p[1].abs() < 1e-6);
        assert!(p[3] > 0.0, "w debe ser positivo delante de la cámara");
    }

    #[test]
    fn viewport_mapea_las_esquinas() {
        let m = Mat4::viewport(800.0, 600.0);
        let c = m.transform_point(v3(0.0, 0.0, 0.0));
        assert!((c[0] - 400.0).abs() < 1e-4 && (c[1] - 300.0).abs() < 1e-4);
        let tl = m.transform_point(v3(-1.0, 1.0, 0.0));
        assert!(tl[0].abs() < 1e-4 && tl[1].abs() < 1e-4);
    }

    #[test]
    fn rotar_en_y_conserva_la_longitud() {
        let p = v3(0.3, -0.7, 0.2);
        let r = rotate_y(p, 1.234);
        assert!((r.length() - p.length()).abs() < 1e-6);
        assert!((r.y - p.y).abs() < 1e-6);
    }

    #[test]
    fn rotar_alrededor_del_eje_fija_el_propio_eje() {
        let axis = v3(0.2, 0.9, -0.3).normalize();
        let r = rotate_axis(axis, axis, 2.0);
        assert!((r - axis).length() < 1e-5);
    }

    #[test]
    fn base_tangente_es_ortonormal() {
        for n in [v3(0.0, 1.0, 0.0), v3(0.3, 0.2, -0.9).normalize()] {
            let (t, b) = tangent_frame(n);
            assert!(t.dot(n).abs() < 1e-5);
            assert!(b.dot(n).abs() < 1e-5);
            assert!(t.dot(b).abs() < 1e-5);
            assert!((t.length() - 1.0).abs() < 1e-5);
        }
    }
}
