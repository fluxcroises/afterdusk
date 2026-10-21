//! Détection des ratés en arrière-plan.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::analysis::run::{self, RunReport};

pub const EVENT_PROGRESS: &str = "analysis-progress";

/// Une seule analyse à la fois ; « Arrêter » l'interrompt entre deux photos.
static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn analysis_run(
    app: AppHandle,
    state: State<'_, AppState>,
    library_id: i64,
    album_id: Option<i64>,
) -> CommandResult<RunReport> {
    let conn = state.connect()?;
    let cache = state.cache_root.clone();
    CANCEL.store(false, Ordering::Relaxed);
    blocking(move || {
        Ok(run::run(
            &conn,
            &cache,
            library_id,
            album_id,
            &CANCEL,
            |done, total| {
                let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
            },
        )?)
    })
    .await
}

#[tauri::command]
pub fn analysis_cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}
