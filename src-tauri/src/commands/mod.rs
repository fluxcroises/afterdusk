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
pub mod ftp;
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
pub mod video;
pub mod warnings;
pub mod watch;
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
    /// Version du système (« macOS 26.6.2 », « Windows 10.0.26200 »).
    pub os_version: Option<String>,
    /// Processeur (« Apple M3 Pro », « 12th Gen Intel(R) Core(TM) i7-12700 »).
    pub cpu: Option<String>,
}

/// Sortie d'une commande du système, première ligne, sans espaces autour.
fn system_line(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Pas de fenêtre de console.
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.to_owned())
}

fn os_version() -> Option<String> {
    if cfg!(target_os = "macos") {
        system_line("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}"))
    } else if cfg!(windows) {
        // « Microsoft Windows [version 10.0.26200.6584] » : le numéro seul.
        let line = system_line("cmd", &["/c", "ver"])?;
        let v = line.rsplit(' ').next()?.trim_end_matches(']');
        Some(format!("Windows {v}"))
    } else {
        system_line("uname", &["-sr"])
    }
}

fn cpu() -> Option<String> {
    if cfg!(target_os = "macos") {
        system_line("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else if cfg!(windows) {
        std::env::var("PROCESSOR_IDENTIFIER").ok()
    } else {
        None
    }
}

/// Site de Flux Croisés, l'association qui publie Afterdusk.
const PUBLISHER_SITE: &str = "https://flux-croises.fr";

/// Ouvre le site de l'éditeur dans le navigateur par défaut, sur clic dans
/// « À propos ». L'adresse est fixe : l'interface ne peut pas en ouvrir d'autre.
#[tauri::command]
pub fn open_publisher_site() {
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    let _ = std::process::Command::new(program)
        .arg(PUBLISHER_SITE)
        .spawn();
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
        os_version: os_version(),
        cpu: cpu(),
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
