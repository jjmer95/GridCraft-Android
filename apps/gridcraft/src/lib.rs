// apps/gridcraft/src/lib.rs
#![cfg(target_os = "android")]
#![allow(unused_imports, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};

// Declaración de módulos compartidos del proyecto
mod control_server;
mod logging;
mod print;

struct AndroidApp(SheetApp);

impl eframe::App for AndroidApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ejecuta la lógica principal del motor de GridCraft en la interfaz gráfica
        self.0.logic(ctx);
        self.0.poll_printing(ctx);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Dibuja la cuadrícula y las celdas en la pantalla táctil
        self.0.ui(ui);
    }
}

/// Proveedor de servicios simulados para evitar llamadas al sistema de escritorio.
fn android_services() -> Services {
    Services {
        copy_html: None,
        paste_html: None,
        paste_text: None,
        pick_open: None,
        pick_picture: None,
        pick_save: None,
    }
}

/// Punto de entrada exclusivo para dispositivos Android (Game-Activity)
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Redirecciona los logs del motor hacia Logcat de Android
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info)
    );
    
    log::info!("¡GridCraft Android levantando entorno gráfico con Game-Activity!");

    // Inyecta la actividad nativa directamente en las opciones de renderizado de eframe
    let mut options = eframe::NativeOptions::default();
    options.android_activity = Some(app);

    // Inicializa la sesión de cálculo subyacente
    let mut session = Session::default();
    let app_instance = SheetApp::new(&mut session, android_services());

    // Levanta el bucle de renderizado nativo sobre la superficie de Android
    let _ = eframe::run_native(
        "GridCraft",
        options,
        Box::new(|_cc| {
            Ok(Box::new(AndroidApp(app_instance)))
        }),
    );
}
