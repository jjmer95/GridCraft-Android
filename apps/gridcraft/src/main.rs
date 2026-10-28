//! GridCraft desktop app.
//!
//! Usage: `gridcraft [--control <port>] [--sample <name>] [files…]`
//!
//! `--control <port>` (or `GRIDCRAFT_CONTROL_PORT`) starts a localhost JSON-lines control
//! server: `{"id":1,"method":"ui.inspect","params":{}}` → `{"id":1,"ok":true,"result":…}`.
//! See `gridcraft_ui_egui::control` and `docs/control-protocol.md` for the methods.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "macos")]
mod apple_events;
mod control_server;
#[cfg(any(target_os = "windows", test))]
mod graphics;
mod logging;
#[cfg(target_os = "macos")]
mod native_menu;
mod print;

use std::cell::RefCell;
use std::rc::Rc;

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

struct App(SheetApp, #[cfg(target_os = "macos")] Option<native_menu::NativeMenu>, #[cfg(target_os = "macos")] fmv_macos_events::Inbox);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        {
            if self.1.is_none() && std::env::var_os("GRIDCRAFT_NO_NATIVE_MENU").is_none() {
                self.1 = Some(native_menu::NativeMenu::install(ctx));
            }
            if let Some(m) = &self.1 {
                m.poll(&mut self.0, ctx);
            }
            apple_events::poll(&self.2, &mut self.0, ctx);
        }
        self.0.logic(ctx);
        self.0.poll_printing(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && self.0.printing.busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(raw);
        // Files dropped on the window open as workbooks.
        for f in std::mem::take(&mut raw.dropped_files) {
            if let Some(p) = f.path().to_str().map(str::to_string).filter(|p| !p.is_empty()) {
                self.0.open_path(&p);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
    fn on_exit(&mut self) {
        print::finish_pending_jobs();
        save_prefs(&self.0);
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

fn config_dir() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support/GridCraft"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("GridCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
            .map(|c| c.join("gridcraft"))
    }
}

fn prefs_enabled() -> bool {
    std::env::var_os("GRIDCRAFT_NO_PREFS").is_none()
}

fn log_dir() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("logs"))
}

fn load_prefs(app: &mut SheetApp) {
    if !prefs_enabled() {
        return;
    }
    app.ui.language = gridcraft_ui_egui::i18n::Language::system();
    app.ui.system_theme = true;
    let Some(dir) = config_dir() else { return };
    if let Ok(b) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<gridcraft_ui_egui::UiState>(&b)
    {
        let saved = serde_json::from_slice(&b).ok().and_then(|v| gridcraft_ui_egui::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui;
        app.ui.language = language;
    }
    if let Ok(b) = std::fs::read(dir.join("prefs.json"))
        && let Ok(p) = serde_json::from_slice(&b)
    {
        app.session.prefs = p;
    }
}

fn save_prefs(app: &SheetApp) {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = config_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec_pretty(&app.ui) {
        let _ = std::fs::write(dir.join("ui.json"), b);
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.session.prefs) {
        let _ = std::fs::write(dir.join("prefs.json"), b);
    }
}

fn read_clipboard<T>(
    handle: &RefCell<Option<arboard::Clipboard>>,
    read: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    let mut clipboard = handle.try_borrow_mut().ok()?;
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    read(clipboard.as_mut()?).ok()
}

#[cfg(not(target_os = "android"))]
fn services() -> Services {
    let clipboard: Rc<RefCell<Option<arboard::Clipboard>>> = Rc::default();
    let (html_clipboard, text_clipboard) = (clipboard.clone(), clipboard.clone());
    Services {
        copy_html: Some(Box::new(move |html, text| {
            let mut clipboard = clipboard.borrow_mut();
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
            }
            clipboard.as_mut().ok_or("clipboard unavailable")?.set_html(html, Some(text)).map_err(|e| e.to_string())
        })),
        paste_html: Some(Box::new(move || read_clipboard(&html_clipboard, |clipboard| clipboard.get().html()))),
        paste_text: Some(Box::new(move || read_clipboard(&text_clipboard, |clipboard| clipboard.get_text()))),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Spreadsheets", &["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "json"])
                .add_filter("Excel Workbook", &["xlsx", "xlsm"])
                .add_filter("Excel 97-2003 Workbook (import)", &["xls"])
                .add_filter("Excel Binary Workbook (data import)", &["xlsb"])
                .add_filter("OpenDocument Spreadsheet (data import)", &["ods"])
                .add_filter("CSV", &["csv"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Pictures (PNG, JPEG)", &["png", "jpg", "jpeg"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let path = std::path::Path::new(suggested);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Book1").to_string();
            rfd::FileDialog::new()
                .set_file_name(&stem)
                .save_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
    }
}
    

#[cfg(not(target_os = "android"))]
fn main() {
    // 1. Inicializar logs de escritorio
    if prefs_enabled() {
        if let Some(dir) = log_dir() {
            let _ = logging::init(&dir);
        }
    }

    // 2. Construir la sesión y app base
    let mut session = Session::default();
    let mut app = SheetApp::new(&mut session, services());
    load_prefs(&mut app);

    // 3. Procesar argumentos de línea de comandos de escritorio
    let mut files = vec![];
    let mut control_port = std::env::var("GRIDCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--control" {
            control_port = args.next().and_then(|p| p.parse().ok());
        } else if arg == "--sample" {
            if let Some(name) = args.next() {
                app.open_sample(&name);
            }
        } else if !arg.starts_with('-') {
            files.push(arg);
        }
    }
    for f in files {
        app.open_path(&f);
    }

    // 4. Iniciar servidor de control si se solicita
    let _server = control_port.map(|port| control_server::Server::start(port, &app.session));

    // 5. Opciones nativas de eframe para Escritorio
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GridCraft")
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    // 6. Lanzar la app de escritorio
    #[cfg(target_os = "macos")]
    let native_menu = None;
    #[cfg(target_os = "macos")]
    let apple_inbox = fmv_macos_events::Inbox::default();

    let _ = eframe::run_native(
        "GridCraft",
        options,
        Box::new(|_cc| {
            Ok(Box::new(App(
                app,
                #[cfg(target_os = "macos")] native_menu,
                #[cfg(target_os = "macos")] apple_inbox,
            )))
        }),
    );
}
