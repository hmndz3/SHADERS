//! Laboratorio de shaders: una misma esfera, tres cuerpos celestes.
//!
//! Uso:
//!   cargo run --release              abre la ventana interactiva

mod framebuffer;
mod math;
mod noise;
mod obj;
mod render;
mod shaders;

use framebuffer::Framebuffer;
use math::{rotate_y, v3};
use minifb::{Key, Window, WindowOptions};
use render::{Camera, Renderer};
use shaders::{gas::GasGiant, rocky::Rocky, star::Star, Shader, Uniforms};
use std::time::Instant;

const ANCHO: usize = 900;
const ALTO: usize = 700;

fn cuerpos() -> Vec<Box<dyn Shader>> {
    vec![
        Box::new(Star::default()),
        Box::new(Rocky::default()),
        Box::new(GasGiant::default()),
    ]
}

fn main() {
    let mesh = match obj::load("assets/sphere.obj") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error cargando la esfera: {e}");
            eprintln!("Ejecutá el programa desde la raíz del repositorio.");
            std::process::exit(1);
        }
    };
    println!(
        "Esfera cargada: {} vértices, {} triángulos",
        mesh.vertices.len(),
        mesh.triangle_count()
    );

    ventana(mesh);
}

fn ventana(mesh: obj::Mesh) {
    let shaders = cuerpos();
    let mut actual = 1usize; // arranca en el planeta rocoso

    let mut window = Window::new(
        "Cuerpos celestes",
        ANCHO,
        ALTO,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("no se pudo abrir la ventana: {e}"));
    window.set_target_fps(60);

    let mut renderer = Renderer::new(ANCHO, ALTO);
    let mut fb = Framebuffer::new(ANCHO, ALTO);
    let mut camara = Camera::default();

    let mut luz_yaw: f32 = 0.6;
    let mut luz_alt: f32 = 0.35;
    let mut tiempo = 0.0f32;
    let mut pausado = false;
    let mut captura = 0usize;

    let inicio = Instant::now();
    let mut ultimo = inicio;
    let mut fps = 0.0f32;

    imprimir_ayuda();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let ahora = Instant::now();
        let dt = (ahora - ultimo).as_secs_f32().min(0.1);
        ultimo = ahora;
        fps = fps * 0.9 + (1.0 / dt.max(1e-4)) * 0.1;
        if !pausado {
            tiempo += dt;
        }

        // Selección de cuerpo celeste.
        if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
            actual = 0;
        }
        if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
            actual = 1;
        }
        if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
            actual = 2;
        }

        // Cámara en órbita (el modelo no se mueve).
        let vel = 1.4 * dt;
        if window.is_key_down(Key::Left) {
            camara.yaw -= vel;
        }
        if window.is_key_down(Key::Right) {
            camara.yaw += vel;
        }
        if window.is_key_down(Key::Up) {
            camara.pitch = (camara.pitch + vel).min(1.45);
        }
        if window.is_key_down(Key::Down) {
            camara.pitch = (camara.pitch - vel).max(-1.45);
        }
        if window.is_key_down(Key::Z) {
            camara.distancia = (camara.distancia - 2.0 * dt).max(1.25);
        }
        if window.is_key_down(Key::X) {
            camara.distancia = (camara.distancia + 2.0 * dt).min(9.0);
        }

        // Luz direccional.
        if window.is_key_down(Key::A) {
            luz_yaw -= vel;
        }
        if window.is_key_down(Key::D) {
            luz_yaw += vel;
        }
        if window.is_key_down(Key::W) {
            luz_alt = (luz_alt + vel).min(1.4);
        }
        if window.is_key_down(Key::S) {
            luz_alt = (luz_alt - vel).max(-1.4);
        }

        if window.is_key_pressed(Key::Space, minifb::KeyRepeat::No) {
            pausado = !pausado;
        }
        if window.is_key_pressed(Key::R, minifb::KeyRepeat::No) {
            camara = Camera::default();
            luz_yaw = 0.6;
            luz_alt = 0.35;
            tiempo = 0.0;
        }

        let luz = rotate_y(
            v3(0.0, luz_alt.sin(), luz_alt.cos()),
            luz_yaw,
        )
        .normalize();

        renderer.draw(
            &mut fb,
            &mesh,
            shaders[actual].as_ref(),
            &Uniforms { time: tiempo },
            &camara,
            luz,
        );

        if window.is_key_pressed(Key::P, minifb::KeyRepeat::No) {
            std::fs::create_dir_all("capturas").ok();
            let nombre = format!("capturas/pantalla_{captura:03}.png");
            match fb.save_png(&nombre) {
                Ok(()) => println!("captura guardada en {nombre}"),
                Err(e) => eprintln!("no se pudo guardar: {e}"),
            }
            captura += 1;
        }

        window.set_title(&format!(
            "{}  —  {}  |  {:.0} fps  |  1/2/3 cambia de cuerpo  |  {}",
            shaders[actual].nombre(),
            shaders[actual].descripcion(),
            fps,
            if pausado { "PAUSA" } else { "t avanza" }
        ));

        window
            .update_with_buffer(&fb.to_u32(), ANCHO, ALTO)
            .expect("no se pudo actualizar la ventana");
    }
}

fn imprimir_ayuda() {
    println!(
        "\nControles
  1 / 2 / 3      Brasa (estrella) / Terralba (rocoso) / Ámbar (gigante gaseoso)
  Flechas        orbitar la cámara
  Z / X          acercar / alejar
  A / D          girar la luz
  W / S          subir / bajar la luz
  Espacio        pausar el tiempo
  R              reiniciar cámara, luz y tiempo
  P              guardar una captura PNG en capturas/
  Esc            salir\n"
    );
}
