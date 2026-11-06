//! Generación de las capturas PNG del README, sin abrir ventana.

use crate::framebuffer::Framebuffer;
use crate::math::{v3, Vec3};
use crate::obj::Mesh;
use crate::render::{Camera, Renderer};
use crate::shaders::gas::GasGiant;
use crate::shaders::rocky::Rocky;
use crate::shaders::star::Star;
use crate::shaders::{Shader, Uniforms};
use std::path::Path;

/// Dibuja un cuadro con supermuestreo y lo devuelve ya reducido.
pub fn frame(
    mesh: &Mesh,
    shader: &dyn Shader,
    camera: &Camera,
    light: Vec3,
    time: f32,
    width: usize,
    height: usize,
    ss: usize,
) -> Framebuffer {
    let (w, h) = (width * ss, height * ss);
    let mut r = Renderer::new(w, h);
    let mut fb = Framebuffer::new(w, h);
    r.draw(
        &mut fb,
        mesh,
        shader,
        &Uniforms { time },
        camera,
        light,
    );
    fb.downsample(ss)
}

/// Centro de la gran tormenta del gigante gaseoso, visto de frente.
fn camara_hacia(dir: Vec3, distancia: f32) -> Camera {
    let d = dir.normalize();
    Camera {
        distancia,
        yaw: d.x.atan2(d.z),
        pitch: d.y.asin(),
        ..Camera::default()
    }
}

pub fn generar_todo(mesh: &Mesh, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;

    let estrella = Star::default();
    let rocoso = Rocky::default();
    let gas = GasGiant::default();

    let cam = Camera::default();
    let luz_frontal = v3(0.55, 0.35, 0.76);
    let luz_lateral = v3(-0.78, 0.18, 0.60);
    let luz_camara = cam.eye().normalize();

    let (pw, ph, pss) = (900usize, 700usize, 2usize);

    // --- Capturas fijas -----------------------------------------------------
    let fijas: Vec<(&str, &dyn Shader, Camera, Vec3, f32)> = vec![
        ("brasa.png", &estrella, cam, luz_frontal, 4.0),
        ("terralba.png", &rocoso, cam, luz_frontal, 0.0),
        ("ambar.png", &gas, camara_hacia(v3(0.1, 0.1, 1.0), 3.1), luz_frontal, 0.0),
        ("terralba_noche.png", &rocoso, cam, luz_lateral, 0.0),
        ("terralba_oceano.png", &rocoso, cam, luz_camara, 0.0),
        (
            "ambar_tormenta.png",
            &gas,
            camara_hacia(v3(0.62, -0.30, 0.72), 2.45),
            luz_frontal,
            0.0,
        ),
    ];
    for (nombre, sh, c, luz, t) in fijas {
        println!("  capturando {nombre}");
        frame(mesh, sh, &c, luz, t, pw, ph, pss).save_png(dir.join(nombre))?;
    }

    // Comparación del parallax: misma cámara, con y sin desplazamiento.
    let mut sin_parallax = Rocky::default();
    sin_parallax.fuerza_parallax = 0.0;
    sin_parallax.cobertura_nubes = 0.0;
    let mut con_parallax = Rocky::default();
    con_parallax.cobertura_nubes = 0.0;
    let cam_rasante = Camera {
        distancia: 2.2,
        yaw: 0.9,
        pitch: 0.1,
        ..Camera::default()
    };
    println!("  capturando terralba_parallax_sin.png / _con.png");
    frame(mesh, &sin_parallax, &cam_rasante, luz_frontal, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_parallax_sin.png"))?;
    frame(mesh, &con_parallax, &cam_rasante, luz_frontal, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_parallax_con.png"))?;

    // Comparación del bump mapping.
    let mut sin_bump = Rocky::default();
    sin_bump.fuerza_relieve = 0.0;
    sin_bump.cobertura_nubes = 0.0;
    // Luz lateral respecto de esa cámara: así el relieve proyecta sombras largas.
    let luz_relieve = crate::math::rotate_y(cam_rasante.eye().normalize(), -0.95);
    println!("  capturando terralba_bump_sin.png / _con.png");
    frame(mesh, &sin_bump, &cam_rasante, luz_relieve, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_bump_sin.png"))?;
    frame(mesh, &con_parallax, &cam_rasante, luz_relieve, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_bump_con.png"))?;

    // Sombras de nubes: cámara cercana y luz rasante, que es cuando la sombra se
    // separa de la nube que la proyecta. Las dos tomas sólo cambian el parámetro
    // `altura_nubes`: con la capa más alta, la sombra se aleja de su nube.
    let nublado = |altura: f32| {
        let mut r = Rocky::default();
        r.altura_nubes = altura;
        r
    };
    let luz_sombras = crate::math::rotate_y(cam_rasante.eye().normalize(), -1.15);
    println!("  capturando terralba_sombras_baja.png / _alta.png");
    frame(mesh, &nublado(0.02), &cam_rasante, luz_sombras, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_sombras_baja.png"))?;
    frame(mesh, &nublado(0.12), &cam_rasante, luz_sombras, 0.0, 700, 700, 2)
        .save_png(dir.join("terralba_sombras_alta.png"))?;

    Ok(())
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
    fn el_cuadro_sale_del_tamano_pedido() {
        let fb = frame(
            &esfera(),
            &Star::default(),
            &Camera::default(),
            v3(0.5, 0.4, 0.8),
            0.0,
            40,
            30,
            2,
        );
        assert_eq!((fb.width, fb.height), (40, 30));
    }

    #[test]
    fn se_escribe_un_png_legible() {
        let dir = std::env::temp_dir().join("shaders_prueba_png");
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("prueba.png");
        let fb = frame(
            &esfera(),
            &Rocky::default(),
            &Camera::default(),
            v3(0.5, 0.4, 0.8),
            0.0,
            32,
            32,
            1,
        );
        fb.save_png(&ruta).unwrap();
        let bytes = std::fs::read(&ruta).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        std::fs::remove_file(&ruta).ok();
    }

}
