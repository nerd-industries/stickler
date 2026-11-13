// Hide the console window on Windows release builds (harmless elsewhere).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's native Wayland path paints a blank window on some systems
    // (seen on KDE Plasma 6 + AMD). XWayland renders correctly. Respect any
    // GDK_BACKEND the user already set.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none() {
        // SAFETY: called before any other threads exist.
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }
    nelko_app::run()
}
