//! Mises à jour : vérification au démarrage, installation sur clic.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use super::{AppState, CommandError, CommandResult, blocking};
use crate::library::LibraryError;
use crate::updates;

/// Événement de progression du téléchargement.
pub const EVENT_PROGRESS: &str = "update://progress";

/// Variable qui permet d'essayer la vérification dans une compilation de
/// débogage ; sans elle, une compilation de débogage ne fait aucune requête.
const DEBUG_ENV: &str = "AFTERDUSK_UPDATE_CHECK";

/// Mise à jour trouvée, gardée jusqu'au clic sur « Installer ».
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub async fn updates_enabled_get(state: State<'_, AppState>) -> CommandResult<bool> {
    let conn = state.connect()?;
    blocking(move || Ok(updates::enabled(&conn).map_err(LibraryError::from)?)).await
}

#[tauri::command]
pub async fn updates_enabled_set(state: State<'_, AppState>, enabled: bool) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(updates::set_enabled(&conn, enabled).map_err(LibraryError::from)?)).await
}

/// Vérification automatique : aucune requête si elle est désactivée ou déjà
/// faite depuis moins d'un jour. Une erreur réseau ne s'affiche pas : la
/// vérification reprendra le lendemain.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    state: State<'_, AppState>,
    pending: State<'_, PendingUpdate>,
) -> CommandResult<Option<UpdateInfo>> {
    if cfg!(debug_assertions) && std::env::var_os(DEBUG_ENV).is_none() {
        return Ok(None);
    }
    let conn = state.connect()?;
    let now = updates::now();
    let due = blocking(move || {
        let due = updates::due(
            updates::enabled(&conn).map_err(LibraryError::from)?,
            updates::last_check(&conn).map_err(LibraryError::from)?,
            now,
        );
        if due {
            updates::set_last_check(&conn, now).map_err(LibraryError::from)?;
        }
        Ok(due)
    })
    .await?;
    if !due {
        return Ok(None);
    }
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let update = match found {
        Ok(update) => update,
        Err(e) => {
            eprintln!("afterdusk : vérification des mises à jour impossible : {e}");
            None
        }
    };
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone(),
    });
    *pending
        .0
        .lock()
        .map_err(|e| CommandError::new("internal", e))? = update;
    Ok(info)
}

/// Télécharge et installe la mise à jour trouvée, puis relance Afterdusk.
/// L'archive est vérifiée avec la clé publique du projet avant installation.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> CommandResult<()> {
    let update = pending
        .0
        .lock()
        .map_err(|e| CommandError::new("internal", e))?
        .take()
        .ok_or_else(|| CommandError::new("noUpdate", "aucune mise à jour en attente"))?;
    let mut downloaded = 0u64;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(EVENT_PROGRESS, Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| CommandError::new("updateFailed", e))?;
    app.restart()
}
