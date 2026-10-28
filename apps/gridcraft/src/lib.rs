// apps/gridcraft/src/lib.rs
#![cfg(target_os = "android")]

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use std::cell::RefCell;
use std::rc::Rc;

// Declaramos los módulos compartidos necesarios para que el compilador no falle
mod print;

struct AndroidApp(SheetApp);

impl eframe::App for AndroidApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.0.logic(ctx);
        self.0.poll_printing(ctx);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
    fn on_exit(&mut self) {
        // En Android simplemente delegamos el cierre al sistema operativo
    }
}

// Servicios mockeados o simplificados para Android (evitando fallos de arboard/rfd en móvil)
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

#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Inicializa el logger interno de Android para ver logs mediante logcat
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info)
    );
    
    log::info!("¡GridCraft Android levantando entorno gráfico!");

    // Configuramos las opciones nativas inyectando la actividad de Android requerida por eframe
    let mut options = eframe::NativeOptions::default();
    options.android_activity = Some(app);

    let mut session = Session::default();
    let app_instance = SheetApp::new(&mut session, android_services());

    let _ = eframe::run_native(
        "GridCraft",
        options,
        Box::new(|_cc| {
            Ok(Box::new(AndroidApp(app_instance)))
        }),
    );
}
