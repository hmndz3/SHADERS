//! Cargador de OBJ. No modifica ni agrega vértices: solo triangula los polígonos
//! que vengan con más de tres lados.

use crate::math::{v3, Vec3};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    /// Índices de triángulos (grupos de 3).
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

}

fn parse_index(token: &str, len: usize) -> Option<usize> {
    let raw: i64 = token.parse().ok()?;
    if raw > 0 {
        Some(raw as usize - 1)
    } else if raw < 0 {
        let i = len as i64 + raw;
        if i >= 0 {
            Some(i as usize)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn load_from_str(data: &str) -> Result<Mesh, String> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();

    for (ln, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let nums: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                if nums.len() < 3 {
                    return Err(format!("vértice inválido en la línea {}", ln + 1));
                }
                positions.push(v3(nums[0], nums[1], nums[2]));
            }
            Some("f") => {
                let mut face = Vec::new();
                for tok in it {
                    let vtok = tok.split('/').next().unwrap_or("");
                    match parse_index(vtok, positions.len()) {
                        Some(i) if i < positions.len() => face.push(i),
                        _ => return Err(format!("índice de cara inválido en la línea {}", ln + 1)),
                    }
                }
                if face.len() >= 3 {
                    faces.push(face);
                }
            }
            _ => {}
        }
    }

    if positions.is_empty() || faces.is_empty() {
        return Err("el OBJ no contiene geometría utilizable".to_string());
    }

    // Triangulación en abanico de cada polígono.
    let mut indices: Vec<u32> = Vec::new();
    for face in &faces {
        for i in 1..face.len() - 1 {
            indices.push(face[0] as u32);
            indices.push(face[i] as u32);
            indices.push(face[i + 1] as u32);
        }
    }

    // Normal provisional: la dirección desde el centro del modelo.
    let vertices = positions
        .iter()
        .map(|&p| Vertex {
            position: p,
            normal: p.normalize(),
        })
        .collect();

    Ok(Mesh { vertices, indices })
}

pub fn load<P: AsRef<Path>>(path: P) -> Result<Mesh, String> {
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("no se pudo leer {}: {e}", path.as_ref().display()))?;
    load_from_str(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TETRA: &str = "\
v 0 0 0
v 1 0 0
v 0 1 0
v 0 0 1
f 1 3 2
f 1 2 4
f 1 4 3
f 2 3 4
";

    #[test]
    fn carga_posiciones_y_caras() {
        let m = load_from_str(TETRA).unwrap();
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.triangle_count(), 4);
    }

    #[test]
    fn triangula_un_cuadrilatero() {
        let quad = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n";
        let m = load_from_str(quad).unwrap();
        assert_eq!(m.triangle_count(), 2);
    }

    #[test]
    fn acepta_el_formato_v_vt_vn() {
        let s = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1\n";
        let m = load_from_str(s).unwrap();
        assert_eq!(m.triangle_count(), 1);
    }

    #[test]
    fn rechaza_un_obj_vacio() {
        assert!(load_from_str("# nada\n").is_err());
    }

}
