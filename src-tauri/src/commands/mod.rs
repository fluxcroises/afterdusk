//! Commandes Tauri exposées à l'interface.

pub mod error;
pub mod import;
pub mod library;
pub mod marks;
pub mod media;
pub mod previews;

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
    pub fn connect(&self) -> CommandResult<Connection> {
        Ok(db::connect(&self.db_path)?)
    }
}

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
