//! Commandes Tauri exposées à l'interface.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    pub libraw: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        libraw: crate::raw::libraw_version(),
    }
}
