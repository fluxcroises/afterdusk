//! Corbeille interne : envoi, restauration, vidage et délai de conservation.

use serde::Serialize;
use tauri::State;

use super::error::CommandResult;
use super::marks::XmpWriter;
use super::{AppState, blocking};
use crate::library::marks;
use crate::library::trash::{self, TrashReport};

#[tauri::command]
pub async fn media_trash(state: State<'_, AppState>, ids: Vec<i64>) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    blocking(move || {
        // Les notes en attente partent dans le XMP avant que le fichier ne bouge :
        // la photo restaurée retrouve ses marques.
        if let Err(err) = marks::flush_pending_xmp(&conn, usize::MAX) {
            eprintln!("XMP avant corbeille : {err}");
        }
        Ok(trash::move_to_trash(&mut conn, &ids)?)
    })
    .await
}

#[tauri::command]
pub async fn media_restore(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    let report = blocking(move || Ok(trash::restore(&mut conn, &ids)?)).await?;
    writer.wake();
    Ok(report)
}

/// Suppression définitive : `ids` choisis dans la corbeille, ou toute la
/// corbeille de la bibliothèque.
#[tauri::command]
pub async fn trash_empty(
    state: State<'_, AppState>,
    library_id: i64,
    ids: Option<Vec<i64>>,
) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    let cache_root = state.cache_root.clone();
    blocking(move || {
        let ids = match ids {
            Some(ids) => ids,
            None => trash::trashed_ids(&conn, Some(library_id), None)?,
        };
        Ok(trash::purge(&mut conn, &cache_root, &ids)?)
    })
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashSummary {
    pub count: usize,
    pub retention_days: u32,
}

/// Nombre de photos dans la corbeille et délai avant leur suppression.
#[tauri::command]
pub async fn trash_summary(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<TrashSummary> {
    let conn = state.connect()?;
    blocking(move || {
        Ok(TrashSummary {
            count: trash::trashed_ids(&conn, Some(library_id), None)?.len(),
            retention_days: trash::retention_days(&conn)?,
        })
    })
    .await
}

#[tauri::command]
pub async fn trash_retention_get(state: State<'_, AppState>) -> CommandResult<u32> {
    let conn = state.connect()?;
    blocking(move || Ok(trash::retention_days(&conn)?)).await
}

#[tauri::command]
pub async fn trash_retention_set(state: State<'_, AppState>, days: u32) -> CommandResult<u32> {
    let conn = state.connect()?;
    blocking(move || {
        trash::set_retention_days(&conn, days)?;
        Ok(trash::retention_days(&conn)?)
    })
    .await
}
