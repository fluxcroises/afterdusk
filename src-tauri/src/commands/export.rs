//! Livrer : réglages d'export (presets, filigrane, destination) et export en
//! arrière-plan, progression envoyée par événements.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{CommandError, CommandResult};
use super::{AppState, blocking};
use crate::export::ExportError;
use crate::export::job::{self, ExportRequest, Progress};
use crate::export::presets::{self, ExportSettings, FILE};

pub const EVENT_PROGRESS: &str = "export://progress";
pub const EVENT_FINISHED: &str = "export://finished";

impl From<ExportError> for CommandError {
    fn from(e: ExportError) -> Self {
        Self::new("internal", e)
    }
}

/// Export en cours : son drapeau d'arrêt.
#[derive(Default)]
pub struct ExportState {
    running: Mutex<Option<Arc<AtomicBool>>>,
}

fn settings_file(state: &AppState) -> PathBuf {
    state
        .db_path
        .parent()
        .map_or_else(|| PathBuf::from(FILE), |d| d.join(FILE))
}

#[tauri::command]
pub async fn export_settings(state: State<'_, AppState>) -> CommandResult<ExportSettings> {
    let path = settings_file(&state);
    blocking(move || Ok(presets::load(&path)?)).await
}

#[tauri::command]
pub async fn export_settings_save(
    state: State<'_, AppState>,
    settings: ExportSettings,
) -> CommandResult<ExportSettings> {
    let path = settings_file(&state);
    blocking(move || Ok(presets::save(&path, settings)?)).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Finished {
    progress: Option<Progress>,
    cancelled: bool,
    destination: PathBuf,
    error: Option<CommandError>,
}

/// Ouvre un dossier dans l'explorateur de fichiers du système.
fn reveal(dir: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    let _ = std::process::Command::new(program).arg(dir).spawn();
}

/// Lance l'export ; la progression et la fin arrivent en événements.
#[tauri::command]
pub fn export_start(
    app: AppHandle,
    state: State<'_, AppState>,
    exports: State<'_, ExportState>,
    request: ExportRequest,
) -> CommandResult<()> {
    let cancel = {
        let mut slot = exports
            .running
            .lock()
            .map_err(|e| CommandError::new("internal", e))?;
        if slot.is_some() {
            return Err(CommandError::new(
                "exportRunning",
                "un export est déjà en cours",
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    let db_path = state.db_path.clone();
    let settings_path = settings_file(&state);
    std::thread::spawn(move || {
        let result = presets::load(&settings_path).and_then(|settings| {
            let out = job::run(&db_path, &settings, &request, &cancel, &|p| {
                let _ = app.emit(EVENT_PROGRESS, p);
            });
            out.map(|p| (p, settings.open_folder))
        });
        if let Ok(mut slot) = app.state::<ExportState>().running.lock() {
            *slot = None;
        }
        let cancelled = cancel.load(Ordering::SeqCst);
        let finished = match result {
            Ok((progress, open)) => {
                if open && !cancelled && progress.done > progress.failures.len() {
                    reveal(&request.destination);
                }
                Finished {
                    progress: Some(progress),
                    cancelled,
                    destination: request.destination.clone(),
                    error: None,
                }
            }
            Err(e) => Finished {
                progress: None,
                cancelled,
                destination: request.destination.clone(),
                error: Some(e.into()),
            },
        };
        let _ = app.emit(EVENT_FINISHED, finished);
    });
    Ok(())
}

/// Arrête l'export en cours, entre deux fichiers.
#[tauri::command]
pub fn export_cancel(exports: State<'_, ExportState>) -> CommandResult<()> {
    if let Ok(slot) = exports.running.lock()
        && let Some(cancel) = slot.as_ref()
    {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Ouvre le dossier d'un export terminé.
#[tauri::command]
pub fn export_reveal(destination: PathBuf) -> CommandResult<()> {
    if destination.is_dir() {
        reveal(&destination);
    }
    Ok(())
}
