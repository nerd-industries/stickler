//! Stickler desktop app (Tauri v2) for the Nelko P21.

pub mod commands;

use nelko_core::service::Nelko;
use std::sync::Arc;
use tauri::{Emitter, Manager};

pub fn run() {
    let nelko = Arc::new(Nelko::new().expect("load settings and templates"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(nelko)
        .setup(|app| {
            // Check the printer once at start-up and tell the UI.
            let handle = app.handle().clone();
            let n = app.state::<Arc<Nelko>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let report = n.report().await;
                let _ = handle.emit("printer-report", report);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::printer_report,
            commands::discover_printers,
            commands::pair_printer,
            commands::list_templates,
            commands::icon_names,
            commands::label_presets,
            commands::preview,
            commands::print,
            commands::save_template,
            commands::delete_template,
            commands::duplicate_template,
            commands::get_settings,
            commands::set_settings,
            commands::self_test,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Stickler");
}
