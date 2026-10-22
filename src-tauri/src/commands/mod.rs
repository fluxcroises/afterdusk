//! Commandes Tauri exposées à l'interface.

pub mod analysis;
pub mod bursts;
pub mod clients;
pub mod collections;
pub mod contact_sheet;
pub mod develop;
pub mod devices;
pub mod error;
pub mod export;
pub mod gallery;
pub mod gear;
pub mod geo;
pub mod import;
pub mod library;
pub mod marks;
pub mod media;
pub mod presets;
pub mod previews;
pub mod trash;
pub mod warnings;
pub mod windows;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use error::{CommandError, CommandResult};

/// État partagé entre les commandes.
pub struct AppState {
    pub db_path: PathBuf,
    /// Dossier du cache d'aperçus, dans les données de l'application.
    pub cache_root: PathBuf,
    /// Drapeau d'arrêt de l'import en cours, s'il y en a un.
    pub running_import: Mutex<Option<Arc<AtomicBool>>>,
    /// Source analysée : seule source dont le protocole `preview://` peut lire les fichiers.
    pub source_root: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new(db_path: PathBuf, cache_root: PathBuf) -> Self {
        Self {
            db_path,
            cache_root,
            running_import: Mutex::new(None),
            source_root: Mutex::new(None),
        }
    }

    /// Une connexion par appel : SQLite en WAL le permet sans verrou applicatif.
    /// Dossier des données de l'application (celui de la base).
    pub fn data_dir(&self) -> PathBuf {
        self.db_path.parent().map(PathBuf::from).unwrap_or_default()
    }

    pub fn connect(&self) -> CommandResult<Connection> {
        Ok(db::connect(&self.db_path)?)
    }
}

/// Version d'Afterdusk et de ses composants, pour « À propos » et les
/// informations système jointes à un signalement de bogue.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub libraw: String,
    pub sqlite: &'static str,
    pub tauri: &'static str,
    /// Moteur d'affichage : WebView2 sous Windows, WebKit sous macOS.
    pub webview: Option<String>,
    pub os: &'static str,
    pub arch: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        libraw: crate::raw::libraw_version(),
        sqlite: rusqlite::version(),
        tauri: tauri::VERSION,
        webview: tauri::webview_version().ok(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

/// Variable d'environnement de l'essai de lancement (CI) : l'application
/// s'arrête dès que la première fenêtre a affiché l'interface.
pub const SMOKE_ENV: &str = "AFTERDUSK_SMOKE";

/// L'interface de la première fenêtre est affichée. En essai de lancement,
/// l'application s'arrête là, avec succès ; sinon, rien.
#[tauri::command]
pub fn app_ready(app: tauri::AppHandle, window: tauri::Window) {
    if std::env::var_os(SMOKE_ENV).is_some() {
        println!("afterdusk : interface affichée ({})", window.label());
        app.exit(0);
    }
}

/// Exécute un traitement bloquant hors du fil de l'interface.
pub async fn blocking<T, F>(f: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::new("internal", e))?
}
