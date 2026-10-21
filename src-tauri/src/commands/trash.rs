//! Corbeille interne : envoi, restauration, vidage et délai de conservation.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::error::CommandResult;
use super::marks::XmpWriter;
use super::warnings::{WarningKind, warn};
use super::{AppState, blocking};
use crate::library::trash::{self, TrashReport};
use crate::library::{marks, pairs};
use crate::metadata::media::MediaFormat;

/// Envoie des photos à la corbeille : les piles entières (pile repliée), ou
/// seulement les fichiers désignés (`whole_piles` faux, pile dépliée).
#[tauri::command]
pub async fn media_trash(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
    whole_piles: Option<bool>,
) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    // Une copie virtuelle n'est pas un fichier : elle ne va pas à la corbeille.
    let ids: Vec<i64> = ids.into_iter().filter(|id| *id > 0).collect();
    blocking(move || {
        let ids = if whole_piles.unwrap_or(true) {
            pairs::expand(&conn, &ids, Some("normal"))?
        } else {
            ids
        };
        Ok(trash_files(&app, &mut conn, &ids)?)
    })
    .await
}

fn trash_files(
    app: &AppHandle,
    conn: &mut rusqlite::Connection,
    ids: &[i64],
) -> Result<TrashReport, trash::TrashError> {
    // Les notes en attente partent dans le XMP avant que le fichier ne bouge :
    // la photo restaurée retrouve ses marques.
    match marks::flush_pending_xmp(conn, usize::MAX) {
        Ok(report) if report.failed > 0 => warn(app, WarningKind::Xmp, report.failed, ""),
        Ok(_) => {}
        Err(err) => warn(
            app,
            WarningKind::Xmp,
            1,
            format!("XMP avant corbeille : {err}"),
        ),
    }
    trash::move_to_trash(conn, ids)
}

/// Photo > Envoyer le JPEG (ou le RAW) dans la corbeille : dans chaque pile,
/// les fichiers de ce format partent, l'autre reste. Une photo seule, ou une
/// pile dont tous les fichiers ont ce format, n'est pas touchée.
#[tauri::command]
pub async fn media_trash_format(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
    format: MediaFormat,
) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    let ids: Vec<i64> = ids.into_iter().filter(|id| *id > 0).collect();
    blocking(move || {
        let chosen = pairs::files_of_format(&conn, &ids, format)?;
        Ok(trash_files(&app, &mut conn, &chosen)?)
    })
    .await
}

/// Photo > Mettre en haut de la pile.
#[tauri::command]
pub async fn pile_set_top(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    if id < 0 {
        return Ok(());
    }
    let mut conn = state.connect()?;
    blocking(move || Ok(pairs::set_top(&mut conn, id)?)).await
}

#[tauri::command]
pub async fn media_restore(
    state: State<'_, AppState>,
    writer: State<'_, XmpWriter>,
    ids: Vec<i64>,
) -> CommandResult<TrashReport> {
    let mut conn = state.connect()?;
    let report = blocking(move || {
        let ids = pairs::expand(&conn, &ids, Some("trashed"))?;
        Ok(trash::restore(&mut conn, &ids)?)
    })
    .await?;
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
            Some(ids) => pairs::expand(&conn, &ids, Some("trashed"))?,
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
            count: trash::trashed_count(&conn, library_id)?,
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
