//! Collections manuelles et albums intelligents.

use tauri::State;

use super::error::CommandResult;
use super::{AppState, blocking};
use crate::library::collections::{self, Collection};
use crate::library::filter::MediaFilter;

#[tauri::command]
pub async fn collections_list(
    state: State<'_, AppState>,
    library_id: i64,
) -> CommandResult<Vec<Collection>> {
    let conn = state.connect()?;
    blocking(move || Ok(collections::list(&conn, library_id)?)).await
}

/// Collection manuelle sans filtre, album intelligent avec.
#[tauri::command]
pub async fn collection_create(
    state: State<'_, AppState>,
    library_id: i64,
    name: String,
    filter: Option<MediaFilter>,
) -> CommandResult<i64> {
    let conn = state.connect()?;
    blocking(move || {
        Ok(collections::create(
            &conn,
            library_id,
            &name,
            filter.as_ref(),
        )?)
    })
    .await
}

#[tauri::command]
pub async fn collection_rename(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(collections::rename(&conn, id, &name)?)).await
}

#[tauri::command]
pub async fn collection_set_filter(
    state: State<'_, AppState>,
    id: i64,
    filter: MediaFilter,
) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(collections::set_filter(&conn, id, &filter)?)).await
}

#[tauri::command]
pub async fn collection_delete(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    let conn = state.connect()?;
    blocking(move || Ok(collections::delete(&conn, id)?)).await
}

#[tauri::command]
pub async fn collection_add_media(
    state: State<'_, AppState>,
    id: i64,
    media_ids: Vec<i64>,
) -> CommandResult<usize> {
    let mut conn = state.connect()?;
    blocking(move || Ok(collections::add_media(&mut conn, id, &media_ids)?)).await
}

#[tauri::command]
pub async fn collection_remove_media(
    state: State<'_, AppState>,
    id: i64,
    media_ids: Vec<i64>,
) -> CommandResult<usize> {
    let conn = state.connect()?;
    blocking(move || Ok(collections::remove_media(&conn, id, &media_ids)?)).await
}
