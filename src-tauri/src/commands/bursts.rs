//! Rafales : détection en arrière-plan, empilage et retrait à la main.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::bursts::{self, DetectReport};

pub const EVENT_PROGRESS: &str = "bursts-progress";

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

/// Recalcule les rafales automatiques d'une bibliothèque, ou d'un album.
#[tauri::command]
pub async fn bursts_detect(
    app: AppHandle,
    state: State<'_, AppState>,
    library_id: i64,
    album_id: Option<i64>,
) -> CommandResult<DetectReport> {
    let mut conn = state.connect()?;
    let cache = state.cache_root.clone();
    blocking(move || {
        Ok(bursts::detect(
            &mut conn,
            &cache,
            library_id,
            album_id,
            |done, total| {
                let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
            },
        )?)
    })
    .await
}

/// Empile des photos à la main ; la première passe au-dessus.
#[tauri::command]
pub async fn bursts_stack(
    state: State<'_, AppState>,
    library_id: i64,
    ids: Vec<i64>,
) -> CommandResult<Option<i64>> {
    let mut conn = state.connect()?;
    let ids: Vec<i64> = ids.into_iter().filter(|&id| id > 0).collect();
    blocking(move || Ok(bursts::stack(&mut conn, library_id, &ids)?)).await
}

/// Retire des photos de leur rafale.
#[tauri::command]
pub async fn bursts_unstack(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<usize> {
    let mut conn = state.connect()?;
    let ids: Vec<i64> = ids.into_iter().filter(|&id| id > 0).collect();
    blocking(move || Ok(bursts::unstack(&mut conn, &ids)?)).await
}
