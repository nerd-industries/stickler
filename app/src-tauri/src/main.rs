// Hide the console window on Windows release builds (harmless elsewhere).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nelko_app::run()
}
